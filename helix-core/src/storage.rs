//! Browser `localStorage`-backed stand-ins for `std::fs`-style IO on wasm32,
//! where no file system exists.
//!
//! Placement note: `docs/architecture.md` describes helix-core as functional
//! editing primitives, and this module is stateful browser IO — a deliberate
//! exception. It lives here because helix-core is the lowest common dependency
//! of its consumers (helix-view's document IO and helix-term's config
//! loading); a dedicated imperative-shell home can be revisited alongside the
//! end-to-end wasm32 work (issue #14).
//!
//! The chunk-buffering logic is split from the `web_sys` backend behind
//! [`StorageBackend`] so it can be unit-tested on the host: the module is
//! compiled with `cfg(any(target_arch = "wasm32", test))`, while everything
//! that actually touches the browser is `cfg(target_arch = "wasm32")`.

use std::io::{ErrorKind, Read, Result, Write};

use log::debug;

/// The key/value store [`StorageWriter`] commits to. On wasm32 the only
/// implementation is browser `localStorage`; tests use an in-memory map.
pub trait StorageBackend {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&mut self, key: &str, value: &str) -> Result<()>;
}

impl<B: StorageBackend + ?Sized> StorageBackend for &mut B {
    fn get(&self, key: &str) -> Result<Option<String>> {
        (**self).get(key)
    }

    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        (**self).set(key, value)
    }
}

/// Reader over a snapshot of the stored content, loaded at [`open`] time.
pub struct StorageReader {
    content: Vec<u8>,
    read_pos: usize,
}

impl StorageReader {
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            read_pos: 0,
        }
    }
}

impl Read for StorageReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        debug!(
            "storage, read request for max. {} bytes; already read: {}, total {}",
            buf.len(),
            self.read_pos,
            self.content.len()
        );
        let remaining = self.content.len() - self.read_pos;
        let n = remaining.min(buf.len());
        buf[..n].copy_from_slice(&self.content[self.read_pos..self.read_pos + n]);
        self.read_pos += n;
        Ok(n)
    }
}

/// Writer that stages every chunk in memory and commits the complete content
/// with a single backend `set` on `flush`. This makes saves atomic — a failed
/// save (e.g. non-UTF-8 content) leaves the previously stored copy untouched —
/// and makes empty saves work: `write_all(&[])` never calls `write`, but
/// `flush` still commits the (empty) staged content, truncating the entry.
///
/// An intermediate `flush` mid-save commits the content staged so far; the
/// final `flush` always commits the complete content.
pub struct StorageWriter<B: StorageBackend> {
    backend: B,
    id: String,
    staged: Vec<u8>,
}

impl<B: StorageBackend> StorageWriter<B> {
    pub fn new(backend: B, id: impl Into<String>) -> Self {
        Self {
            backend,
            id: id.into(),
            staged: Vec::new(),
        }
    }
}

impl<B: StorageBackend> Write for StorageWriter<B> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        debug!("storage, staging {} bytes", buf.len());
        self.staged.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        match std::str::from_utf8(&self.staged) {
            Ok(content) => {
                debug!("storage, storing {} bytes", content.len());
                self.backend.set(&self.id, content)
            }
            Err(_) => {
                debug!("storage only supports writing UTF-8 content");
                Err(ErrorKind::InvalidData.into())
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::path::{Path, PathBuf};

    use super::*;

    /// Browser `localStorage` backend.
    pub struct LocalStorage(web_sys::Storage);

    impl LocalStorage {
        fn new() -> Result<Self> {
            // In windowless contexts (e.g. web workers) there is no window and
            // no localStorage; report Unsupported instead of aborting.
            let window = web_sys::window().ok_or_else(|| {
                debug!("no window available (windowless context?)");
                std::io::Error::from(ErrorKind::Unsupported)
            })?;
            match window.local_storage() {
                Ok(Some(storage)) => Ok(Self(storage)),
                Ok(None) => {
                    debug!("no storage available");
                    Err(ErrorKind::Unsupported.into())
                }
                Err(e) => {
                    debug!("error accessing storage: {:?}", e);
                    Err(ErrorKind::Other.into())
                }
            }
        }
    }

    impl StorageBackend for LocalStorage {
        fn get(&self, key: &str) -> Result<Option<String>> {
            match self.0.get_item(key) {
                Ok(content) => Ok(content),
                Err(e) => {
                    debug!("error reading from storage {:?}", e);
                    Err(ErrorKind::Other.into())
                }
            }
        }

        fn set(&mut self, key: &str, value: &str) -> Result<()> {
            match self.0.set_item(key, value) {
                Ok(()) => Ok(()),
                Err(e) => {
                    debug!("error writing to storage {:?}", e);
                    Err(ErrorKind::Other.into())
                }
            }
        }
    }

    pub fn exists<P: AsRef<Path>>(path: P) -> bool {
        fn inner(path: &Path) -> bool {
            let Ok(storage) = LocalStorage::new() else {
                return false;
            };
            matches!(storage.get(&path.to_string_lossy()), Ok(Some(_)))
        }
        inner(path.as_ref())
    }

    /// Opens the stored content at `path` for reading. Missing entries read as
    /// empty, mirroring the previous behavior for brand-new files.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<StorageReader> {
        fn inner(path: &Path) -> Result<StorageReader> {
            let storage = LocalStorage::new()?;
            let content = storage.get(&path.to_string_lossy())?.unwrap_or_else(|| {
                debug!("content not found in storage");
                String::new()
            });
            Ok(StorageReader::new(content.into_bytes()))
        }
        inner(path.as_ref())
    }

    /// Creates a writer for `path`. Content is staged in memory and committed
    /// on `flush`, replacing whatever was stored before.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<StorageWriter<LocalStorage>> {
        fn inner(path: &Path) -> Result<StorageWriter<LocalStorage>> {
            let storage = LocalStorage::new()?;
            Ok(StorageWriter::new(storage, path.to_string_lossy()))
        }
        inner(path.as_ref())
    }

    pub fn read_to_string(path: PathBuf) -> Result<String> {
        let storage = LocalStorage::new()?;
        match storage.get(&path.to_string_lossy())? {
            Some(content) => Ok(content),
            None => {
                debug!("nothing found in storage for path {}", path.display());
                Err(ErrorKind::NotFound.into())
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemBackend {
        map: HashMap<String, String>,
        sets: usize,
    }

    impl StorageBackend for MemBackend {
        fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.map.get(key).cloned())
        }

        fn set(&mut self, key: &str, value: &str) -> Result<()> {
            self.map.insert(key.to_owned(), value.to_owned());
            self.sets += 1;
            Ok(())
        }
    }

    #[test]
    fn reader_yields_content_across_small_chunks() {
        let mut reader = StorageReader::new(b"hello world".to_vec());
        let mut buf = [0u8; 4];
        let mut out = Vec::new();
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        assert_eq!(out, b"hello world");
    }

    #[test]
    fn reader_handles_oversized_buffer() {
        let mut reader = StorageReader::new(b"hi".to_vec());
        let mut buf = [0u8; 16];
        assert_eq!(reader.read(&mut buf).unwrap(), 2);
        assert_eq!(&buf[..2], b"hi");
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn multi_chunk_write_commits_once_on_flush() {
        let mut backend = MemBackend::default();
        let mut writer = StorageWriter::new(&mut backend, "file");
        writer.write_all(b"hello ").unwrap();
        writer.write_all(b"world").unwrap();
        assert_eq!(writer.backend.sets, 0, "no commit before flush");
        writer.flush().unwrap();
        assert_eq!(backend.sets, 1, "exactly one commit per save");
        assert_eq!(backend.map["file"], "hello world");
    }

    #[test]
    fn empty_save_truncates_stored_content() {
        let mut backend = MemBackend::default();
        backend.map.insert("file".into(), "old content".into());
        let mut writer = StorageWriter::new(&mut backend, "file");
        // `write_all(&[])` never calls `write`; only `flush` runs.
        writer.write_all(&[]).unwrap();
        writer.flush().unwrap();
        assert_eq!(backend.map["file"], "");
    }

    #[test]
    fn failed_save_preserves_previous_content() {
        let mut backend = MemBackend::default();
        backend.map.insert("file".into(), "good copy".into());
        let mut writer = StorageWriter::new(&mut backend, "file");
        // First chunk is valid UTF-8 on its own; the second is not (as with
        // non-UTF-8 document encodings). The save must fail as a whole.
        writer.write_all(b"valid prefix").unwrap();
        writer.write_all(&[0xff, 0xfe]).unwrap();
        let err = writer.flush().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(backend.sets, 0, "failed save must not touch the backend");
        assert_eq!(backend.map["file"], "good copy");
    }

    #[test]
    fn utf8_split_across_write_chunks_is_accepted() {
        let mut backend = MemBackend::default();
        let mut writer = StorageWriter::new(&mut backend, "file");
        let bytes = "héllo".as_bytes();
        // Split inside the two-byte 'é' sequence; validation must run over
        // the complete staged content, not per chunk.
        writer.write_all(&bytes[..2]).unwrap();
        writer.write_all(&bytes[2..]).unwrap();
        writer.flush().unwrap();
        assert_eq!(backend.map["file"], "héllo");
    }
}
