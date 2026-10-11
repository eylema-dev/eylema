use crate::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
pub(crate) const MAX_CALLS: usize = 2048;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Call {
    Now(std::result::Result<u64, ErrorCode>),
    Execution(std::result::Result<String, ErrorCode>),
    Event(std::result::Result<String, ErrorCode>),
    Hash(std::result::Result<[u8; 32], ErrorCode>),
}
enum Mode {
    Idle,
    Record(Vec<Call>),
    Replay(VecDeque<Call>, bool),
}
struct Driver<S> {
    services: S,
    mode: Mode,
}
pub(crate) struct Services<S>(Arc<Mutex<Driver<S>>>);
impl<S> Clone for Services<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<S: HostServices> Services<S> {
    pub(crate) fn new(services: S) -> Self {
        Self(Arc::new(Mutex::new(Driver {
            services,
            mode: Mode::Idle,
        })))
    }
    pub(crate) fn record(&self) -> Result<()> {
        let mut driver = self.0.lock().map_err(|_| HostError::Poisoned)?;
        if !matches!(driver.mode, Mode::Idle) {
            return Err(HostError::Poisoned);
        }
        driver.mode = Mode::Record(vec![]);
        Ok(())
    }
    pub(crate) fn replay(&self, calls: Vec<Call>) -> Result<()> {
        if calls.len() > MAX_CALLS {
            return Err(HostError::InvalidRecord);
        }
        let mut driver = self.0.lock().map_err(|_| HostError::Poisoned)?;
        if !matches!(driver.mode, Mode::Idle) {
            return Err(HostError::Poisoned);
        }
        driver.mode = Mode::Replay(calls.into(), false);
        Ok(())
    }
    pub(crate) fn finish(&self) -> Result<Vec<Call>> {
        let mut driver = self.0.lock().map_err(|_| HostError::Poisoned)?;
        match std::mem::replace(&mut driver.mode, Mode::Idle) {
            Mode::Record(calls) => Ok(calls),
            Mode::Replay(calls, false) if calls.is_empty() => Ok(vec![]),
            _ => Err(HostError::ReplayMismatch),
        }
    }
    fn invoke(&self, tag: u8, hash_input: Option<&[u8]>) -> Call {
        let failure = || match tag {
            0 => Call::Now(Err(ErrorCode::OutcomeUnknown)),
            1 => Call::Execution(Err(ErrorCode::OutcomeUnknown)),
            2 => Call::Event(Err(ErrorCode::OutcomeUnknown)),
            _ => Call::Hash(Err(ErrorCode::OutcomeUnknown)),
        };
        let Ok(mut driver) = self.0.lock() else {
            return failure();
        };
        let mut mode = std::mem::replace(&mut driver.mode, Mode::Idle);
        let call = match &mut mode {
            Mode::Replay(calls, mismatch) => {
                let expected = calls.pop_front();
                let matches = matches!(
                    (&expected, tag),
                    (Some(Call::Now(_)), 0)
                        | (Some(Call::Execution(_)), 1)
                        | (Some(Call::Event(_)), 2)
                        | (Some(Call::Hash(_)), 3)
                );
                if !matches {
                    *mismatch = true;
                    failure()
                } else {
                    let expected = expected.expect("checked matching recorded service kind");
                    if let Call::Hash(Ok(hash)) = &expected {
                        if driver
                            .services
                            .sha256(hash_input.expect("SHA invocation input"))
                            != Ok(*hash)
                        {
                            *mismatch = true;
                            failure()
                        } else {
                            expected
                        }
                    } else {
                        expected
                    }
                }
            }
            Mode::Idle | Mode::Record(_) => {
                let value = match tag {
                    0 => Call::Now(driver.services.now().map(|t| t.unix_ms())),
                    1 => {
                        Call::Execution(driver.services.execution_id().map(|id| id.as_str().into()))
                    }
                    2 => Call::Event(driver.services.event_id().map(|id| id.as_str().into())),
                    _ => Call::Hash(
                        driver
                            .services
                            .sha256(hash_input.expect("SHA invocation input")),
                    ),
                };
                if let Mode::Record(calls) = &mut mode {
                    if calls.len() >= MAX_CALLS {
                        driver.mode = Mode::Replay(VecDeque::new(), true);
                        return failure();
                    }
                    calls.push(value.clone());
                }
                value
            }
        };
        driver.mode = mode;
        call
    }
}
impl<S: HostServices> HostServices for Services<S> {
    fn now(&mut self) -> eylema_runtime::Result<Timestamp> {
        match self.invoke(0, None) {
            Call::Now(v) => {
                v.and_then(|n| Timestamp::new(n).map_err(|_| ErrorCode::OutcomeUnknown))
            }
            _ => Err(ErrorCode::OutcomeUnknown),
        }
    }
    fn execution_id(&mut self) -> eylema_runtime::Result<ExecutionId> {
        match self.invoke(1, None) {
            Call::Execution(v) => {
                v.and_then(|s| ExecutionId::new(s).map_err(|_| ErrorCode::OutcomeUnknown))
            }
            _ => Err(ErrorCode::OutcomeUnknown),
        }
    }
    fn event_id(&mut self) -> eylema_runtime::Result<EventId> {
        match self.invoke(2, None) {
            Call::Event(v) => {
                v.and_then(|s| EventId::new(s).map_err(|_| ErrorCode::OutcomeUnknown))
            }
            _ => Err(ErrorCode::OutcomeUnknown),
        }
    }
    fn sha256(&mut self, bytes: &[u8]) -> eylema_runtime::Result<[u8; 32]> {
        match self.invoke(3, Some(bytes)) {
            Call::Hash(v) => v,
            _ => Err(ErrorCode::OutcomeUnknown),
        }
    }
}
