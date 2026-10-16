//! Closed predecode envelope and prepaid, cooperatively bounded physical IO.
use super::{MemoryControl, MemoryResult as R, MemoryStoreError as E};
use eylema_contracts::codec;
use eylema_journal::Storage;
use std::{
    cell::RefCell,
    io::{self, Read, Seek, SeekFrom, Write},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub(super) struct PayloadGuard;
impl PayloadGuard {
    pub(super) fn validate(b: &[u8]) -> R<()> {
        if b.len() > 8192 || b.get(..7) != Some(codec::HEADER.as_slice()) {
            return Err(E::InvalidRequest);
        }
        fn value(b: &[u8], p: &mut usize, n: &mut usize, depth: usize) -> R<()> {
            *n += 1;
            if *n > 256 || depth > 8 {
                return Err(E::Limit);
            }
            let tag = *b.get(*p).ok_or(E::InvalidRequest)?;
            *p += 1;
            let size = match tag {
                0..=2 => 0,
                3 | 4 => 8,
                5..=7 => {
                    let a = b
                        .get(*p..p.checked_add(4).ok_or(E::Limit)?)
                        .ok_or(E::InvalidRequest)?;
                    *p += 4;
                    let len =
                        u32::from_be_bytes(a.try_into().map_err(|_| E::InvalidRequest)?) as usize;
                    if tag == 7 {
                        if len > 32 || len > 256usize.saturating_sub(*n) {
                            return Err(E::Limit);
                        }
                        for _ in 0..len {
                            value(b, p, n, depth + 1)?
                        }
                        return Ok(());
                    }
                    len
                }
                _ => return Err(E::InvalidRequest),
            };
            *p = p.checked_add(size).ok_or(E::Limit)?;
            if *p > b.len() {
                return Err(E::InvalidRequest);
            }
            Ok(())
        }
        let mut p = 7;
        value(b, &mut p, &mut 0, 0)?;
        if p != b.len() {
            return Err(E::InvalidRequest);
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Create,
    Append,
    Replay,
}
struct Permit {
    read_bytes: usize,
    reads: usize,
    write_bytes: usize,
    writes: usize,
    seeks: usize,
    metadata: usize,
    flushes: usize,
    syncs: usize,
    scans: usize,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
}
impl Permit {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::SeqCst) {
            Err(io::ErrorKind::Interrupted.into())
        } else if Instant::now() >= self.deadline {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct Gate(Rc<RefCell<Option<Permit>>>);
impl Gate {
    pub(super) fn begin(&self, mode: Mode, c: &mut MemoryControl) -> R<usize> {
        if self.0.borrow().is_some() {
            return Err(E::NeedsReopen);
        }
        let replay = matches!(mode, Mode::Replay);
        let reserve = if replay { 48 * 1024 * 1024 } else { 262144 };
        c.inner.reserve(reserve)?;
        if let Err(e) = c.inner.charge(if replay { 51_380_224 } else { 101360 }) {
            c.inner.release(reserve);
            return Err(e.into());
        }
        *self.0.borrow_mut() = Some(Permit {
            read_bytes: if replay { 4194304 } else { 0 },
            reads: if replay { 4096 } else { 0 },
            write_bytes: if replay { 0 } else { 8276 },
            writes: if replay { 0 } else { 1024 },
            seeks: if replay { 16 } else { 4 },
            metadata: if replay { 16 } else { 4 },
            flushes: if replay {
                0
            } else if matches!(mode, Mode::Create) {
                2
            } else {
                1
            },
            syncs: if replay {
                0
            } else if matches!(mode, Mode::Create) {
                2
            } else {
                1
            },
            scans: if replay { 2 } else { 0 },
            deadline: c.deadline,
            cancel: c.cancel.flag.clone(),
        });
        Ok(reserve)
    }
    pub(super) fn end(&self, reserve: usize, c: &mut MemoryControl) {
        *self.0.borrow_mut() = None;
        c.inner.release(reserve)
    }
    fn access<U>(&self, f: impl FnOnce(&mut Permit) -> io::Result<U>) -> io::Result<U> {
        let mut gate = self.0.borrow_mut();
        let p = gate
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
        p.check()?;
        f(p)
    }
    fn check(&self) -> io::Result<()> {
        self.access(|p| p.check())
    }
}
fn take(n: &mut usize, v: usize) -> io::Result<()> {
    if *n < v {
        return Err(io::ErrorKind::OutOfMemory.into());
    }
    *n -= v;
    Ok(())
}
pub(super) struct GuardedStorage<T: Storage> {
    inner: T,
    gate: Gate,
    buffer: Vec<u8>,
    offset: usize,
    position: u64,
}
impl<T: Storage> GuardedStorage<T> {
    pub(super) fn new(inner: T, gate: Gate) -> Self {
        Self {
            inner,
            gate,
            buffer: vec![],
            offset: 0,
            position: 0,
        }
    }
    fn physical_read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.gate.access(|p| {
            take(&mut p.reads, 1)?;
            if p.read_bytes == 0 {
                return Err(io::ErrorKind::OutOfMemory.into());
            }
            Ok(())
        })?;
        let size = self.gate.access(|p| Ok(b.len().min(p.read_bytes)))?;
        let n = self.inner.read(&mut b[..size])?;
        if n > size {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.gate.access(|p| take(&mut p.read_bytes, n))?;
        self.gate.check()?;
        Ok(n)
    }
    fn exact(&mut self, b: &mut [u8]) -> io::Result<()> {
        let mut at = 0;
        while at < b.len() {
            let n = self.physical_read(&mut b[at..])?;
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            at += n;
        }
        Ok(())
    }
    fn fill(&mut self) -> io::Result<()> {
        if self.position == 0 {
            let mut header = [0; 8];
            self.exact(&mut header)?;
            self.buffer = header.to_vec();
        } else {
            let mut h = [0; 76];
            self.exact(&mut h)?;
            let len = u32::from_be_bytes(
                h[8..12]
                    .try_into()
                    .map_err(|_| io::ErrorKind::InvalidData)?,
            ) as usize;
            if !(8..=8192).contains(&len) {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut payload = vec![0; len];
            self.exact(&mut payload)?;
            PayloadGuard::validate(&payload).map_err(|_| io::ErrorKind::InvalidData)?;
            self.buffer = Vec::with_capacity(76 + len);
            self.buffer.extend(h);
            self.buffer.extend(payload);
        }
        self.offset = 0;
        Ok(())
    }
}
impl<T: Storage> Read for GuardedStorage<T> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.gate.check()?;
        if b.is_empty() {
            return Ok(0);
        }
        if self.offset == self.buffer.len() {
            self.fill()?
        }
        let n = b.len().min(self.buffer.len() - self.offset);
        b[..n].copy_from_slice(&self.buffer[self.offset..self.offset + n]);
        self.offset += n;
        self.position += n as u64;
        self.gate.check()?;
        Ok(n)
    }
}
impl<T: Storage> Write for GuardedStorage<T> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.gate.access(|p| {
            take(&mut p.writes, 1)?;
            if b.len() > p.write_bytes {
                return Err(io::ErrorKind::OutOfMemory.into());
            }
            Ok(())
        })?;
        let n = self.inner.write(b)?;
        if n > b.len() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.gate.access(|p| take(&mut p.write_bytes, n))?;
        self.position += n as u64;
        self.gate.check()?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.gate.access(|p| take(&mut p.flushes, 1))?;
        self.inner.flush()?;
        self.gate.check()
    }
}
impl<T: Storage> Seek for GuardedStorage<T> {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.gate.access(|permit| {
            take(&mut permit.seeks, 1)?;
            if matches!(p, SeekFrom::Start(0)) && permit.reads > 0 {
                take(&mut permit.scans, 1)?
            }
            Ok(())
        })?;
        let at = self.inner.seek(p)?;
        self.buffer.clear();
        self.offset = 0;
        self.position = at;
        self.gate.check()?;
        Ok(at)
    }
}
impl<T: Storage> Storage for GuardedStorage<T> {
    fn byte_len(&mut self) -> io::Result<u64> {
        self.gate.access(|p| take(&mut p.metadata, 1))?;
        let n = self.inner.byte_len()?;
        self.gate.check()?;
        if n > 2097152 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(n)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        self.gate.access(|p| take(&mut p.syncs, 1))?;
        self.inner.synchronize()?;
        self.gate.check()
    }
}
