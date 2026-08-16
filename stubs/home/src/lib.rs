use std::path::PathBuf;
pub fn home_dir() -> Option<PathBuf> { Some(PathBuf::from("/home/web")) }
pub fn cargo_home() -> std::io::Result<PathBuf> { Ok(PathBuf::from("/home/web/.cargo")) }
pub fn rustup_home() -> std::io::Result<PathBuf> { Ok(PathBuf::from("/home/web/.rustup")) }
