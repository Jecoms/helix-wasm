//! An in-memory virtual file system.
//!
//! On `wasm32-unknown-unknown` there is no real file system, so document IO
//! (and anything else that would touch `std::fs`) is routed through this
//! module instead. Embedders inject and extract files through the same API
//! (and can layer their own persistence on top of it); paths are normalized
//! with [`crate::path::canonicalize`], so relative paths resolve against
//! [`crate::env::current_working_dir`].
//!
//! The module is compiled on every target so it can be tested on the host,
//! but only wasm32 code paths consult it.

use std::collections::BTreeMap;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static FILES: RwLock<BTreeMap<PathBuf, Vec<u8>>> = RwLock::new(BTreeMap::new());

fn normalize(path: impl AsRef<Path>) -> PathBuf {
    crate::path::canonicalize(path)
}

/// Whether a file exists at `path`.
pub fn exists(path: impl AsRef<Path>) -> bool {
    FILES.read().unwrap().contains_key(&normalize(path))
}

/// The contents of the file at `path`, or `NotFound`.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    FILES
        .read()
        .unwrap()
        .get(&normalize(path))
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such virtual file"))
}

/// An `std::io::Read` over a snapshot of the file at `path`, or `NotFound`.
pub fn reader(path: impl AsRef<Path>) -> io::Result<Cursor<Vec<u8>>> {
    read(path).map(Cursor::new)
}

/// Creates or replaces the file at `path`.
pub fn write(path: impl AsRef<Path>, contents: impl Into<Vec<u8>>) {
    FILES
        .write()
        .unwrap()
        .insert(normalize(path), contents.into());
}

/// Removes the file at `path`, or `NotFound`.
pub fn remove(path: impl AsRef<Path>) -> io::Result<()> {
    FILES
        .write()
        .unwrap()
        .remove(&normalize(path))
        .map(|_| ())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such virtual file"))
}

/// All file paths, sorted.
pub fn list() -> Vec<PathBuf> {
    FILES.read().unwrap().keys().cloned().collect()
}

/// An `std::io::Write` that stages everything written to it in memory and
/// commits the staged content as the file's new contents on `flush`. Saves
/// are therefore atomic: a save abandoned before `flush` (e.g. on an
/// encoding error) leaves the previously stored contents untouched.
pub struct VfsWriter {
    path: PathBuf,
    staged: Vec<u8>,
}

/// A [`VfsWriter`] for the file at `path`.
pub fn writer(path: impl AsRef<Path>) -> VfsWriter {
    VfsWriter {
        path: normalize(path),
        staged: Vec::new(),
    }
}

impl Write for VfsWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.staged.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        FILES
            .write()
            .unwrap()
            .insert(self.path.clone(), self.staged.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The store is a process-wide static and the harness runs tests in
    // parallel, so every test works under its own absolute directory.

    #[test]
    fn write_read_roundtrip() {
        write("/vfs-test-roundtrip/a.txt", "hello".as_bytes());
        assert!(exists("/vfs-test-roundtrip/a.txt"));
        assert_eq!(read("/vfs-test-roundtrip/a.txt").unwrap(), b"hello");
    }

    #[test]
    fn paths_are_normalized() {
        write("/vfs-test-normalize/./x/../a.txt", "n".as_bytes());
        assert!(exists("/vfs-test-normalize/a.txt"));
    }

    #[test]
    fn missing_files_report_not_found() {
        assert!(!exists("/vfs-test-missing/a.txt"));
        let err = read("/vfs-test-missing/a.txt").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        let err = remove("/vfs-test-missing/a.txt").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn writer_commits_on_flush_only() {
        write("/vfs-test-writer/a.txt", "old".as_bytes());

        // Abandoned before flush: the old contents survive.
        let mut w = writer("/vfs-test-writer/a.txt");
        w.write_all(b"discarded").unwrap();
        drop(w);
        assert_eq!(read("/vfs-test-writer/a.txt").unwrap(), b"old");

        let mut w = writer("/vfs-test-writer/a.txt");
        w.write_all(b"new ").unwrap();
        w.write_all(b"contents").unwrap();
        assert_eq!(read("/vfs-test-writer/a.txt").unwrap(), b"old");
        w.flush().unwrap();
        assert_eq!(read("/vfs-test-writer/a.txt").unwrap(), b"new contents");
    }

    #[test]
    fn empty_flush_truncates() {
        write("/vfs-test-truncate/a.txt", "old".as_bytes());
        writer("/vfs-test-truncate/a.txt").flush().unwrap();
        assert_eq!(read("/vfs-test-truncate/a.txt").unwrap(), b"");
    }

    #[test]
    fn list_is_sorted_and_removals_apply() {
        write("/vfs-test-list/b.txt", "b".as_bytes());
        write("/vfs-test-list/a.txt", "a".as_bytes());
        let mine: Vec<_> = list()
            .into_iter()
            .filter(|p| p.starts_with("/vfs-test-list"))
            .collect();
        assert_eq!(
            mine,
            vec![
                PathBuf::from("/vfs-test-list/a.txt"),
                PathBuf::from("/vfs-test-list/b.txt")
            ]
        );

        remove("/vfs-test-list/a.txt").unwrap();
        assert!(!exists("/vfs-test-list/a.txt"));
        assert!(exists("/vfs-test-list/b.txt"));
    }
}
