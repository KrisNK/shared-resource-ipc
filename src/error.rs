//! ### Shared Resource Error
//! 

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("[semaphore error] [errno {0}] {1}")]
    SemaphoreError(i32, String),
    #[error("[shared memory error] [errno {0}] {1}")]
    SharedMemoryError(i32, String),
    #[error("[bincode error]")]
    BincodeError(#[from] bincode::Error),
    #[error("unsupported operating system")]
    UnsupportedOS,
}

impl Error {
    pub fn sem_error() -> Error {
        let (errno, message) = get_unix_error();

        return Error::SemaphoreError(errno, message);
    }

    pub fn shm_error() -> Error {
        let (errno, message) = get_unix_error();

        return Error::SharedMemoryError(errno, message);
    }
}

pub fn get_unix_errno() -> i32 {
    use libc::__errno_location;

    return unsafe { *__errno_location().clone() };
}

pub fn get_unix_error() -> (i32, String) {
    use libc::{__errno_location, strerror};
    use std::ffi::CStr;

    let (errno, message) = unsafe {
        let errno = *__errno_location();
        let message = strerror(errno.clone());
        let message = CStr::from_ptr(message).to_string_lossy().to_string();
        (errno,message)
    };

    return (errno, message);
}