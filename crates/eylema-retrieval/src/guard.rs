use crate::{CancelToken, RetrievalError, wire};
use eylema_journal::Storage;
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
    time::Instant,
};
pub(crate) struct SnapshotGuard<T> {
    storage: T,
    phase: u8,
    remaining: usize,
    buffer: Vec<u8>,
    records: usize,
    deadline: Instant,
    token: CancelToken,
    error: Arc<Mutex<Option<RetrievalError>>>,
}
impl<T> SnapshotGuard<T> {
    pub(crate) fn new(storage: T, deadline: Instant, token: CancelToken) -> Self {
        Self {
            storage,
            phase: 0,
            remaining: 8,
            buffer: Vec::new(),
            records: 0,
            deadline,
            token,
            error: Arc::new(Mutex::new(None)),
        }
    }
    pub(crate) fn error(&self) -> Arc<Mutex<Option<RetrievalError>>> {
        self.error.clone()
    }
    fn fail(&self, e: RetrievalError) -> io::Error {
        *self.error.lock().expect("guard error lock") = Some(e);
        io::ErrorKind::InvalidData.into()
    }
}
impl<T: Storage> Read for SnapshotGuard<T> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.token.cancelled() {
            return Err(self.fail(RetrievalError::Cancelled));
        }
        if Instant::now() >= self.deadline {
            return Err(self.fail(RetrievalError::Deadline));
        }
        if b.is_empty() {
            return Ok(0);
        }
        let n = b.len().min(self.remaining);
        let n = self.storage.read(&mut b[..n])?;
        if n > self.remaining {
            return Err(self.fail(RetrievalError::CorruptSnapshot));
        }
        self.buffer.extend_from_slice(&b[..n]);
        self.remaining -= n;
        if self.remaining == 0 {
            match self.phase {
                0 => {
                    if self.buffer != eylema_journal::HEADER {
                        return Err(self.fail(RetrievalError::CorruptSnapshot));
                    }
                    self.phase = 1;
                    self.remaining = 76;
                }
                1 => {
                    self.records += 1;
                    if self.records > 4 {
                        return Err(self.fail(RetrievalError::CorruptSnapshot));
                    }
                    let len =
                        u32::from_be_bytes(self.buffer[8..12].try_into().expect("frame header"))
                            as usize;
                    if !(8..=wire::MAX_SNAPSHOT).contains(&len) {
                        return Err(self.fail(RetrievalError::CorruptSnapshot));
                    }
                    self.phase = 2;
                    self.remaining = len;
                }
                _ => {
                    if wire::preflight(&self.buffer).is_err() {
                        return Err(self.fail(RetrievalError::CorruptSnapshot));
                    }
                    self.phase = 1;
                    self.remaining = 76;
                }
            }
            self.buffer.clear();
        }
        Ok(n)
    }
}
impl<T: Storage> Write for SnapshotGuard<T> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.storage.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.storage.flush()
    }
}
impl<T: Storage> Seek for SnapshotGuard<T> {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        let at = self.storage.seek(p)?;
        if at == 0 {
            self.phase = 0;
            self.remaining = 8;
            self.records = 0;
            self.buffer.clear();
        }
        Ok(at)
    }
}
impl<T: Storage> Storage for SnapshotGuard<T> {
    fn byte_len(&mut self) -> io::Result<u64> {
        self.storage.byte_len()
    }
    fn synchronize(&mut self) -> io::Result<()> {
        self.storage.synchronize()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    struct Partial(Cursor<Vec<u8>>, bool);
    impl Read for Partial {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.1 = !self.1;
            if self.1 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let n = b.len().min(3);
            self.0.read(&mut b[..n])
        }
    }
    impl Write for Partial {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Seek for Partial {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.0.seek(p)
        }
    }
    impl Storage for Partial {
        fn byte_len(&mut self) -> io::Result<u64> {
            Ok(self.0.get_ref().len() as u64)
        }
        fn synchronize(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn partial_interrupted_frames_reject_before_decode() {
        let bad =
            eylema_contracts::codec::encode(&eylema_contracts::codec::Value::Map(vec![])).unwrap();
        let mut b = eylema_journal::HEADER.to_vec();
        let mut frame = [0; 76];
        frame[8..12].copy_from_slice(&(bad.len() as u32).to_be_bytes());
        b.extend_from_slice(&frame);
        b.extend_from_slice(&bad);
        let mut g = SnapshotGuard::new(
            Partial(Cursor::new(b), false),
            Instant::now() + std::time::Duration::from_secs(1),
            CancelToken::new(),
        );
        let mut n = 0;
        let mut buf = [0; 128];
        loop {
            match g.read(&mut buf) {
                Ok(0) => panic!("unexpected EOF"),
                Ok(k) => n += k,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        assert!(n < 8 + 76 + bad.len());
        assert_eq!(
            *g.error().lock().unwrap(),
            Some(RetrievalError::CorruptSnapshot)
        );
    }
}
