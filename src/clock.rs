//! Suspend-aware elapsed time is the authority; Tokio timers only schedule work.
pub fn now_ms() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let mut t = std::mem::MaybeUninit::<libc::timespec>::uninit();
        // SAFETY: valid out pointer; CLOCK_BOOTTIME includes time asleep.
        let ok = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, t.as_mut_ptr()) };
        assert_eq!(ok, 0, "CLOCK_BOOTTIME unavailable");
        let t = unsafe { t.assume_init() };
        t.tv_sec as u64 * 1000 + t.tv_nsec as u64 / 1_000_000
    }
    #[cfg(target_os = "macos")]
    {
        #[repr(C)]
        struct Timebase {
            numer: u32,
            denom: u32,
        }
        unsafe extern "C" {
            fn mach_continuous_time() -> u64;
            fn mach_timebase_info(info: *mut Timebase) -> i32;
        }
        let mut info = Timebase { numer: 0, denom: 0 };
        // SAFETY: native clock and writable timebase structure, no retained pointer.
        unsafe {
            assert_eq!(mach_timebase_info(&mut info), 0);
            (mach_continuous_time() as u128 * info.numer as u128 / info.denom as u128 / 1_000_000)
                as u64
        }
    }
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
    }
}
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
