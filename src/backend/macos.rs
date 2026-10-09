use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: CFStringRef,
        level: u32,
        name: CFStringRef,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}
/// An assertion belongs to this process, not to the popup or an external helper.
pub struct Resource {
    id: Option<u32>,
}
impl Resource {
    pub fn acquire() -> anyhow::Result<Self> {
        let kind = CFString::new("PreventUserIdleSystemSleep");
        let reason = CFString::new(super::REASON);
        let mut id = 0;
        // SAFETY: the strings are owned through the synchronous call; the out pointer is valid.
        let result = unsafe {
            IOPMAssertionCreateWithName(
                kind.as_concrete_TypeRef(),
                1,
                reason.as_concrete_TypeRef(),
                &mut id,
            )
        };
        anyhow::ensure!(result == 0, "IOKit assertion creation failed ({result})");
        Ok(Self { id: Some(id) })
    }
    pub fn state(&self) -> &'static str {
        if self.id.is_some() {
            "accepted"
        } else {
            "none"
        }
    }
    pub fn release(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self.id {
            // SAFETY: balanced release of our own successful creation only.
            anyhow::ensure!(
                unsafe { IOPMAssertionRelease(id) } == 0,
                "IOKit assertion release failed"
            );
            self.id = None;
        }
        Ok(())
    }
}
impl Drop for Resource {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            unsafe {
                IOPMAssertionRelease(id);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "opt-in native assertion; not a sleep effectiveness test"]
    fn native_owned_assertion() {
        let mut r = super::Resource::acquire().unwrap();
        r.release().unwrap();
    }
}
