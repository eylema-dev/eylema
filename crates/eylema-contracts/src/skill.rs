//! Closed reusable leaf metadata. Packages never install code or grant authority.
use crate::{codec::Value, platform::*, primitives::require, project_data::DataRef, tool::*, *};
pub const OWNED_SKILL_ADAPTER: &str = "owned:native-counter:v1";
pub const MAX_SKILL_ARTIFACT_BYTES: usize = 4096;
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SkillId(ResourceId);
impl SkillId {
    pub fn new(s: impl Into<String>) -> Result<Self, ValidationError> {
        ResourceId::new(s).map(Self)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDependency {
    profile: ModelProfileId,
    revision: Revision,
    artifact: [u8; 32],
    adapter: SemanticProfileId,
}
impl ModelDependency {
    pub fn new(
        profile: ModelProfileId,
        revision: Revision,
        artifact: [u8; 32],
        adapter: SemanticProfileId,
    ) -> Result<Self, ValidationError> {
        require(artifact != [0; 32], "artifact", "missing model artifact")?;
        Ok(Self {
            profile,
            revision,
            artifact,
            adapter,
        })
    }
    pub fn profile(&self) -> &ModelProfileId {
        &self.profile
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn artifact(&self) -> [u8; 32] {
        self.artifact
    }
    pub fn adapter(&self) -> &SemanticProfileId {
        &self.adapter
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct SkillDefinition {
    id: SkillId,
    revision: Revision,
    profile: ToolProfile,
    adapter: SemanticProfileId,
    model: Option<ModelDependency>,
    acceptance: SemanticProfileId,
}
impl std::fmt::Debug for SkillDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SkillDefinition(<declared>)")
    }
}
impl SkillDefinition {
    pub fn new(
        id: SkillId,
        revision: Revision,
        profile: ToolProfile,
        adapter: SemanticProfileId,
        model: Option<ModelDependency>,
        acceptance: SemanticProfileId,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            id,
            revision,
            profile,
            adapter,
            model,
            acceptance,
        })
    }
    pub fn id(&self) -> &SkillId {
        &self.id
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn profile(&self) -> &ToolProfile {
        &self.profile
    }
    pub fn adapter(&self) -> &SemanticProfileId {
        &self.adapter
    }
    pub fn model_dependency(&self) -> Option<&ModelDependency> {
        self.model.as_ref()
    }
    pub fn declared_acceptance(&self) -> &SemanticProfileId {
        &self.acceptance
    }
    pub fn to_value(&self) -> Value {
        let text = |s: &str| Value::Text(s.into());
        let model = self.model.as_ref().map_or(Value::Null, |m| {
            Value::List(vec![
                text(m.profile.as_str()),
                Value::Unsigned(m.revision.get()),
                Value::Bytes(m.artifact.to_vec()),
                text(m.adapter.as_str()),
            ])
        });
        Value::List(vec![
            text("skill"),
            Value::Unsigned(1),
            text(self.id.as_str()),
            Value::Unsigned(self.revision.get()),
            text(self.profile.id().as_str()),
            Value::Unsigned(self.profile.revision().get()),
            text(self.profile.operation().operation_id()),
            text(self.profile.input_schema().as_str()),
            text(self.profile.output_schema().as_str()),
            text(self.adapter.as_str()),
            model,
            text(self.acceptance.as_str()),
        ])
    }
    pub fn from_value(v: &Value) -> Result<Self, ValidationError> {
        let bad = || ValidationError {
            field: "skill",
            message: "invalid closed skill/v1 artifact",
        };
        let Value::List(f) = v else { return Err(bad()) };
        require(f.len() == 12, "skill", "expected closed leaf artifact")?;
        require(
            string(&f[0])? == "skill" && number(&f[1])? == 1,
            "skill",
            "unknown skill version",
        )?;
        let op = match string(&f[6])? {
            "readTaskValue" => OwnedTool::ReadTaskValue,
            "addTaskValue" => OwnedTool::AddTaskValue,
            _ => return Err(bad()),
        };
        let profile = ToolProfile::new(
            SemanticProfileId::new(string(&f[4])?)?,
            Revision::new(number(&f[5])?)?,
            op,
        );
        require(
            string(&f[7])? == profile.input_schema().as_str()
                && string(&f[8])? == profile.output_schema().as_str(),
            "schema",
            "incompatible owned schema",
        )?;
        let model = match &f[10] {
            Value::Null => None,
            Value::List(m) if m.len() == 4 => {
                let Value::Bytes(digest) = &m[2] else {
                    return Err(bad());
                };
                let artifact: [u8; 32] = digest.as_slice().try_into().map_err(|_| bad())?;
                Some(ModelDependency::new(
                    ModelProfileId::new(string(&m[0])?)?,
                    Revision::new(number(&m[1])?)?,
                    artifact,
                    SemanticProfileId::new(string(&m[3])?)?,
                )?)
            }
            _ => return Err(bad()),
        };
        Self::new(
            SkillId::new(string(&f[2])?)?,
            Revision::new(number(&f[3])?)?,
            profile,
            SemanticProfileId::new(string(&f[9])?)?,
            model,
            SemanticProfileId::new(string(&f[11])?)?,
        )
    }
}
fn string(v: &Value) -> Result<&str, ValidationError> {
    if let Value::Text(s) = v {
        Ok(s)
    } else {
        Err(ValidationError {
            field: "skill",
            message: "expected text identity",
        })
    }
}
fn number(v: &Value) -> Result<u64, ValidationError> {
    if let Value::Unsigned(n) = v {
        Ok(*n)
    } else {
        Err(ValidationError {
            field: "skill",
            message: "expected positive revision/version",
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillCall {
    pub reference: DataRef,
    pub skill: SkillId,
    pub revision: Revision,
}
#[derive(Clone, Eq, PartialEq)]
pub struct SkillLoop {
    id: LoopId,
    revision: Revision,
    calls: Vec<SkillCall>,
    iterations: u32,
}
impl std::fmt::Debug for SkillLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillLoop")
            .field("calls", &self.calls.len())
            .field("iterations", &self.iterations)
            .finish()
    }
}
impl SkillLoop {
    pub fn new(
        id: LoopId,
        revision: Revision,
        calls: Vec<SkillCall>,
        iterations: u32,
    ) -> Result<Self, ValidationError> {
        require(
            !calls.is_empty()
                && calls.len() <= 8
                && (1..=8).contains(&iterations)
                && calls.len() * iterations as usize <= 8,
            "calls",
            "bounded flat loop required",
        )?;
        require(
            calls
                .iter()
                .all(|c| c.reference.project == calls[0].reference.project),
            "project",
            "cross-project skill loop",
        )?;
        Ok(Self {
            id,
            revision,
            calls,
            iterations,
        })
    }
    pub fn id(&self) -> &LoopId {
        &self.id
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn calls(&self) -> &[SkillCall] {
        &self.calls
    }
    pub fn iterations(&self) -> u32 {
        self.iterations
    }
    pub fn expanded(&self) -> impl Iterator<Item = &SkillCall> {
        (0..self.iterations).flat_map(|_| self.calls.iter())
    }
}
