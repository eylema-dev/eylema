//! Closed own-tool selection metadata. None of these fields grant authority.
use crate::{
    codec::Value,
    platform::{ProjectId, SemanticProfileId},
    primitives::require,
    project_data::{DataId, DataRef},
    *,
};
pub const MAX_TOOL_PROFILES: usize = 64;
pub const MAX_TOOL_ACTIONS: usize = 8;
pub const MAX_TOOL_INPUT_BYTES: usize = crate::codec::HEADER.len() + 9;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedTool {
    ReadTaskValue,
    AddTaskValue,
}
impl OwnedTool {
    pub const fn operation_id(self) -> &'static str {
        match self {
            Self::ReadTaskValue => "readTaskValue",
            Self::AddTaskValue => "addTaskValue",
        }
    }
    pub const fn input_schema(self) -> &'static str {
        match self {
            Self::ReadTaskValue => "eyl:empty:v1",
            Self::AddTaskValue => "eyl:u64:v1",
        }
    }
    pub const fn effect(self) -> SideEffectClass {
        match self {
            Self::ReadTaskValue => SideEffectClass::Read,
            Self::AddTaskValue => SideEffectClass::RecordMutation,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolProfile {
    id: SemanticProfileId,
    revision: Revision,
    operation: OwnedTool,
}
impl ToolProfile {
    pub fn new(id: SemanticProfileId, revision: Revision, operation: OwnedTool) -> Self {
        Self {
            id,
            revision,
            operation,
        }
    }
    pub fn id(&self) -> &SemanticProfileId {
        &self.id
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn operation(&self) -> OwnedTool {
        self.operation
    }
    pub fn input_schema(&self) -> SemanticProfileId {
        SemanticProfileId::new(self.operation.input_schema()).expect("fixed schema")
    }
    pub fn output_schema(&self) -> SemanticProfileId {
        SemanticProfileId::new("eyl:task-value:v1").expect("fixed schema")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolBinding {
    pub action_id: ActionId,
    pub profile: SemanticProfileId,
    pub revision: Revision,
    pub input_schema: SemanticProfileId,
    pub output_schema: SemanticProfileId,
    pub source: Option<DataRef>,
}
impl ToolBinding {
    pub fn to_value(&self) -> Value {
        let text = |s: &str| Value::Text(s.into());
        Value::List(vec![
            text("tool-binding"),
            Value::Unsigned(1),
            text(self.action_id.as_str()),
            text(self.profile.as_str()),
            Value::Unsigned(self.revision.get()),
            text(self.input_schema.as_str()),
            text(self.output_schema.as_str()),
            self.source.as_ref().map_or(Value::Null, |s| {
                Value::List(vec![text(s.project.as_str()), text(s.id.as_str())])
            }),
        ])
    }
    pub fn from_value(v: &Value) -> Result<Self, ValidationError> {
        let Value::List(row) = v else {
            return Err(error());
        };
        let [
            tag,
            version,
            action,
            profile,
            revision,
            input,
            output,
            source,
        ]: &[Value; 8] = row.as_slice().try_into().map_err(|_| error())?;
        if string(tag)? != "tool-binding" || number(version)? != 1 {
            return Err(error());
        }
        let source = if *source == Value::Null {
            None
        } else {
            let Value::List(s) = source else {
                return Err(error());
            };
            let [project, id]: &[Value; 2] = s.as_slice().try_into().map_err(|_| error())?;
            Some(DataRef {
                project: ProjectId::new(string(project)?)?,
                id: DataId::new(string(id)?)?,
            })
        };
        Ok(Self {
            action_id: ActionId::new(string(action)?)?,
            profile: SemanticProfileId::new(string(profile)?)?,
            revision: Revision::new(number(revision)?)?,
            input_schema: SemanticProfileId::new(string(input)?)?,
            output_schema: SemanticProfileId::new(string(output)?)?,
            source,
        })
    }
}
fn string(v: &Value) -> Result<&str, ValidationError> {
    if let Value::Text(s) = v {
        Ok(s)
    } else {
        Err(error())
    }
}
fn number(v: &Value) -> Result<u64, ValidationError> {
    if let Value::Unsigned(n) = v {
        Ok(*n)
    } else {
        Err(error())
    }
}
fn error() -> ValidationError {
    ValidationError {
        field: "tool.binding",
        message: "invalid closed tool binding",
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct ToolPlan {
    task: TaskSpec,
    proposal: ActionProposal,
    bindings: Vec<ToolBinding>,
}
impl std::fmt::Debug for ToolPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolPlan")
            .field("actions", &self.bindings.len())
            .finish()
    }
}
impl ToolPlan {
    pub fn new(
        task: TaskSpec,
        proposal: ActionProposal,
        bindings: Vec<ToolBinding>,
    ) -> Result<Self, ValidationError> {
        ActionProposal::new(proposal.fields().clone(), &task)?;
        require(
            !bindings.is_empty()
                && bindings.len() <= MAX_TOOL_ACTIONS
                && bindings.len() == proposal.fields().actions.len(),
            "tool.bindings",
            "one bounded binding per action required",
        )?;
        require(
            bindings
                .iter()
                .zip(&proposal.fields().actions)
                .all(|(b, a)| b.action_id == a.fields().action_id),
            "tool.bindings",
            "bindings must match action order and identity",
        )?;
        Ok(Self {
            task,
            proposal,
            bindings,
        })
    }
    pub fn task(&self) -> &TaskSpec {
        &self.task
    }
    pub fn proposal(&self) -> &ActionProposal {
        &self.proposal
    }
    pub fn bindings(&self) -> &[ToolBinding] {
        &self.bindings
    }
}
