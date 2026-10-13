//! Owned subprocess group for trusted native provider tools, not arbitrary PIDs.
use std::{
    io,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus},
};

pub struct OwnedGroup {
    child: Option<Child>,
}

impl OwnedGroup {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            let mut child = command.spawn()?;
            if i32::try_from(child.id()).is_err() || child.id() == 0 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::ErrorKind::InvalidInput.into());
            }
            Ok(Self { child: Some(child) })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = command;
            Err(io::ErrorKind::Unsupported.into())
        }
    }
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.as_mut()?.stdin.take()
    }
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.as_mut()?.stdout.take()
    }
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.as_mut()?.stderr.take()
    }
    /// Observe exit without reaping, then stop lingering group members before
    /// releasing the leader's PID. No caller can wait/reap the inner Child.
    pub fn poll(&mut self) -> io::Result<Option<ExitStatus>> {
        let Some(child) = &self.child else {
            return Ok(None);
        };
        if native::exited(child.id())? {
            self.stop()
        } else {
            Ok(None)
        }
    }
    /// Only the still-owned, unreaped leader's group can receive SIGKILL.
    /// Local reaping does not prove a remote provider request was cancelled.
    pub fn stop(&mut self) -> io::Result<Option<ExitStatus>> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };
        native::stop(child.id())?;
        let status = child.wait()?;
        self.child = None;
        Ok(Some(status))
    }
}
impl Drop for OwnedGroup {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(target_os = "macos")]
mod native {
    use std::{
        ffi::{c_int, c_void},
        io,
    };
    // macOS27 SDK sys/signal.h siginfo_t: six 32-bit fields, pointer/value/band
    // and seven reserved machine words. Only si_pid is observed, no pointers.
    #[repr(C)]
    #[derive(Default)]
    struct SigInfo {
        signo: c_int,
        errno: c_int,
        code: c_int,
        pid: c_int,
        uid: u32,
        status: c_int,
        address: usize,
        value: usize,
        band: i64,
        reserved: [usize; 7],
    }
    #[link(name = "System")]
    unsafe extern "C" {
        fn waitid(kind: c_int, id: u32, info: *mut SigInfo, options: c_int) -> c_int;
        fn kill(pid: c_int, signal: c_int) -> c_int;
    }
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_listpids(kind: u32, group: u32, buffer: *mut c_void, size: c_int) -> c_int;
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_void,
            size: c_int,
        ) -> c_int;
    }
    #[link(name = "System")]
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    // SDK proc_bsdshortinfo: fixed 64-byte native record. No names are exposed.
    #[repr(C)]
    #[derive(Default)]
    struct ShortInfo {
        pid: u32,
        parent: u32,
        group: u32,
        status: u32,
        name: [u8; 16],
        flags: u32,
        uid: u32,
        gid: u32,
        ruid: u32,
        rgid: u32,
        svuid: u32,
        svgid: u32,
        reserved: u32,
    }
    fn inactive_group(id: u32) -> io::Result<bool> {
        let mut members = [0_i32; 128];
        // SAFETY: __error returns this thread's writable errno slot. SDK
        // PROC_PGRP_ONLY=2, buffer contains 128 writable pid_t slots (512 bytes).
        // Every query is restricted to the opaque owner's still-unreaped group.
        unsafe {
            *__error() = 0;
        }
        let bytes = unsafe { proc_listpids(2, id, members.as_mut_ptr().cast(), 512) };
        if !(0..512).contains(&bytes) || bytes % 4 != 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if bytes == 0 {
            let errno = unsafe { *__error() };
            return if errno == 0 {
                Ok(true)
            } else {
                Err(io::Error::from_raw_os_error(errno))
            };
        }
        for &pid in &members[..bytes as usize / 4] {
            if pid <= 0 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut record = ShortInfo::default();
            // SAFETY: SDK PROC_PIDT_SHORTBSDINFO=13 accepts exactly this 64-byte
            // writable record. Listed PIDs are only observed, never signalled.
            let size =
                unsafe { proc_pidinfo(pid, 13, 0, (&mut record as *mut ShortInfo).cast(), 64) };
            if size == 0 && io::Error::last_os_error().raw_os_error() == Some(3) {
                continue;
            }
            if size != 64 || record.pid != pid as u32 || record.group != id {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // SDK SZOMB=5: this member cannot execute, fork or retain a pipe.
            if record.status != 5 {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(super) fn exited(id: u32) -> io::Result<bool> {
        let mut info = SigInfo::default();
        // SAFETY: SDK P_PID=1, id_t=u32; WNOHANG|WEXITED|WNOWAIT=0x25.
        // id belongs to our unreaped Child, and info is valid/writable for the
        // full SDK structure. WNOWAIT retains PID ownership until group cleanup.
        if unsafe { waitid(1, id, &mut info, 0x25) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if info.pid == 0 {
            return Ok(false);
        }
        if u32::try_from(info.pid) != Ok(id) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(true)
    }
    pub(super) fn stop(id: u32) -> io::Result<()> {
        let pid = i32::try_from(id).map_err(|_| io::ErrorKind::InvalidInput)?;
        if pid <= 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // SAFETY: spawn set pgid to this leader's PID. The opaque owner has not
        // reaped it, so that number cannot be reused. Negative pid targets only
        // that group; no external caller supplies PID/signal. SDK SIGKILL=9.
        if unsafe { kill(-pid, 9) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // ESRCH means no member remains; the leader is still reaped by Child.
        if error.raw_os_error() == Some(3) {
            Ok(())
        } else if error.raw_os_error() == Some(1) && inactive_group(id)? {
            // Darwin reports EPERM for a group containing only zombies. Do
            // not waive a permission failure while any live/unknown member
            // remains: bounded native inspection must establish inactivity.
            Ok(())
        } else {
            Err(error)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn sdk_siginfo_layout_matches_native_arm64_abi() {
            assert_eq!(std::mem::size_of::<SigInfo>(), 104);
            assert_eq!(std::mem::align_of::<SigInfo>(), 8);
            assert_eq!(std::mem::offset_of!(SigInfo, pid), 12);
            assert_eq!(std::mem::offset_of!(SigInfo, address), 24);
            assert_eq!(std::mem::size_of::<ShortInfo>(), 64);
            assert_eq!(std::mem::offset_of!(ShortInfo, status), 12);
        }
    }
}
#[cfg(not(target_os = "macos"))]
mod native {
    use std::io;
    pub(super) fn exited(_: u32) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub(super) fn stop(_: u32) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
