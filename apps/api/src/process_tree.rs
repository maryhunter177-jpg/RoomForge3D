//! Own the optimizer process tree so dropping a worker cannot orphan native encoders.
#[cfg(windows)]
pub struct ProcessTree {
    handle: usize,
}
#[cfg(windows)]
impl ProcessTree {
    pub fn attach(pid: u32) -> std::io::Result<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{JobObjects::*, Threading::*},
        };
        // Windows closes the job handle on API exit; kill-on-close includes descendants.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                let error = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(error);
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                let error = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(error);
            }
            let assigned = AssignProcessToJobObject(job, process);
            let error = std::io::Error::last_os_error();
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err(error);
            }
            Ok(Self {
                handle: job as usize,
            })
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(
                self.handle as windows_sys::Win32::Foundation::HANDLE,
            );
        }
    }
}
#[cfg(unix)]
pub struct ProcessTree {
    pid: i32,
}
#[cfg(unix)]
impl ProcessTree {
    pub fn attach(pid: u32) -> std::io::Result<Self> {
        Ok(Self {
            pid: i32::try_from(pid).map_err(std::io::Error::other)?,
        })
    }
}
#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        // The command starts its own process group; negative PID targets that group only.
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn dropping_job_terminates_owned_process() {
        let mut child = tokio::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .kill_on_drop(true)
            .spawn()
            .expect("spawn isolated test process");
        let job = ProcessTree::attach(child.id().expect("process id")).expect("attach Windows job");
        assert!(child.try_wait().expect("poll owned process").is_none());
        drop(job);
        let _status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .expect("job closes promptly")
            .expect("wait process");
        // Windows kill-on-close may report exit code zero; prompt termination is the guarantee.
    }
}
