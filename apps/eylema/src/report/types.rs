//! Closed, content-redacted report domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportError {
    InvalidInput,
    Limit,
    Forbidden,
    Unsupported,
    NeedsSourceReview,
    NoEvidence,
    Abstained,
    Conflict,
    RevisionConflict,
    InvalidWitness,
    Stopped,
    Deadline,
    WorkerFailed,
    OutcomeUnknown,
    Partial,
}
impl ReportError {
    /// Closed public vocabulary; never formats source, model or OS error data.
    pub(crate) fn cli_code(self) -> &'static str {
        match self {
            Self::InvalidInput => "REPORT_INVALID_INPUT",
            Self::Limit => "REPORT_LIMIT",
            Self::Forbidden => "REPORT_FORBIDDEN",
            Self::Unsupported => "REPORT_UNSUPPORTED",
            Self::NeedsSourceReview => "REPORT_NEEDS_SOURCE_REVIEW",
            Self::NoEvidence => "REPORT_NO_EVIDENCE",
            Self::Abstained => "REPORT_ABSTAINED",
            Self::Conflict => "REPORT_CONFLICT",
            Self::RevisionConflict => "REPORT_REVISION_CONFLICT",
            Self::InvalidWitness => "REPORT_INVALID_WITNESS",
            Self::Stopped => "REPORT_STOPPED",
            Self::Deadline => "REPORT_DEADLINE",
            Self::WorkerFailed => "REPORT_WORKER_FAILED",
            Self::OutcomeUnknown => "REPORT_OUTCOME_UNKNOWN",
            Self::Partial => "REPORT_PARTIAL",
        }
    }
}
pub type Result<T> = std::result::Result<T, ReportError>;
#[derive(Clone, Eq, PartialEq)]
pub struct SourceDocument {
    pub language: String,
    pub site: String,
    pub owner: Option<String>,
    pub checklist: Option<String>,
}
impl std::fmt::Debug for SourceDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SourceDocument(<private>)")
    }
}
use eylema_contracts::{
    TaskId,
    codec::{self, Value},
    platform::ProjectId,
};
pub struct ReportNeed {
    pub project: ProjectId,
    pub task: TaskId,
    pub generation: u64,
    pub sections: Vec<(String, String)>,
    source_binding: [u8; 32],
}
impl ReportNeed {
    pub fn new(
        project: &str,
        task: &str,
        generation: u64,
        sections: Vec<(String, String)>,
    ) -> Result<Self> {
        if generation == 0
            || sections.is_empty()
            || sections.len() > 2
            || sections.iter().any(|(lang, site)| {
                !matches!(lang.as_str(), "en" | "tr")
                    || site.is_empty()
                    || site.len() > 128
                    || site.chars().any(char::is_control)
            })
            || sections
                .iter()
                .enumerate()
                .any(|(i, s)| sections[..i].contains(s))
        {
            return Err(ReportError::InvalidInput);
        }
        Ok(Self {
            project: ProjectId::new(project).map_err(|_| ReportError::InvalidInput)?,
            task: TaskId::new(task).map_err(|_| ReportError::InvalidInput)?,
            generation,
            sections,
            source_binding: [0; 32],
        })
    }
    pub fn bind_sources(&mut self, binding: [u8; 32]) -> Result<()> {
        if self.source_binding != [0; 32] || binding == [0; 32] {
            return Err(ReportError::InvalidInput);
        }
        self.source_binding = binding;
        Ok(())
    }
    pub fn digest(&self) -> Result<[u8; 32]> {
        let v = Value::List(vec![
            Value::Text("report:need:v1".into()),
            Value::Bytes(self.source_binding.to_vec()),
            Value::Text(
                "g30:c64:g8;g57:f750000:m100000;g68:d1024;c4096;report:root2500000000000:v1".into(),
            ),
            Value::Text(self.project.as_str().into()),
            Value::Text(self.task.as_str().into()),
            Value::Unsigned(self.generation),
            Value::List(
                self.sections
                    .iter()
                    .map(|(l, s)| Value::List(vec![Value::Text(l.clone()), Value::Text(s.clone())]))
                    .collect(),
            ),
        ]);
        eylema_platform::sha256(&codec::encode(&v).map_err(|_| ReportError::InvalidInput)?)
            .map_err(|_| ReportError::Unsupported)
    }
}
pub struct JobLedger {
    remaining: u64,
    repairs: u8,
}
impl JobLedger {
    pub fn new(work: u64) -> Result<Self> {
        if work == 0 || work > 2_500_000_000_000 {
            return Err(ReportError::Limit);
        }
        Ok(Self {
            remaining: work,
            repairs: 0,
        })
    }
    pub fn reserve(&mut self, work: u64) -> Result<()> {
        self.remaining = self.remaining.checked_sub(work).ok_or(ReportError::Limit)?;
        Ok(())
    }
    pub fn repair(&mut self) -> Result<()> {
        if self.repairs != 0 {
            return Err(ReportError::Limit);
        }
        self.repairs = 1;
        Ok(())
    }
}

#[cfg(test)]
mod error_vocabulary_tests {
    use super::ReportError;

    #[test]
    fn all_report_error_categories_are_distinct_static_literals() {
        let cases = [
            (ReportError::InvalidInput, "REPORT_INVALID_INPUT"),
            (ReportError::Limit, "REPORT_LIMIT"),
            (ReportError::Forbidden, "REPORT_FORBIDDEN"),
            (ReportError::Unsupported, "REPORT_UNSUPPORTED"),
            (ReportError::NeedsSourceReview, "REPORT_NEEDS_SOURCE_REVIEW"),
            (ReportError::NoEvidence, "REPORT_NO_EVIDENCE"),
            (ReportError::Abstained, "REPORT_ABSTAINED"),
            (ReportError::Conflict, "REPORT_CONFLICT"),
            (ReportError::RevisionConflict, "REPORT_REVISION_CONFLICT"),
            (ReportError::InvalidWitness, "REPORT_INVALID_WITNESS"),
            (ReportError::Stopped, "REPORT_STOPPED"),
            (ReportError::Deadline, "REPORT_DEADLINE"),
            (ReportError::WorkerFailed, "REPORT_WORKER_FAILED"),
            (ReportError::OutcomeUnknown, "REPORT_OUTCOME_UNKNOWN"),
            (ReportError::Partial, "REPORT_PARTIAL"),
        ];
        let mut seen = std::collections::BTreeSet::new();
        for (error, expected) in cases {
            assert_eq!(error.cli_code(), expected);
            assert!(seen.insert(error.cli_code()));
        }
    }
}
