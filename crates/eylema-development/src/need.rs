use eylema_contracts::{RequestId, Revision};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput,
    InvalidState,
    Forbidden,
    StaleBinding,
    FailedEvidence,
    CallInFlight,
    UnknownCall,
    ClockRegressed,
    BudgetExhausted,
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn require(value: bool) -> Result<()> {
    value.then_some(()).ok_or(Error::InvalidInput)
}

pub(crate) fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= limit
        && !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
}

pub(crate) fn token(value: &str) -> bool {
    text(value, 96)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-:.".contains(&b))
}

pub(crate) fn repository(value: &str) -> bool {
    let parts: Vec<_> = value.split('/').collect();
    parts.len() == 2 && parts.iter().all(|s| token(s) && !s.contains([':', '.']))
}

/// Narrow lexical scope: exact files or directory prefixes ending in '/'.
/// Actual filesystem/symlink ownership is a separate workspace-host gate.
pub(crate) fn path(value: &str, directory: bool) -> bool {
    if value.is_empty() || value.len() > 256 || value.starts_with('/') {
        return false;
    }
    let value = if directory {
        value.strip_suffix('/').unwrap_or(value)
    } else {
        value
    };
    !value.is_empty()
        && value.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s != ".git"
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskKey {
    repository: String,
    request_id: RequestId,
    revision: Revision,
}

impl TaskKey {
    pub fn new(repository_name: String, request_id: RequestId, revision: Revision) -> Result<Self> {
        require(repository(&repository_name))?;
        Ok(Self {
            repository: repository_name,
            request_id,
            revision,
        })
    }
    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit(String);

impl Commit {
    pub fn new(value: String) -> Result<Self> {
        require(
            value.len() == 40
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        )?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Usage units are a host-defined conservative ceiling, not currency or billing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub model_calls: u32,
    pub repairs: u32,
    pub wall_ms: u64,
    pub usage_units: u64,
}

impl Limits {
    pub(crate) fn valid(self) -> bool {
        (1..=1024).contains(&self.model_calls)
            && self.repairs <= 32
            && (1..=86_400_000).contains(&self.wall_ms)
            && (1..=1_000_000_000_000).contains(&self.usage_units)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Acceptance {
    pub id: String,
    pub scenario: String,
    pub expected: String,
}

/// Owned typed NeedSpec 0.1 semantics; JSON parsing is an adapter responsibility.
/// The internal usage ceiling supplements its existing call/repair/wall budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedDraft {
    pub key: TaskKey,
    pub source_repository: String,
    pub issue_number: Option<u64>,
    pub objective: String,
    pub acceptance: Vec<Acceptance>,
    pub allowed_paths: Vec<String>,
    pub out_of_scope: Vec<String>,
    pub targets: Vec<String>,
    pub authorization_ref: String,
    pub limits: Limits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedSpec(NeedDraft);

impl NeedSpec {
    pub fn new(draft: NeedDraft) -> Result<Self> {
        require(repository(&draft.source_repository) && draft.issue_number != Some(0))?;
        require(
            text(&draft.objective, 4096) && token(&draft.authorization_ref) && draft.limits.valid(),
        )?;
        require(!draft.acceptance.is_empty() && draft.acceptance.len() <= 32)?;
        let mut ids = BTreeSet::new();
        for a in &draft.acceptance {
            require(
                token(&a.id)
                    && ids.insert(&a.id)
                    && text(&a.scenario, 4096)
                    && text(&a.expected, 4096),
            )?;
        }
        require(!draft.allowed_paths.is_empty() && draft.allowed_paths.len() <= 32)?;
        require(draft.allowed_paths.iter().all(|s| path(s, true)) && unique(&draft.allowed_paths))?;
        require(
            draft.out_of_scope.len() <= 32
                && draft.out_of_scope.iter().all(|s| text(s, 4096))
                && unique(&draft.out_of_scope),
        )?;
        require(
            !draft.targets.is_empty()
                && draft.targets.len() <= 8
                && draft.targets.iter().all(|s| token(s))
                && unique(&draft.targets),
        )?;
        Ok(Self(draft))
    }
    pub fn draft(&self) -> &NeedDraft {
        &self.0
    }
    pub fn key(&self) -> &TaskKey {
        &self.0.key
    }
    pub fn limits(&self) -> Limits {
        self.0.limits
    }
    pub(crate) fn permits_path(&self, changed: &str) -> bool {
        path(changed, false)
            && self
                .0
                .allowed_paths
                .iter()
                .any(|p| p == changed || (p.ends_with('/') && changed.starts_with(p)))
    }
}

pub(crate) fn unique(items: &[String]) -> bool {
    items.iter().collect::<BTreeSet<_>>().len() == items.len()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckMapping {
    pub acceptance_id: String,
    pub check: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkPlan {
    pub base: Commit,
    pub implementer_context: String,
    pub checks: Vec<CheckMapping>,
}

impl WorkPlan {
    pub(crate) fn validate(&self, need: &NeedSpec, base: &Commit) -> Result<()> {
        require(self.base == *base && token(&self.implementer_context))?;
        require(!self.checks.is_empty() && self.checks.len() <= 64)?;
        let expected: BTreeSet<_> = need
            .draft()
            .acceptance
            .iter()
            .map(|a| a.id.as_str())
            .collect();
        let mut observed = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        for c in &self.checks {
            require(
                expected.contains(c.acceptance_id.as_str())
                    && token(&c.check)
                    && pairs.insert((&c.acceptance_id, &c.check)),
            )?;
            observed.insert(c.acceptance_id.as_str());
        }
        require(observed == expected)
    }
}
