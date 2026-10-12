//! Standard native adapters; unsupported platform services fail explicitly.
#![forbid(unsafe_code)]
use eylema_contracts::{ErrorCode, EventId, ExecutionId, Timestamp};
use eylema_platform::IdKind;
use eylema_runtime::HostServices;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct NativeServices;
impl HostServices for NativeServices {
    fn now(&mut self) -> eylema_runtime::Result<Timestamp> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::CapabilityUnavailable)?;
        Timestamp::new(
            u64::try_from(elapsed.as_millis()).map_err(|_| ErrorCode::CapabilityUnavailable)?,
        )
        .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn execution_id(&mut self) -> eylema_runtime::Result<ExecutionId> {
        ExecutionId::new(
            eylema_platform::random_id(IdKind::Execution)
                .map_err(|_| ErrorCode::CapabilityUnavailable)?,
        )
        .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn event_id(&mut self) -> eylema_runtime::Result<EventId> {
        EventId::new(
            eylema_platform::random_id(IdKind::Event)
                .map_err(|_| ErrorCode::CapabilityUnavailable)?,
        )
        .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn sha256(&mut self, bytes: &[u8]) -> eylema_runtime::Result<[u8; 32]> {
        eylema_platform::sha256(bytes).map_err(|_| ErrorCode::CapabilityUnavailable)
    }
}
pub struct NativeHash;
impl eylema_journal::Sha256 for NativeHash {
    fn digest(&mut self, bytes: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        eylema_platform::sha256(bytes).map_err(|_| eylema_journal::JournalError::HashUnavailable)
    }
}
