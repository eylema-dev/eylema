//! Project-data structure, not consent, authentication or secret detection.
use crate::{ResourceId, Revision, ValidationError, platform::ProjectId, primitives::require};
use std::collections::BTreeSet;
pub const MAX_DATA_ITEMS: usize = 1024;
pub const MAX_DATA_PAYLOAD_BYTES: usize = 64 * 1024;
pub const MAX_DATA_STORE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DATA_PARENTS: usize = 16;
pub const MAX_DATA_DEPTH: usize = 32;
pub const MAX_DATA_SESSIONS: usize = 64;
pub const MAX_DATA_SCOPE_IDS: usize = 64;
pub const MAX_DATA_TTL_MS: u64 = 365 * 24 * 60 * 60 * 1000;
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataId(ResourceId);
impl DataId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        ResourceId::new(value).map(Self)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataRef {
    pub project: ProjectId,
    pub id: DataId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataUse {
    Execute,
    Collect,
    Train,
    ExternalDisclosure,
    EcosystemExport,
}
impl DataUse {
    pub const ALL: [Self; 5] = [
        Self::Execute,
        Self::Collect,
        Self::Train,
        Self::ExternalDisclosure,
        Self::EcosystemExport,
    ];
    pub const fn is_egress(self) -> bool {
        matches!(self, Self::ExternalDisclosure | Self::EcosystemExport)
    }
    pub const fn bit(self) -> u8 {
        match self {
            Self::Execute => 1,
            Self::Collect => 2,
            Self::Train => 4,
            Self::ExternalDisclosure => 8,
            Self::EcosystemExport => 16,
        }
    }
}
/// Host authority is required to apply this declared mask to a record/session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DataPermissions(u8);
impl DataPermissions {
    pub const NONE: Self = Self(0);
    pub fn new(purposes: Vec<DataUse>) -> Result<Self, ValidationError> {
        require(purposes.len() <= 5, "permissions", "too many purposes")?;
        let mut bits = 0;
        for purpose in purposes {
            require(
                bits & purpose.bit() == 0,
                "permissions",
                "duplicate purpose",
            )?;
            bits |= purpose.bit();
        }
        Ok(Self(bits))
    }
    pub fn from_bits(bits: u64) -> Result<Self, ValidationError> {
        require(bits <= 31, "permissions", "unknown purpose bits")?;
        Ok(Self(bits as u8))
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
    pub const fn allows(self, purpose: DataUse) -> bool {
        self.0 & purpose.bit() != 0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataKind {
    Episode,
    Artifact,
    Derived,
    TrainingCandidate,
}
impl DataKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Episode => "episode",
            Self::Artifact => "artifact",
            Self::Derived => "derived",
            Self::TrainingCandidate => "training_candidate",
        }
    }
    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        match value {
            "episode" => Ok(Self::Episode),
            "artifact" => Ok(Self::Artifact),
            "derived" => Ok(Self::Derived),
            "training_candidate" => Ok(Self::TrainingCandidate),
            _ => Err(ValidationError {
                field: "kind",
                message: "unsupported data kind",
            }),
        }
    }
}
/// Eligible caller content only. This type intentionally has no Debug payload dump.
pub struct DataDraft {
    pub reference: DataRef,
    pub kind: DataKind,
    pub parents: Vec<DataRef>,
    pub payload: Vec<u8>,
    pub expires_at: u64,
}
impl DataDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        require(
            !self.payload.is_empty() && self.payload.len() <= MAX_DATA_PAYLOAD_BYTES,
            "payload",
            "expected bounded nonempty payload",
        )?;
        require(
            self.parents.len() <= MAX_DATA_PARENTS,
            "parents",
            "too many sources",
        )?;
        require(
            !matches!(self.kind, DataKind::Derived | DataKind::TrainingCandidate)
                || !self.parents.is_empty(),
            "parents",
            "derived data requires source lineage",
        )?;
        let mut seen = BTreeSet::new();
        for parent in &self.parents {
            require(
                parent.project == self.reference.project && parent != &self.reference,
                "parents",
                "foreign or self source",
            )?;
            require(seen.insert(parent), "parents", "duplicate source")?;
        }
        require(self.expires_at > 0, "expires_at", "zero retention expiry")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataMetadata {
    pub reference: DataRef,
    pub kind: DataKind,
    pub parents: Vec<DataRef>,
    pub digest: [u8; 32],
    pub revision: Revision,
    pub permissions: DataPermissions,
    pub reviewed_egress: bool,
    pub expires_at: u64,
    pub withdrawn: bool,
}
/// Closed projection: no free-text or arbitrary record payload. IDs must be nonsecret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataManifest {
    pub reference: DataRef,
    pub kind: DataKind,
    pub parents: Vec<DataRef>,
    pub digest: [u8; 32],
    pub revision: Revision,
    pub expires_at: u64,
}
#[derive(Clone, Debug)]
pub struct DataGrant {
    pub permissions: DataPermissions,
    pub scope: Vec<DataId>,
    pub expires_at: u64,
}
