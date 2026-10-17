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
