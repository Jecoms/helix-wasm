//! An in-memory virtual file system.
//!
//! On `wasm32-unknown-unknown` there is no real file system, so document IO
//! (and anything else that would touch `std::fs`) is routed through this
//! module instead. Embedders inject and extract files through the same API
//! (and can layer their own persistence on top of it); paths are normalized
//! with [`crate::path::canonicalize`], so relative paths resolve against
//! [`crate::env::current_working_dir`].
//!
//! The module is gated to wasm32 and `test`: only wasm32 code paths consult
//! it, and the `test` arm exists solely so the unit tests below run on the
//! host. Native builds do not carry it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static FILES: RwLock<BTreeMap<PathBuf, Vec<u8>>> = RwLock::new(BTreeMap::new());

fn normalize(path: impl AsRef<Path>) -> PathBuf {
    crate::path::canonicalize(path)
}

/// The normalized store key for `path`, or `InvalidInput` if `path` names no
/// file: `""`, `"."`, and `"/"` all normalize to a bare directory-like path
/// (the cwd or the root), and such keys would be undeletable "files" that
/// crash directory-shaped consumers (e.g. the file picker's path column
/// expects every key to have a file name). Best-effort — a key that only
/// *later* becomes the cwd (via `:cd`) cannot be caught here.
///
/// Mirrors [`crate::path::canonicalize`] but snapshots the cwd once, so the
/// "normalizes to the cwd" comparison cannot race a concurrent cwd change
/// (the test harness runs cwd-mutating tests in parallel).
fn validated(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = crate::path::expand_tilde(path.as_ref());
    let cwd = crate::env::current_working_dir();
    let key = if path.is_relative() {
        crate::path::normalize(cwd.join(&path))
    } else {
        crate::path::normalize(&path)
    };
    if key.file_name().is_none() || key == crate::path::normalize(&cwd) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path names no virtual file",
        ));
    }
    Ok(key)
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

/// Creates or replaces the file at `path`. Fails with `InvalidInput` if
/// `path` names no file (e.g. `""`, `"."`, or `"/"`).
pub fn write(path: impl AsRef<Path>, contents: impl Into<Vec<u8>>) -> io::Result<()> {
    let key = validated(path)?;
    FILES.write().unwrap().insert(key, contents.into());
    Ok(())
}

/// Moves the file at `from` to `to`, replacing any file already there.
///
/// Mirrors [`std::fs::rename`], which is what the native code paths this
/// stands in for call: the source key is dropped, an existing target key is
/// overwritten, and a rename onto the same key is a no-op that keeps the
/// contents. Fails with `NotFound` if `from` names no file and with
/// `InvalidInput` if `to` names no file — the target is validated *before*
/// the source is removed, so a rejected rename cannot lose the contents.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let to = validated(to)?;
    let from = normalize(from);
    let mut files = FILES.write().unwrap();
    let contents = files
        .remove(&from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such virtual file"))?;
    files.insert(to, contents);
    Ok(())
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

/// The immediate children of the directory at `path`, sorted by name, as
/// `(name, is_dir)` pairs.
///
/// The store is a flat key namespace, so a directory is never an entry of
/// its own here: it is any path that some key extends. A child is therefore
/// a file when a key ends at it and a directory when keys continue past it,
/// and a name that is both — a key that is also the prefix of another key,
/// e.g. `/dir` beside `/dir/inner` — is reported as a directory, that being
/// the only one of the two a caller can descend into. A real file system
/// cannot produce that case; this one can, because a key with separators in
/// it is just a name.
///
/// A `path` no key lives under has no children, which is also all an empty
/// directory would have: the two are indistinguishable, and neither is an
/// error.
pub fn read_dir(path: impl AsRef<Path>) -> Vec<(OsString, bool)> {
    let dir = normalize(path);
    let mut entries: BTreeMap<OsString, bool> = BTreeMap::new();
    for key in FILES.read().unwrap().keys() {
        let Ok(rest) = key.strip_prefix(&dir) else {
            continue;
        };
        let mut components = rest.components();
        let Some(name) = components.next() else {
            continue; // `key` is the directory itself.
        };
        *entries
            .entry(name.as_os_str().to_os_string())
            .or_insert(false) |= components.next().is_some();
    }
    entries.into_iter().collect()
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
        // Kept as given: `flush` normalizes and validates in one step.
        path: path.as_ref().to_path_buf(),
        staged: Vec::new(),
    }
}

impl Write for VfsWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.staged.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Normalization and validation both happen here rather than in
        // `writer` so an invalid path (e.g. `:w /`) surfaces as an ordinary
        // IO error on the save path.
        let key = validated(&self.path)?;
        FILES.write().unwrap().insert(key, self.staged.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The store is a process-wide static and the harness runs tests in
    // parallel, so every test works under its own absolute directory.
    //
    // Store keys are normalized with `path::canonicalize`, which on Windows
    // hosts prefixes rooted-but-driveless paths like `/vfs-test/a.txt` with
    // the cwd's drive (`C:\vfs-test\a.txt`). Key comparisons therefore go
    // through `normalize` too, never against literal `/`-rooted `PathBuf`s.

    #[test]
    fn write_read_roundtrip() {
        write("/vfs-test-roundtrip/a.txt", "hello".as_bytes()).unwrap();
        assert!(exists("/vfs-test-roundtrip/a.txt"));
        assert_eq!(read("/vfs-test-roundtrip/a.txt").unwrap(), b"hello");
    }

    #[test]
    fn paths_are_normalized() {
        write("/vfs-test-normalize/./x/../a.txt", "n".as_bytes()).unwrap();
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
    fn directory_like_keys_are_rejected() {
        // "/" has no file name; "" and "." normalize to the cwd. Any of
        // these as a store key would crash directory-shaped consumers
        // (e.g. the file picker's path column).
        for path in ["", ".", "/"] {
            let err = write(path, "x".as_bytes()).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "write({path:?})");
            let err = writer(path).flush().unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "flush({path:?})");
        }
        assert!(!exists("/"));
    }

    #[test]
    fn writer_commits_on_flush_only() {
        write("/vfs-test-writer/a.txt", "old".as_bytes()).unwrap();

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
        write("/vfs-test-truncate/a.txt", "old".as_bytes()).unwrap();
        writer("/vfs-test-truncate/a.txt").flush().unwrap();
        assert_eq!(read("/vfs-test-truncate/a.txt").unwrap(), b"");
    }

    #[test]
    fn rename_moves_contents_and_drops_the_source() {
        write("/vfs-test-rename/a.txt", "contents".as_bytes()).unwrap();
        rename("/vfs-test-rename/a.txt", "/vfs-test-rename/b.txt").unwrap();
        assert!(!exists("/vfs-test-rename/a.txt"));
        assert_eq!(read("/vfs-test-rename/b.txt").unwrap(), b"contents");
    }

    #[test]
    fn rename_replaces_an_existing_target() {
        // What `fs::rename` does on unix, so `:move` behaves the same here.
        write("/vfs-test-rename-over/a.txt", "new".as_bytes()).unwrap();
        write("/vfs-test-rename-over/b.txt", "old".as_bytes()).unwrap();
        rename("/vfs-test-rename-over/a.txt", "/vfs-test-rename-over/b.txt").unwrap();
        assert!(!exists("/vfs-test-rename-over/a.txt"));
        assert_eq!(read("/vfs-test-rename-over/b.txt").unwrap(), b"new");
    }

    #[test]
    fn rename_onto_itself_keeps_the_file() {
        write("/vfs-test-rename-self/a.txt", "kept".as_bytes()).unwrap();
        rename("/vfs-test-rename-self/a.txt", "/vfs-test-rename-self/./a.txt").unwrap();
        assert_eq!(read("/vfs-test-rename-self/a.txt").unwrap(), b"kept");
    }

    #[test]
    fn rename_rejects_a_bad_target_without_touching_the_source() {
        write("/vfs-test-rename-bad/a.txt", "kept".as_bytes()).unwrap();
        for path in ["", ".", "/"] {
            let err = rename("/vfs-test-rename-bad/a.txt", path).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "rename(_, {path:?})");
        }
        assert_eq!(read("/vfs-test-rename-bad/a.txt").unwrap(), b"kept");
    }

    #[test]
    fn rename_of_a_missing_source_reports_not_found() {
        let err = rename("/vfs-test-rename-missing/a.txt", "/vfs-test-rename-missing/b.txt")
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(!exists("/vfs-test-rename-missing/b.txt"));
    }

    #[test]
    fn list_is_sorted_and_removals_apply() {
        write("/vfs-test-list/b.txt", "b".as_bytes()).unwrap();
        write("/vfs-test-list/a.txt", "a".as_bytes()).unwrap();
        let mine: Vec<_> = list()
            .into_iter()
            .filter(|p| p.starts_with(normalize("/vfs-test-list")))
            .collect();
        assert_eq!(
            mine,
            vec![
                normalize("/vfs-test-list/a.txt"),
                normalize("/vfs-test-list/b.txt")
            ]
        );

        remove("/vfs-test-list/a.txt").unwrap();
        assert!(!exists("/vfs-test-list/a.txt"));
        assert!(exists("/vfs-test-list/b.txt"));
    }

    #[test]
    fn read_dir_lists_immediate_children_only() {
        write("/vfs-test-readdir/a.txt", "a".as_bytes()).unwrap();
        write("/vfs-test-readdir/sub/b.txt", "b".as_bytes()).unwrap();
        write("/vfs-test-readdir/sub/deeper/c.txt", "c".as_bytes()).unwrap();

        // `sub` is a directory because keys continue past it, and `deeper`
        // does not surface until you descend.
        assert_eq!(
            read_dir("/vfs-test-readdir"),
            vec![("a.txt".into(), false), ("sub".into(), true)]
        );
        assert_eq!(
            read_dir("/vfs-test-readdir/sub"),
            vec![("b.txt".into(), false), ("deeper".into(), true)]
        );
        assert_eq!(
            read_dir("/vfs-test-readdir/sub/deeper"),
            vec![("c.txt".into(), false)]
        );
    }

    #[test]
    fn read_dir_of_a_path_no_key_lives_under_is_empty() {
        write("/vfs-test-readdir-empty/a.txt", "a".as_bytes()).unwrap();
        assert!(read_dir("/vfs-test-readdir-empty/nope").is_empty());
        // Not a prefix match: `/vfs-test-readdir-e` is a different directory.
        assert!(read_dir("/vfs-test-readdir-e").is_empty());
    }

    #[test]
    fn read_dir_reports_a_key_that_is_also_a_prefix_as_a_directory() {
        write("/vfs-test-readdir-both/dir", "file".as_bytes()).unwrap();
        write("/vfs-test-readdir-both/dir/inner.txt", "i".as_bytes()).unwrap();
        assert_eq!(
            read_dir("/vfs-test-readdir-both"),
            vec![("dir".into(), true)]
        );
    }
}
