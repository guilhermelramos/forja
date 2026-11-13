//! Job Object do Windows: processos atribuídos a ele morrem quando o handle fecha (app encerrado
//! ou travado). Fora do Windows é um no-op.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// Mata os processos quando o handle é fechado.
    pub struct JobObject(HANDLE);

    // SAFETY: o handle de job é um objeto do kernel, válido em qualquer thread. O dono só é
    // alcançado por um `Mutex`, então o handle nunca é usado em duas threads ao mesmo tempo.
    unsafe impl Send for JobObject {}

    impl JobObject {
        pub fn new() -> Option<Self> {
            // SAFETY: atributos e nome nulos são argumentos válidos; o handle é conferido e
            // fechado em todo caminho de falha.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of_val(&info) as u32,
                );
                if ok == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Self(job))
            }
        }

        pub fn assign(&self, process: *mut c_void) -> bool {
            // SAFETY: os dois handles estão vivos: o job pertence a `self`, e o processo ainda não
            // foi esperado.
            unsafe { AssignProcessToJobObject(self.0, process) != 0 }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // SAFETY: o handle foi criado por CreateJobObjectW e é fechado exatamente uma vez.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// Sem Job Object fora do Windows: nada a fazer.
    pub struct JobObject;

    impl JobObject {
        pub fn new() -> Option<Self> {
            Some(Self)
        }

        pub fn assign(&self, _process: *mut std::ffi::c_void) -> bool {
            true
        }
    }
}

pub use imp::JobObject;
