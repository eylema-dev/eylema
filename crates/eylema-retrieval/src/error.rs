use eylema_journal::JournalError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalError {
    InvalidInput,
    WrongProject,
    IncompatibleBinding,
    StaleGeneration,
    NotIndexed,
    InvalidVector,
    LimitExceeded,
    TieLimit,
    Cancelled,
    Deadline,
    TokenCounterUnavailable,
    CorruptSnapshot,
    NeedsReopen,
    Journal(JournalError),
}
pub type RetrievalResult<T> = Result<T, RetrievalError>;
impl From<JournalError> for RetrievalError {
    fn from(e: JournalError) -> Self {
        Self::Journal(e)
    }
}
impl std::fmt::Display for RetrievalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RetrievalError {}
