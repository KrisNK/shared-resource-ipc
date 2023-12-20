//! ## Shared Resource IPC
//!
//! A resource shared across processes. Supports any number of processes.
//!

use error::Error;
use serde::{de::DeserializeOwned, Serialize};

mod unix {
    pub mod semaphore;
    pub mod shared_mem;
    pub mod unix;
}

mod error;

use unix::unix::UnixSharedResource;

trait SharedResourceBackend<T: Serialize + DeserializeOwned> {
    /// Access an immutable reference to the shared resource using a clojure.
    /// The clojure can return a value based on the reference to the resource.
    ///
    /// #### Arguments
    /// - `accessor`: A clojure that accepts a value of type `&T` and returns a value of generic type `R`
    ///
    /// #### Returns
    /// On success, returns the value of generic type `R`. On failure, returns an `Error`.
    ///
    fn access<F: Fn(&T) -> R, R>(&self, accessor: F) -> Result<R, Error>;

    /// Access a mutable reference to the shared resource using a clojure.
    /// The clojure can return a value based on the reference to the resource.
    ///
    /// #### Arguments
    /// - `accessor`: A clojure that accepts a value of type `&mut T` and returns a value of generic type `R`
    ///
    /// #### Returns
    /// On success, returns the value of generic type `R`. On failure, returns an `Error`.
    ///
    fn access_mut<F: Fn(&mut T) -> D, D>(&self, accessor: F) -> Result<D, Error>;
}

pub enum SharedResource<T: Serialize + DeserializeOwned> {
    Unix(UnixSharedResource<T>),
}

impl<T: Serialize + DeserializeOwned> SharedResource<T> {
    pub fn new(name: &str, initial_value: T) -> Result<SharedResource<T>, Error> {
        // determine the OS
        let shared_resource = match std::env::consts::OS {
            "linux" => SharedResource::Unix(UnixSharedResource::<T>::new(name, initial_value)?),
            "macos" => SharedResource::Unix(UnixSharedResource::<T>::new(name, initial_value)?),
            _ => return Err(Error::UnsupportedOS),
        };

        return Ok(shared_resource);
    }

    /// Access an immutable reference to the shared resource using a clojure.
    /// The clojure can return a value based on the reference to the resource.
    ///
    /// #### Arguments
    /// - `accessor`: A clojure that accepts a value of type `&T` and returns a value of generic type `R`
    ///
    /// #### Returns
    /// On success, returns the value of generic type `R`. On failure, returns an `Error`.
    ///
    pub fn access<F: Fn(&T) -> R, R>(&self, accessor: F) -> Result<R, Error> {
        let resource = match self {
            Self::Unix(res) => res,
        };
        resource.access(accessor)
    }

    /// Access a mutable reference to the shared resource using a clojure.
    /// The clojure can return a value based on the reference to the resource.
    ///
    /// #### Arguments
    /// - `accessor`: A clojure that accepts a value of type `&mut T` and returns a value of generic type `R`
    ///
    /// #### Returns
    /// On success, returns the value of generic type `R`. On failure, returns an `Error`.
    ///
    pub fn access_mut<F: Fn(&mut T) -> D, D>(&self, accessor: F) -> Result<D, Error> {
        let resource = match self {
            Self::Unix(res) => res,
        };
        resource.access_mut(accessor)
    }
}
