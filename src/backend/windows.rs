use crate::runtime::windows_security::Handle;
use std::{
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Power::*, Threading::*},
};

pub struct Resource {
    handle: Option<Handle>,
    set: bool,
}
impl Resource {
    pub fn acquire() -> anyhow::Result<Self> {
        let mut reason: Vec<u16> = super::REASON.encode_utf16().chain(Some(0)).collect();
        let context = REASON_CONTEXT {
            Version: 0,
            Flags: 1,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: reason.as_mut_ptr(),
            },
        };
        // SAFETY: reason and context remain live through the synchronous creation call.
        let handle = unsafe { PowerCreateRequest(&context) };
        anyhow::ensure!(
            handle != INVALID_HANDLE_VALUE && !handle.is_null(),
            "PowerCreateRequest failed"
        );
        let handle = Handle(handle);
        // SystemRequired only: neither display, Away Mode nor ExecutionRequired.
        anyhow::ensure!(
            unsafe { PowerSetRequest(handle.0, PowerRequestSystemRequired) } != 0,
            "PowerSetRequest failed"
        );
        Ok(Self {
            handle: Some(handle),
            set: true,
        })
    }
    pub fn state(&self) -> &'static str {
        if self.handle.is_some() && self.set {
            "accepted"
        } else {
            "none"
        }
    }
    pub fn release(&mut self) -> anyhow::Result<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        let result = if self.set {
            unsafe { PowerClearRequest(handle.0, PowerRequestSystemRequired) }
        } else {
            1
        };
        self.set = false;
        drop(handle);
        // User-initiated sleep may have already terminated the request. Handle still closes.
        anyhow::ensure!(result != 0, "PowerClearRequest failed (handle closed)");
        Ok(())
    }
}
impl Drop for Resource {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if self.set {
                unsafe {
                    PowerClearRequest(handle.0, PowerRequestSystemRequired);
                }
            }
            drop(handle);
        }
    }
}
// Static bounded event bits avoid retaining a pointer to heap/callback context during teardown.
static EVENTS: AtomicU32 = AtomicU32::new(0);
unsafe extern "system" fn callback(
    _: *const core::ffi::c_void,
    kind: u32,
    _: *const core::ffi::c_void,
) -> u32 {
    if kind == 4 {
        EVENTS.fetch_or(1, Ordering::Release);
    } // PBT_APMSUSPEND
    if kind == 7 || kind == 18 {
        EVENTS.fetch_or(2, Ordering::Release);
    } // resume suspend / automatic
    0
}
struct Notification(*mut core::ffi::c_void);
unsafe impl Send for Notification {} // Owned registration; callback uses only static atomics.
impl Notification {
    fn close(&mut self) -> anyhow::Result<()> {
        if self.0.is_null() {
            return Ok(());
        }
        let code = unsafe { PowerUnregisterSuspendResumeNotification(self.0 as HPOWERNOTIFY) };
        anyhow::ensure!(
            code == 0,
            "Suspend/resume notification cleanup failed (Windows error {code})"
        );
        self.0 = ptr::null_mut();
        Ok(())
    }
}
impl Drop for Notification {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn validate_registration(code: u32, registration: *mut core::ffi::c_void) -> anyhow::Result<()> {
    anyhow::ensure!(
        code == 0,
        "Suspend/resume notification registration failed (Windows error {code})"
    );
    anyhow::ensure!(
        !registration.is_null(),
        "Suspend/resume notification registration returned a null handle"
    );
    Ok(())
}
fn register_power() -> anyhow::Result<Notification> {
    let params = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(callback),
        Context: ptr::null_mut(),
    };
    let mut registration = ptr::null_mut();
    // DEVICE_NOTIFY_CALLBACK = 2; no window, service, delay lock or veto.
    let code = unsafe {
        PowerRegisterSuspendResumeNotification(
            2,
            (&params as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS)
                .cast_mut()
                .cast(),
            &mut registration,
        )
    };
    validate_registration(code, registration)?;
    Ok(Notification(registration))
}
pub fn watch_power(tx: tokio::sync::mpsc::Sender<super::PowerEvent>) -> anyhow::Result<()> {
    let notification = register_power()?;
    tokio::spawn(async move {
        let _notification = notification;
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            tick.tick().await;
            if tx.is_closed() {
                break;
            }
            let bits = EVENTS.swap(0, Ordering::AcqRel);
            if bits & 1 != 0 {
                let _ = tx.try_send(super::PowerEvent::Suspend);
            }
            if bits & 2 != 0 {
                let _ = tx.try_send(super::PowerEvent::Resume);
            }
        }
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn failed_or_null_notification_registration_is_not_ready() {
        assert!(super::validate_registration(5, std::ptr::null_mut()).is_err());
        assert!(super::validate_registration(0, std::ptr::null_mut()).is_err());
        assert!(
            super::validate_registration(0, std::ptr::dangling_mut::<core::ffi::c_void>()).is_ok()
        );
    }
    #[test]
    #[ignore = "opt-in native notification registration, not sleep effectiveness proof"]
    fn native_notification_registration_is_released() {
        super::register_power().unwrap().close().unwrap();
    }
    #[test]
    #[ignore = "opt-in power request; not sleep effectiveness proof"]
    fn native_request_is_balanced() {
        let mut r = super::Resource::acquire().unwrap();
        r.release().unwrap();
    }
}
