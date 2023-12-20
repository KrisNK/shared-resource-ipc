//! ## Shared Memory
//!

use std::marker::PhantomData;

use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use serde::{de::DeserializeOwned, Serialize};
use tracing::error;

use crate::error::{get_unix_errno, Error};

pub struct SharedMemory<T: Serialize + DeserializeOwned> {
    memory: *mut MemoryMeta,
    fd: i32,
    name: String,
    _datatype: PhantomData<T>,
}

struct MemoryMeta {
    size: u64,
    data: *mut u8,
}

impl<T: Serialize + DeserializeOwned> SharedMemory<T> {
    const META_SIZE: usize = std::mem::size_of::<MemoryMeta>();

    pub fn new(name: &str, initial_value: T) -> Result<SharedMemory<T>, Error> {
        use libc::{
            c_int, ftruncate, mmap, shm_open, EEXIST, MAP_FAILED, MAP_SHARED, O_CREAT, O_EXCL,
            O_RDWR, PROT_READ, PROT_WRITE, S_IRWXU,
        };

        // format the name
        let name = name.trim_start_matches("/").trim_end_matches("\0");
        let shm_name = format!("shm_{}", name);
        let name = shm_name.as_ptr().cast::<i8>();

        // open shared memory
        let mut memory_is_new: bool = true;
        let shm_fd: c_int = unsafe {
            let mut shm_fd = shm_open(name, O_RDWR | O_CREAT | O_EXCL, S_IRWXU);

            if shm_fd < 0 {
                // possibly, the memory already exists
                if get_unix_errno() == EEXIST {
                    shm_fd = shm_open(name, O_RDWR, S_IRWXU);
                    if shm_fd < 0 {
                        error!("failed to open existing shared memory");
                        return Err(Error::shm_error());
                    }
                    memory_is_new = false;
                } else {
                    error!("failed to create or open shared memory");
                    return Err(Error::shm_error());
                }
            }

            shm_fd
        };

        if memory_is_new {
            // truncate the memory
            unsafe {
                let res = ftruncate(shm_fd.clone(), Self::META_SIZE as i64);
                if res < 0 {
                    error!("failed to truncate shared memory");
                    return Err(Error::shm_error());
                }
            }
        }

        // Map the memory meta
        let meta_ptr = unsafe {
            let shm_ptr = mmap(
                std::ptr::null_mut(),
                Self::META_SIZE,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                shm_fd.clone(),
                0,
            );
            if shm_ptr == MAP_FAILED {
                error!("failed to map shared memory metadata");
                return Err(Error::shm_error());
            }

            shm_ptr.cast::<MemoryMeta>()
        };

        // set the size of the actual data
        if memory_is_new {
            unsafe {
                (*meta_ptr).size = std::mem::size_of_val(&initial_value) as u64;
            }
        }

        // map the actual data
        let data_ptr = unsafe {
            let shm_ptr = mmap(
                std::ptr::null_mut(),
                (*meta_ptr).size as usize,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                shm_fd.clone(),
                0,
            );
            if shm_ptr == MAP_FAILED {
                error!("failed to map shared memory data");
                return Err(Error::shm_error());
            }

            shm_ptr.cast::<u8>()
        };

        // initialize the data
        if memory_is_new {
            let initial_value = bincode::serialize(&initial_value)?;
            let raw_data = unsafe {
                &mut *std::ptr::slice_from_raw_parts_mut(data_ptr, (*meta_ptr).size as usize)
            };

            raw_data.par_iter_mut().enumerate().for_each(|(i, v)| {
                *v = initial_value[i];
            });
        }

        unsafe {
            (*meta_ptr).data = data_ptr;
        }

        return Ok(SharedMemory {
            memory: meta_ptr,
            name: shm_name,
            fd: shm_fd,
            _datatype: PhantomData::<T>,
        });
    }

    pub fn get(&self) -> Result<T, Error> {
        let bytes = unsafe {
            &*std::ptr::slice_from_raw_parts((*self.memory).data, (*self.memory).size as usize)
        };
        let data = bincode::deserialize::<T>(bytes)?;

        return Ok(data);
    }

    pub fn set(&self, new_data: T) -> Result<(), Error> {
        use libc::{c_void, mremap, MAP_FAILED, MREMAP_MAYMOVE};

        let new_data = bincode::serialize(&new_data)?;
        let new_size: usize = new_data.len();

        // remap if the size is different
        unsafe {
            if (*self.memory).size as usize != new_size {
                let new_data_ptr = mremap(
                    (*self.memory).data.cast::<c_void>(),
                    (*self.memory).size as usize,
                    new_size,
                    MREMAP_MAYMOVE,
                );
                if new_data_ptr == MAP_FAILED {
                    error!("failed to remap shared memory data");
                    return Err(Error::shm_error());
                }

                (*self.memory).data = new_data_ptr.cast::<u8>();
                (*self.memory).size = new_size as u64;
            }
        };

        // set the new data
        unsafe {
            let raw_data = &mut *std::ptr::slice_from_raw_parts_mut((*self.memory).data, new_size);

            raw_data.par_iter_mut().enumerate().for_each(|(i, v)| {
                *v = new_data[i];
            });
        }

        Ok(())
    }

    pub fn close(&self) -> Result<(), Error> {
        use libc::{c_void, close, munmap};

        unsafe {
            // unmap the data
            let res = munmap(
                (*self.memory).data.cast::<c_void>(),
                (*self.memory).size as usize,
            );
            if res < 0 {
                error!("failed to unmap data");
                return Err(Error::shm_error());
            }

            // unmap the memory meta
            let res = munmap(self.memory.cast::<c_void>(), Self::META_SIZE);
            if res < 0 {
                error!("failed to unmap metadata");
                return Err(Error::shm_error());
            }

            let res = close(self.fd);
            if res < 0 {
                error!("failed to close shared memory");
                return Err(Error::shm_error());
            }
        }

        return Ok(());
    }

    pub fn unlink(&self) -> Result<(), Error> {
        use libc::shm_unlink;

        unsafe {
            let res = shm_unlink(self.name.as_ptr().cast::<i8>());
            if res < 0 {
                error!("failed to unlink shared memory");
                return Err(Error::shm_error());
            }
        }

        return Ok(());
    }
}
