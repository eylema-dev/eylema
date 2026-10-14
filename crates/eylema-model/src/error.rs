use eylema_platform::PlatformError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadError {
    InvalidProfile,
    InvalidHeader,
    InvalidLayout,
    SizeLimit,
    WorkLimit,
    ResidentLimit,
    MissingTensor,
    ExtraTensor,
    HashMismatch,
    AllocationFailed,
    Io,
    PlatformUnavailable,
    PlatformFailed,
}
impl From<PlatformError> for LoadError {
    fn from(e: PlatformError) -> Self {
        match e {
            PlatformError::Unavailable => Self::PlatformUnavailable,
            PlatformError::TooLarge => Self::SizeLimit,
            _ => Self::PlatformFailed,
        }
    }
}
pub(crate) fn reserved<T>(n: usize) -> Result<Vec<T>, LoadError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| LoadError::AllocationFailed)?;
    Ok(v)
}
pub(crate) fn copied(s: &str) -> Result<String, LoadError> {
    let mut v = String::new();
    v.try_reserve_exact(s.len())
        .map_err(|_| LoadError::AllocationFailed)?;
    v.push_str(s);
    Ok(v)
}
