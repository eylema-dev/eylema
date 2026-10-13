//! Source-authorized closed leaf reuse; not arbitrary code or model execution.
use crate::{
    project_data::{DataClock, DataSession, ProjectStore},
    tool::{ToolError, ToolReport},
};
use eylema_contracts::{
    ErrorCode,
    codec::{self, Value},
    project_data::{DataKind, DataRef, DataUse},
    skill::*,
    tool::ToolPlan,
};
use eylema_journal::{Sha256, Storage};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillUse {
    pub reference: DataRef,
    pub skill: SkillId,
    pub revision: eylema_contracts::Revision,
    pub source_put_digest: [u8; 32],
}
#[derive(Clone, Eq, PartialEq)]
pub struct SkillWitness {
    pub(crate) definition: SkillLoop,
    pub(crate) uses: Vec<SkillUse>,
    pub(crate) controls: Vec<DataRef>,
}
impl std::fmt::Debug for SkillWitness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillWitness")
            .field("uses", &self.uses.len())
            .finish()
    }
}
impl SkillWitness {
    pub fn definition(&self) -> &SkillLoop {
        &self.definition
    }
    pub fn uses(&self) -> &[SkillUse] {
        &self.uses
    }
    pub const fn model_executed(&self) -> bool {
        false
    }
    pub const fn loader_implementation(&self) -> &'static str {
        "owned:flat-skill-loader:v1"
    }
    pub(crate) fn workload_key(&self) -> Value {
        let text = |s: &str| Value::Text(s.into());
        Value::List(vec![
            text(self.loader_implementation()),
            text(self.definition.id().as_str()),
            Value::Unsigned(self.definition.revision().get()),
            Value::Unsigned(self.definition.iterations() as u64),
            Value::List(
                self.uses
                    .iter()
                    .map(|u| {
                        Value::List(vec![
                            text(u.reference.project.as_str()),
                            text(u.reference.id.as_str()),
                            text(u.skill.as_str()),
                            Value::Unsigned(u.revision.get()),
                            Value::Bytes(u.source_put_digest.to_vec()),
                        ])
                    })
                    .collect(),
            ),
        ])
    }
}
#[derive(Clone)]
pub struct SkillEvidence {
    pub(crate) witness: SkillWitness,
    pub(crate) outcome: ToolReport,
}
impl std::fmt::Debug for SkillEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillEvidence")
            .field("status", &self.outcome.status)
            .finish()
    }
}
impl SkillEvidence {
    pub fn witness(&self) -> &SkillWitness {
        &self.witness
    }
    pub fn outcome(&self) -> &ToolReport {
        &self.outcome
    }
}
pub(crate) fn resolve<T: Storage, H: Sha256, C: DataClock>(
    data: &mut ProjectStore<T, H, C>,
    session: &DataSession,
    plan: &ToolPlan,
    definition: SkillLoop,
) -> Result<SkillWitness, ToolError> {
    if definition.expanded().count() != plan.bindings().len() {
        return Err(ToolError::InvalidEvidence);
    }
    let mut uses = vec![];
    let mut sources = std::collections::BTreeSet::new();
    for ((call, b), a) in definition
        .expanded()
        .zip(plan.bindings())
        .zip(&plan.proposal().fields().actions)
    {
        let bytes = data.read(session, &call.reference, DataUse::Execute)?;
        if bytes.len() > MAX_SKILL_ARTIFACT_BYTES {
            return Err(ToolError::Runtime(ErrorCode::BudgetExceeded));
        }
        let value = codec::decode(&bytes).map_err(|_| ToolError::InvalidEvidence)?;
        if codec::encode(&value).map_err(|_| ToolError::InvalidEvidence)? != bytes {
            return Err(ToolError::InvalidEvidence);
        }
        let package =
            SkillDefinition::from_value(&value).map_err(|_| ToolError::InvalidEvidence)?;
        let metadata = data.authorized_metadata(session, &call.reference, DataUse::Execute)?;
        if !matches!(metadata.kind, DataKind::Artifact | DataKind::Derived) {
            return Err(ToolError::InvalidEvidence);
        }
        if package.id() != &call.skill || package.revision() != call.revision {
            return Err(ToolError::Runtime(ErrorCode::RevisionConflict));
        }
        if package.adapter().as_str() != OWNED_SKILL_ADAPTER || package.model_dependency().is_some()
        {
            return Err(ToolError::Runtime(ErrorCode::CapabilityUnavailable));
        }
        let profile = package.profile();
        if b.profile != *profile.id()
            || b.revision != profile.revision()
            || b.input_schema != profile.input_schema()
            || b.output_schema != profile.output_schema()
            || a.fields().operation_id.as_str() != profile.operation().operation_id()
        {
            return Err(ToolError::InvalidEvidence);
        }
        sources.insert(call.reference.clone());
        uses.push(SkillUse {
            reference: call.reference.clone(),
            skill: call.skill.clone(),
            revision: call.revision,
            source_put_digest: metadata.digest,
        });
    }
    Ok(SkillWitness {
        definition,
        uses,
        controls: sources.into_iter().collect(),
    })
}
