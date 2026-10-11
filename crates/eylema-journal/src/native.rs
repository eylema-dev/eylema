//! Host-configured macOS path and synchronization callback. Cooperative locks
//! do not protect against hostile processes bypassing filesystem ownership.
use super::Storage;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct NativeStorage {
    file: File,
    parent: File,
    full_sync: fn(&File) -> io::Result<()>,
}
impl NativeStorage {
    pub fn create_new(path: &Path, full_sync: fn(&File) -> io::Result<()>) -> io::Result<Self> {
        Self::open(path, full_sync, true)
    }
    pub fn open_existing(path: &Path, full_sync: fn(&File) -> io::Result<()>) -> io::Result<Self> {
        Self::open(path, full_sync, false)
    }
    fn open(path: &Path, full_sync: fn(&File) -> io::Result<()>, create: bool) -> io::Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(io::ErrorKind::Unsupported.into());
        }
        let parent = File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or(io::ErrorKind::InvalidInput)?,
        )?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(create);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => io::ErrorKind::WouldBlock.into(),
            std::fs::TryLockError::Error(e) => e,
        })?;
        if !file.metadata()?.is_file() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(Self {
            file,
            parent,
            full_sync,
        })
    }
}
impl Read for NativeStorage {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.file.read(b)
    }
}
impl Write for NativeStorage {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.file.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl Seek for NativeStorage {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}
impl Storage for NativeStorage {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }
    fn synchronize(&mut self) -> io::Result<()> {
        self.file.sync_all()?;
        self.parent.sync_all()?;
        (self.full_sync)(&self.file)
    }
}
