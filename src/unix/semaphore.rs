//! ## Semaphores
//!
//! Wrappers around semaphores for the uses of this library
//!

use crate::error::{get_unix_errno, Error};
use tracing::error;

/// Inter-process mutex made using a Named Semaphore.
///
pub struct MutexSemaphore {
    sem: *mut libc::sem_t,
    name: String,
}

impl MutexSemaphore {
    /// Create a new inter-process mutex via the Semaphore API.
    ///
    /// The name of the mutex allows other processes to connect to it.
    /// If this process is the first to create the mutex, it will give it
    /// the specified initialization value.
    ///
    /// #### Arguments
    /// - `name`: name of the mutex
    /// - `init_locked`: whether or not to initialize the mutex locked
    ///
    /// #### Returns
    /// On success, returns a `MutexSemaphore`. On failure, returns an `Error`.
    ///
    pub fn new(name: &str, init_locked: bool) -> Result<MutexSemaphore, Error> {
        use libc::{c_int, sem_open, sem_t, EEXIST, O_CREAT, O_EXCL, O_RDWR, SEM_FAILED, S_IRWXU};

        // format the name
        let name = name.trim_start_matches("/").trim_end_matches("\0");
        let sem_name = format!("/sem_mutex_{}", name);
        let name = sem_name.as_ptr().cast::<i8>();

        let init_value: c_int = if init_locked { 0 } else { 1 };

        let sem_ptr: *mut sem_t = 'open_sem: {
            unsafe {
                let mut sem_ptr = sem_open(name, O_RDWR | O_CREAT | O_EXCL, S_IRWXU, init_value);

                if sem_ptr == SEM_FAILED {
                    // if the file already exists, we just open it normally
                    if get_unix_errno() == EEXIST {
                        sem_ptr = sem_open(name, O_RDWR);
                        if sem_ptr == SEM_FAILED {
                            error!("failed to create mutex");
                            return Err(Error::sem_error());
                        }
                    } else {
                        error!("failed to create mutex");
                        return Err(Error::sem_error());
                    }
                }

                break 'open_sem sem_ptr;
            }
        };

        return Ok(MutexSemaphore {
            sem: sem_ptr,
            name: sem_name,
        });
    }

    /// Lock the mutex before entering a critical code section.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn lock(&self) -> Result<(), Error> {
        use libc::{c_void, free, malloc, sem_timedwait, timespec};
        use std::time::{SystemTime, UNIX_EPOCH};

        unsafe {
            let duration = malloc(std::mem::size_of::<timespec>()).cast::<timespec>();
            (*duration).tv_sec = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + 5;

            let res = sem_timedwait(self.sem, duration);
            free(duration.cast::<c_void>());

            if res < 0 {
                error!("failed to lock mutex");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }

    /// Unlock the mutex before exiting a critical code section.
    ///     
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn unlock(&self) -> Result<(), Error> {
        use libc::sem_post;

        unsafe {
            let res = sem_post(self.sem);
            if res < 0 {
                error!("failed to unlock mutex");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }

    /// Close the mutex without destroying it.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn close(&self) -> Result<(), Error> {
        use libc::sem_close;

        unsafe {
            let res = sem_close(self.sem);
            if res < 0 {
                error!("failed to close mutex");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }

    /// Destroy the mutex for all other processes.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn unlink(&self) -> Result<(), Error> {
        use libc::sem_unlink;

        let name = self.name.as_ptr().cast::<i8>();

        unsafe {
            let res = sem_unlink(name);
            if res < 0 {
                error!("failed to unlink mutex");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }
}

pub struct CounterSemaphore {
    sem: *mut libc::sem_t,
    name: String,
}

impl CounterSemaphore {
    pub fn new(name: &str, init_value: i32) -> Result<CounterSemaphore, Error> {
        use libc::{c_int, sem_open, sem_t, EEXIST, O_CREAT, O_EXCL, O_RDWR, SEM_FAILED, S_IRWXU};

        // format the name
        let name = name.trim_start_matches("/").trim_end_matches("\0");
        let sem_name = format!("sem_counter_{}", name);
        let name = sem_name.as_ptr().cast::<i8>();

        let sem_ptr: *mut sem_t = 'open_sem: {
            unsafe {
                let mut sem_ptr = sem_open(
                    name,
                    O_RDWR | O_CREAT | O_EXCL,
                    S_IRWXU,
                    init_value as c_int,
                );

                if sem_ptr == SEM_FAILED {
                    // if the file already exists, we just open it normally
                    if get_unix_errno() == EEXIST {
                        sem_ptr = sem_open(name, O_RDWR);
                        if sem_ptr == SEM_FAILED {
                            error!("failed to open counter");
                            return Err(Error::sem_error());
                        }
                    } else {
                        error!("failed to open counter");
                        return Err(Error::sem_error());
                    }
                }

                break 'open_sem sem_ptr;
            }
        };

        return Ok(CounterSemaphore {
            sem: sem_ptr,
            name: sem_name,
        });
    }

    /// Increment the counter by one.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn increment(&self) -> Result<(), Error> {
        use libc::sem_post;

        unsafe {
            let res = sem_post(self.sem);
            if res < 0 {
                error!("failed to increment counter");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }

    /// Decrement the counter by one.
    ///
    /// #### Return
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn decrement(&self) -> Result<(), Error> {
        use libc::{sem_trywait, EAGAIN};

        unsafe {
            let res = sem_trywait(self.sem);
            if res < 0 {
                if res == EAGAIN {
                    return Ok(());
                } else {
                    error!("failed to decrement counter");
                    return Err(Error::sem_error());
                }
            }
        }

        return Ok(());
    }

    pub fn get_value(&self) -> Result<i32, Error> {
        use libc::sem_getvalue;

        let value = unsafe {
            let buf: *mut i32 = &mut 0;

            let res = sem_getvalue(self.sem, buf);
            if res < 0 {
                error!("failed to get counter value");
                return Err(Error::sem_error());
            }

            *buf
        };

        return Ok(value);
    }
    
    /// Close the mutex without destroying it.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn close(&self) -> Result<(), Error> {
        use libc::sem_close;

        unsafe {
            let res = sem_close(self.sem);
            if res < 0 {
                error!("failed to close counter");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }

    /// Destroy the mutex for all other processes.
    ///
    /// #### Returns
    /// On success, returns nothing. On failure, returns an `Error`.
    ///
    pub fn unlink(&self) -> Result<(), Error> {
        use libc::sem_unlink;

        let name = self.name.as_ptr().cast::<i8>();

        unsafe {
            let res = sem_unlink(name);
            if res < 0 {
                error!("failed to unlink counter");
                return Err(Error::sem_error());
            }
        }

        return Ok(());
    }
}
