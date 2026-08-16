use std::ffi::OsStr;
use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error { CannotFindBinaryPath, CannotGetCurrentDirAndPathListEmpty, CannotCanonicalize }
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "cannot find binary path") }
}
impl std::error::Error for Error {}
pub fn which<T: AsRef<OsStr>>(_binary_name: T) -> Result<PathBuf, Error> { Err(Error::CannotFindBinaryPath) }
pub fn which_all<T: AsRef<OsStr>>(_binary_name: T) -> Result<std::vec::IntoIter<PathBuf>, Error> { Ok(vec![].into_iter()) }
