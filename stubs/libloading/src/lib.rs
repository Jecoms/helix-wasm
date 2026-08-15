use std::marker::PhantomData;
use std::ops::Deref;

#[derive(Debug)]
pub struct Error;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dynamic library loading is not supported on wasm")
    }
}
impl std::error::Error for Error {}

#[derive(Debug)]
pub struct Library;
impl Library {
    /// # Safety
    /// Never succeeds on wasm; provided for API compatibility only.
    pub unsafe fn new<P: AsRef<std::ffi::OsStr>>(_filename: P) -> Result<Library, Error> {
        Err(Error)
    }
    /// # Safety
    /// Never reachable on wasm; `Library` cannot be constructed.
    pub unsafe fn get<T>(&self, _symbol: &[u8]) -> Result<Symbol<T>, Error> {
        Err(Error)
    }
}

#[derive(Debug)]
pub struct Symbol<T> {
    _never: std::convert::Infallible,
    _marker: PhantomData<T>,
}
impl<T> Deref for Symbol<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self._never {}
    }
}
