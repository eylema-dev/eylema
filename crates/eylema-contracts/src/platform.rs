//! Declared harness metadata, not model support, authority or execution.
//!
//! Semantic profile IDs pin interfaces by equality. Constructors neither load
//! schemas/artifacts nor prove a model implements them. Runtime controllers
//! must enforce project scope, disclosure rights, step bounds and state reset.
use crate::{ResourceId, Revision, ValidationError, primitives::require};
use std::collections::{BTreeMap, BTreeSet};

/// Independent platform metadata version; existing task and wire versions stay unchanged.
pub const PLATFORM_CONTRACT_VERSION: (u16, u16, u16) = (0, 1, 0);

macro_rules! ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(ResourceId);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
                ResourceId::new(value).map(Self)
            }
            pub fn as_str(&self) -> &str { self.0.as_str() }
        }
    )+};
}
ids!(
    LoopId,
    RoleId,
    ModelId,
    ModelProfileId,
    SemanticProfileId,
    ProjectId
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleKind {
    Perception,
    Reasoning,
    Planning,
    Decision,
    Policy,
    WorldModel,
    Specialist,
    Critic,
    Representation,
}
impl RoleKind {
    pub const ALL: [Self; 9] = [
        Self::Perception,
        Self::Reasoning,
        Self::Planning,
        Self::Decision,
        Self::Policy,
        Self::WorldModel,
        Self::Specialist,
        Self::Critic,
        Self::Representation,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Perception => "perception",
            Self::Reasoning => "reasoning",
            Self::Planning => "planning",
            Self::Decision => "decision",
            Self::Policy => "policy",
            Self::WorldModel => "world_model",
            Self::Specialist => "specialist",
            Self::Critic => "critic",
            Self::Representation => "representation",
        }
    }
}

/// Declared deployment intent, never accepted device support or provider access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeploymentKind {
    OwnedRustCpu,
    OwnedRustMetal,
    ExternalConfigured,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopMode {
    LocalOnly,
    ExplicitExternal,
}

/// Exact declared profile identity; tensor shape alone cannot establish compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleInterface {
    pub role: RoleKind,
    pub version: Revision,
    pub input_schema: SemanticProfileId,
    pub output_schema: SemanticProfileId,
    pub input_units: SemanticProfileId,
    pub output_units: SemanticProfileId,
    pub preprocessing: SemanticProfileId,
    pub action_semantics: SemanticProfileId,
    pub state_reset: SemanticProfileId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProfileDraft {
    pub id: ModelProfileId,
    pub model: ModelId,
    pub artifact: [u8; 32],
    pub artifact_revision: Revision,
    pub interface: RoleInterface,
    pub deployment: DeploymentKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProfile(ModelProfileDraft);
impl ModelProfile {
    pub fn new(draft: ModelProfileDraft) -> Result<Self, ValidationError> {
        require(
            draft.artifact != [0; 32],
            "artifact",
            "zero artifact identity is a placeholder",
        )?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &ModelProfileDraft {
        &self.0
    }
}

/// Bounded declared profiles. Multiple profiles may describe the same model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRegistry(BTreeMap<ModelProfileId, ModelProfile>);
impl ModelRegistry {
    pub fn new(profiles: Vec<ModelProfile>) -> Result<Self, ValidationError> {
        require(
            !profiles.is_empty() && profiles.len() <= 128,
            "profiles",
            "expected 1 to 128 model profiles",
        )?;
        let mut registry = BTreeMap::new();
        for profile in profiles {
            require(
                !registry.contains_key(&profile.fields().id),
                "profiles",
                "duplicate model profile ID",
            )?;
            registry.insert(profile.fields().id.clone(), profile);
        }
        Ok(Self(registry))
    }
    pub fn resolve(&self, id: &ModelProfileId) -> Option<&ModelProfile> {
        self.0.get(id)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleBinding {
    pub id: RoleId,
    pub expected: RoleInterface,
    pub profile: ModelProfileId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopEdge {
    pub from: RoleId,
    pub to: RoleId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopDraft {
    pub id: LoopId,
    pub revision: Revision,
    pub project: ProjectId,
    pub mode: LoopMode,
    pub roles: Vec<RoleBinding>,
    pub edges: Vec<LoopEdge>,
    pub max_steps: u32,
    pub max_iterations: u32,
}

/// Immutable resolved snapshot. Limits are structural ceilings, not runtime enforcement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopDefinition {
    draft: LoopDraft,
    profiles: Vec<ModelProfile>,
}
impl LoopDefinition {
    pub fn new(draft: LoopDraft, registry: &ModelRegistry) -> Result<Self, ValidationError> {
        require(
            !draft.roles.is_empty() && draft.roles.len() <= 32,
            "roles",
            "expected 1 to 32 role bindings",
        )?;
        require(draft.edges.len() <= 128, "edges", "too many loop edges")?;
        require(
            (1..=4096).contains(&draft.max_steps),
            "max_steps",
            "expected 1 to 4096 steps",
        )?;
        require(
            (1..=1024).contains(&draft.max_iterations),
            "max_iterations",
            "expected 1 to 1024 iterations",
        )?;
        let mut interfaces = BTreeMap::new();
        let mut profiles = Vec::with_capacity(draft.roles.len());
        for binding in &draft.roles {
            require(
                !interfaces.contains_key(&binding.id),
                "roles",
                "duplicate role ID",
            )?;
            let profile = registry.resolve(&binding.profile).ok_or(ValidationError {
                field: "profile",
                message: "unknown model profile",
            })?;
            require(
                profile.fields().interface == binding.expected,
                "role_interface",
                "incompatible declared role interface",
            )?;
            require(
                draft.mode != LoopMode::LocalOnly
                    || profile.fields().deployment != DeploymentKind::ExternalConfigured,
                "deployment",
                "local-only loop cannot bind external deployment",
            )?;
            interfaces.insert(binding.id.clone(), &binding.expected);
            profiles.push(profile.clone());
        }
        let mut edges = BTreeSet::new();
        for edge in &draft.edges {
            require(
                edges.insert((&edge.from, &edge.to)),
                "edges",
                "duplicate loop edge",
            )?;
            let from = interfaces.get(&edge.from).ok_or(ValidationError {
                field: "edge.from",
                message: "unknown source role",
            })?;
            let to = interfaces.get(&edge.to).ok_or(ValidationError {
                field: "edge.to",
                message: "unknown destination role",
            })?;
            require(
                from.output_schema == to.input_schema && from.output_units == to.input_units,
                "edges",
                "incompatible declared edge schema or units",
            )?;
        }
        Ok(Self { draft, profiles })
    }
    pub fn fields(&self) -> &LoopDraft {
        &self.draft
    }
    pub fn profiles(&self) -> &[ModelProfile] {
        &self.profiles
    }
}
