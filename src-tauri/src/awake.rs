//! The computer kept awake while pktito transcribes, and only then. Sleep
//! when idle doesn't count the work a transcription does as activity, so a
//! long recording left to transcribe stopped where the computer slept, or
//! on a Windows laptop that sleeps in Modern Standby went on at a crawl:
//! pinfer measured a run that kept going into it taking about 70 times as
//! long (pinfer's core/SCHED.md, Windows). The display still sleeps, and a
//! closed lid or the power button still puts the computer to sleep: only
//! idle sleep waits for the job.
//!
//! On Windows the job is also kept out of power throttling, which runs the
//! threads of a window minimized or out of sight, and on battery those of
//! a foreground app left without input (Windows 11 24H2), on efficient
//! cores at their most efficient speed: a job started and walked away from.
//! pinfer measured a windowless process at 1.2 tokens a second left to the
//! system and 30.6 kept out of it (pinfer's core/SCHED.md).

use std::marker::PhantomData;

/// While this lives the computer doesn't sleep when left idle. It is let go
/// on the thread that took it, since Windows keeps the request per thread.
pub struct Awake {
    _held: os::Hold,
    _here: PhantomData<*const ()>,
}

/// Keep the computer awake, and on Windows the job out of power throttling,
/// until the result is dropped. `why` is the reason where the system shows
/// one: macOS's `pmset -g assertions`, Linux's `systemd-inhibit --list`.
/// Where the system can't be asked, nothing happens and the job goes on.
pub fn awake(why: &str) -> Awake {
    Awake {
        _held: os::hold(why),
        _here: PhantomData,
    }
}

#[cfg(target_os = "windows")]
mod os {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
        fn GetCurrentProcess() -> *mut c_void;
        fn SetProcessInformation(
            process: *mut c_void,
            class: i32,
            info: *const c_void,
            size: u32,
        ) -> i32;
    }
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const PROCESS_POWER_THROTTLING: i32 = 4; // ProcessPowerThrottling
    const EXECUTION_SPEED: u32 = 0x1; // PROCESS_POWER_THROTTLING_EXECUTION_SPEED

    /// PROCESS_POWER_THROTTLING_STATE
    #[repr(C)]
    struct Throttling {
        version: u32,
        control: u32,
        state: u32,
    }

    /// Execution speed's throttling for the whole process, the engine's
    /// threads with the rest: taken in hand and off (EXECUTION_SPEED), or
    /// left to the system again (0).
    fn throttling(control: u32) {
        let t = Throttling {
            version: 1,
            control,
            state: 0,
        };
        unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                PROCESS_POWER_THROTTLING,
                &t as *const Throttling as *const c_void,
                std::mem::size_of::<Throttling>() as u32,
            )
        };
    }

    /// The system required, not the display, and the process unthrottled.
    pub struct Hold;

    pub fn hold(_why: &str) -> Hold {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        throttling(EXECUTION_SPEED);
        Hold
    }

    impl Drop for Hold {
        fn drop(&mut self) {
            throttling(0);
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        }
    }
}

#[cfg(target_os = "macos")]
mod os {
    use std::ffi::{c_char, c_void, CString};

    type CFStringRef = *const c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            s: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }
    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }
    const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
    const ON: u32 = 255; // kIOPMAssertionLevelOn

    /// A power assertion, kIOPMAssertionTypePreventUserIdleSystemSleep;
    /// None where it couldn't be made.
    pub struct Hold(Option<u32>);

    pub fn hold(why: &str) -> Hold {
        let cf = |s: &str| {
            let c = CString::new(s).unwrap_or_default();
            unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8) }
        };
        let (kind, name) = (cf("PreventUserIdleSystemSleep"), cf(why));
        let mut id = 0;
        let made = !kind.is_null()
            && !name.is_null()
            && unsafe { IOPMAssertionCreateWithName(kind, ON, name, &mut id) } == 0;
        for s in [kind, name] {
            if !s.is_null() {
                unsafe { CFRelease(s) };
            }
        }
        Hold(made.then_some(id))
    }

    impl Drop for Hold {
        fn drop(&mut self) {
            if let Some(id) = self.0 {
                unsafe { IOPMAssertionRelease(id) };
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod os {
    use std::process::{Child, Command, Stdio};

    /// logind's lock on sleep, held by systemd-inhibit for as long as its
    /// cat reads the pipe from here. The pipe closes when the job ends, or
    /// when pktito dies, so that a crash can't leave the computer unable to
    /// sleep. None without systemd.
    pub struct Hold(Option<Child>);

    pub fn hold(why: &str) -> Hold {
        let child = Command::new("systemd-inhibit")
            .args([
                "--what=sleep",
                "--who=pktito",
                &format!("--why={why}"),
                "--mode=block",
                "cat",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        Hold(child)
    }

    impl Drop for Hold {
        fn drop(&mut self) {
            if let Some(mut c) = self.0.take() {
                drop(c.stdin.take());
                let _ = c.wait();
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod os {
    pub struct Hold;

    pub fn hold(_why: &str) -> Hold {
        Hold
    }
}
