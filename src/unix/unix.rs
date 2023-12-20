//! ## Unix Implementation of the Shared Resource
//!

use serde::{de::DeserializeOwned, Serialize};

use crate::error::Error;
use crate::SharedResourceBackend;

use super::semaphore::{CounterSemaphore, MutexSemaphore};
use super::shared_mem::SharedMemory;

pub struct UnixSharedResource<T: Serialize + DeserializeOwned> {
    mutex: MutexSemaphore,
    counter: CounterSemaphore,
    resource: SharedMemory<T>,
}

impl<T: Serialize + DeserializeOwned> UnixSharedResource<T> {
    pub fn new(name: &str, initial_value: T) -> Result<UnixSharedResource<T>, Error> {
        let mutex = MutexSemaphore::new(name, false)?;
        let counter = CounterSemaphore::new(name, 0)?;

        // IMPORTANT THAT THE COUNTER IS INCREMENTED BEFORE EVEN LOCKING THE MUTEX
        counter.increment()?;
        mutex.lock()?;

        // CRITICAL SECTION
        let resource = SharedMemory::new(name, initial_value)?;

        mutex.unlock()?;

        return Ok(UnixSharedResource {
            mutex,
            counter,
            resource,
        });
    }
}

impl<T: Serialize + DeserializeOwned> Drop for UnixSharedResource<T> {
    fn drop(&mut self) {
        self.mutex.lock().expect("failed to lock mutex in drop");
        self.counter
            .decrement()
            .expect("failed to decrement counter in drop");

        // check the value of the counter
        let mut is_final_process: bool = false;
        if self
            .counter
            .get_value()
            .expect("failed to get counter value in drop")
            == 0
        {
            is_final_process = true;
        }

        if is_final_process {
            // FINAL PROCESS... DESTROY EVERYTHING
            tracing::debug!("FINAL {}", std::os::unix::process::parent_id());
            self.counter
                .close()
                .expect("failed to close counter in drop");
            self.counter
                .unlink()
                .expect("failed to unlink counter in drop");
            self.resource
                .close()
                .expect("failed to close shared memory in drop");
            self.resource
                .unlink()
                .expect("failed to unlink shared memory in drop");
            self.mutex.close().expect("failed to close mutex in drop");
            self.mutex.unlink().expect("failed to unlink mutex in drop");
        } else {
            // NOT FINAL, SO JUST CLOSE FOR THIS PROCESS
            tracing::debug!("NOT FINAL {}", std::os::unix::process::parent_id());
            self.counter
                .close()
                .expect("failed to close counter in drop");
            self.resource
                .close()
                .expect("failed to close shared memory in drop");
            self.mutex.unlock().expect("failed to unlock mutex in drop");
            self.mutex.close().expect("failed to close mutex in drop");
        }
    }
}

impl<T: Serialize + DeserializeOwned> SharedResourceBackend<T> for UnixSharedResource<T> {
    fn access<F: Fn(&T) -> R, R>(&self, accessor: F) -> Result<R, Error> {
        self.mutex.lock()?;
        let data: T = self.resource.get()?;
        let res: R = accessor(&data);
        self.mutex.unlock()?;
        return Ok(res);
    }

    fn access_mut<F: Fn(&mut T) -> D, D>(&self, accessor: F) -> Result<D, Error> {
        self.mutex.lock()?;
        let mut data: T = self.resource.get()?;
        let res: D = accessor(&mut data);
        self.resource.set(data)?;
        self.mutex.unlock()?;
        return Ok(res);
    }
}

#[cfg(test)]
mod tests {
    use super::{SharedResourceBackend, UnixSharedResource};
    use rusty_fork::rusty_fork_test;

    fn init() -> String {
        let _ = tracing_subscriber::fmt::try_init();

        // generate a name
        let name: String = format!("test_{}", std::process::id());

        return name;
    }

    fn spawn_children(num_children: usize) {
        use libc::fork;

        let parent_id: u32 = std::process::id();
        let mut num_children: usize = num_children;

        while num_children > 0 {
            unsafe {
                fork();
            }

            if std::process::id() == parent_id {
                num_children -= 1;
            } else {
                break;
            }
        }
    }

    rusty_fork_test! {
        #[test]
        fn test_single_proc_open_close_resource() {
            let name = init();

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            drop(resource);
        }

        #[test]
        fn test_many_proc_open_close_resource() {
            let name = init();

            spawn_children(5);

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            drop(resource);
        }

        #[test]
        fn test_single_proc_read() {
            let name = init();

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            let data = resource
                .access(|data| data.clone())
                .expect("failed to access data");

            drop(resource);

            assert_eq!(data, 1000);
        }

        #[test]
        fn test_many_proc_read() {
            let name = init();
            spawn_children(5);

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            let data = resource
                .access(|data| data.clone())
                .expect("failed to access data");

            drop(resource);

            assert_eq!(data, 1000);
        }

        #[test]
        fn test_single_proc_mutate() {
            let name = init();

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            resource
                .access_mut(|data| { *data = 100; })
                .expect("failed to access mutable data");

            let data = resource
                .access_mut(|data| data.clone())
                .expect("failed to access data");

            drop(resource);

            assert_eq!(data, 100);
        }

        #[test]
        fn test_many_proc_mutate() {
            let name = init();

            let parent_id = std::process::id();

            spawn_children(1);

            let resource =
                UnixSharedResource::<usize>::new(&name, 1000).expect("failed to open resource");

            let val: usize = if std::process::id() == parent_id {
                std::thread::sleep(std::time::Duration::from_millis(10));
                resource
                    .access_mut(|data| data.clone())
                    .expect("failed to access data")
            } else {
                resource
                    .access_mut(|data| { *data = 100; data.clone() })
                    .expect("failed to access mutable data")
            };

            drop(resource);

            assert_eq!(val, 100);
        }


    }
}
