use eylema_platform::PlatformError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenizeError {
    UnsupportedProfile,
    InvalidProfile,
    HashMismatch,
    InvalidConfig,
    InvalidJson,
    InvalidVocabulary,
    InvalidMerge,
    UnsupportedUnicode,
    InvalidInput,
    TokenLimit,
    SizeLimit,
    ResidentLimit,
    WorkLimit,
    Io,
    AllocationFailed,
    PlatformUnavailable,
    PlatformFailed,
}
impl From<PlatformError> for TokenizeError {
    fn from(e: PlatformError) -> Self {
        match e {
            PlatformError::Unavailable => Self::PlatformUnavailable,
            PlatformError::TooLarge => Self::SizeLimit,
            _ => Self::PlatformFailed,
        }
    }
}
