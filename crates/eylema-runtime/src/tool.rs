//! Immutable typed selection for the executor's two existing owned handlers.
use eylema_contracts::{
    codec::{self, Value},
    tool::*,
    *,
};
use std::collections::BTreeMap;
pub struct ToolRegistry {
    profiles: BTreeMap<eylema_contracts::platform::SemanticProfileId, ToolProfile>,
}
impl ToolRegistry {
    pub fn new(profiles: Vec<ToolProfile>) -> Result<Self, ErrorCode> {
        if profiles.is_empty() || profiles.len() > MAX_TOOL_PROFILES {
            return Err(ErrorCode::BudgetExceeded);
        }
        let mut map = BTreeMap::new();
        for p in profiles {
            if map.insert(p.id().clone(), p).is_some() {
                return Err(ErrorCode::InvalidRequest);
            }
        }
        Ok(Self { profiles: map })
    }
    pub fn validate(&self, plan: &ToolPlan) -> Result<(), ErrorCode> {
        for (b, a) in plan
            .bindings()
            .iter()
            .zip(&plan.proposal().fields().actions)
        {
            let profile = self
                .profiles
                .get(&b.profile)
                .ok_or(ErrorCode::CapabilityUnavailable)?;
            if b.revision != profile.revision() {
                return Err(ErrorCode::RevisionConflict);
            }
            if b.input_schema != profile.input_schema()
                || b.output_schema != profile.output_schema()
            {
                return Err(ErrorCode::InvalidRequest);
            }
            let a = a.fields();
            let op = profile.operation();
            if a.operation_id.as_str() != op.operation_id()
                || a.operation_version != ContractVersion::CURRENT
                || a.side_effect_class != op.effect()
            {
                return Err(ErrorCode::CapabilityUnavailable);
            }
            if a.target.context() != ExecutionContext::NativeMacos
                || a.target.backend() != Backend::NativeCpu
                || a.target.gpu_api() != GpuApi::None
                || a.required_capabilities.len() != 1
                || a.required_capabilities[0].as_str() != "task:state"
            {
                return Err(ErrorCode::CapabilityUnavailable);
            }
            match op {
                OwnedTool::ReadTaskValue if a.payload.is_empty() && b.source.is_none() => {}
                OwnedTool::AddTaskValue if a.payload.len() <= MAX_TOOL_INPUT_BYTES => {
                    let value = codec::decode(&a.payload).map_err(|_| ErrorCode::InvalidRequest)?;
                    if !matches!(value, Value::Unsigned(_))
                        || codec::encode(&value).map_err(|_| ErrorCode::InvalidRequest)?
                            != a.payload
                    {
                        return Err(ErrorCode::InvalidRequest);
                    }
                }
                _ => return Err(ErrorCode::InvalidRequest),
            }
        }
        Ok(())
    }
}
