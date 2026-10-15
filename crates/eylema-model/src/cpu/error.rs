#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuError {
    UnsupportedProfile,
    InvalidLayout,
    IdentityMismatch,
    NonFinite,
    InvalidInput,
    ContextLimit,
    WorkLimit,
    ResidentLimit,
    Cancelled,
    Deadline,
    InvalidState,
    InvalidOutput,
    AllocationFailed,
    PlatformUnavailable,
}
