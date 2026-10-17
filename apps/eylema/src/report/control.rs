//! Cooperative SDK controls plus an authoritative parent deadline.
use super::types::*;
use eylema_model::cpu::CpuControl;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Default)]
pub struct Controls {
    stopped: Arc<AtomicBool>,
    active: Arc<Mutex<Vec<CpuControl>>>,
}
impl Controls {
    pub fn check(&self) -> Result<()> {
        if self.stopped.load(Ordering::Acquire) {
            Err(ReportError::Stopped)
        } else {
            Ok(())
        }
    }
    pub fn register(&self, c: CpuControl) -> Result<()> {
        let mut controls = self.active.lock().map_err(|_| ReportError::Stopped)?;
        self.check()?;
        if controls.len() >= 32 {
            return Err(ReportError::Limit);
        }
        controls.push(c);
        Ok(())
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(controls) = self.active.lock() {
            for c in controls.iter() {
                c.cancel()
            }
        }
    }
}
use std::{
    io::Read,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
/// Owned terminal only. Worker pipes and source text never enter this receiver.
pub struct Terminal {
    input: Receiver<Result<String>>,
}
impl Terminal {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            for _ in 0..32 {
                let mut line = vec![];
                let mut byte = [0];
                loop {
                    match input.read(&mut byte) {
                        Ok(1) => {
                            if byte[0] == b'\n' {
                                break;
                            }
                            if line.len() >= 128 || !byte[0].is_ascii_graphic() && byte[0] != b' ' {
                                let _ = tx.send(Err(ReportError::InvalidInput));
                                return;
                            }
                            line.push(byte[0]);
                        }
                        _ => {
                            let _ = tx.send(Err(ReportError::Stopped));
                            return;
                        }
                    }
                }
                if tx
                    .send(String::from_utf8(line).map_err(|_| ReportError::InvalidInput))
                    .is_err()
                {
                    return;
                }
            }
        });
        Self { input: rx }
    }
    #[cfg(test)]
    pub(crate) fn synthetic_input(lines: Vec<Result<String>>) -> Self {
        let (tx, rx) = mpsc::sync_channel(32);
        for line in lines {
            tx.send(line).expect("bounded synthetic terminal input");
        }
        drop(tx);
        Self { input: rx }
    }
    pub fn poll_stop(&self) -> Result<()> {
        match self.input.try_recv() {
            Ok(Ok(line)) if line == "stop" => Err(ReportError::Stopped),
            Ok(_) => Err(ReportError::InvalidInput),
            Err(mpsc::TryRecvError::Empty) => Ok(()),
            Err(_) => Err(ReportError::Stopped),
        }
    }
    pub fn acknowledge(&self, command: &str, digest: [u8; 32], deadline: Instant) -> Result<()> {
        let expected = format!("{command} {}", hex(&digest));
        println!("Type {expected} or stop:");
        let line = self
            .input
            .recv_timeout(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_secs(300)),
            )
            .map_err(|_| ReportError::Deadline)??;
        if line == "stop" {
            return Err(ReportError::Stopped);
        }
        if line != expected {
            return Err(ReportError::InvalidWitness);
        }
        Ok(())
    }
    pub fn wait_for_stop(&self, deadline: Instant) -> Result<()> {
        println!("Actual planning receipt received. Type stop:");
        let line = self
            .input
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| ReportError::Deadline)??;
        if line == "stop" {
            Ok(())
        } else {
            Err(ReportError::InvalidInput)
        }
    }
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
