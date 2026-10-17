//! Project-private report composition; model proposals never grant authority.
mod source;
#[cfg(test)]
mod tests;
mod types;

mod control;
mod models;
mod wire;
mod worker;

pub(crate) fn worker_entry() -> types::Result<()> {
    worker::serve()
}
mod storage;
mod verify;

use eylema_contracts::{
    codec::{self, Value},
    memory::durable::*,
    platform::ProjectId,
    project_data::*,
    tool::*,
    *,
};
use eylema_host::{
    DurableHost, DurableSession, Owner,
    memory_store::MemoryStatus,
    native::{NativeHash, NativeServices},
    tool::{ToolHost, report_control::ReportController},
};
use eylema_journal::{Journal, NativeStorage};
use eylema_runtime::{Grant, tool::ToolRegistry};
use models::{AssetPaths, DecisionInput, ModelReply, ModelRequest};
use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use storage::{JobData, Memory, SourceRow, pin, rev, semantic};
use types::*;
use verify::{PreparedReport, SelectedFacts};
type Tools = ToolHost<NativeStorage, NativeHash, NativeServices>;
struct Section {
    language: String,
    site: String,
    entity: String,
    query: String,
    prompt: String,
}
struct Config {
    project: ProjectId,
    task: String,
    sections: Vec<Section>,
    rows: Vec<SourceRow>,
    assets: AssetPaths,
    value: Value,
}
fn invalid<T>(_: T) -> ReportError {
    ReportError::InvalidInput
}
fn encoded(v: &Value) -> Result<Vec<u8>> {
    codec::encode(v).map_err(invalid)
}
fn digest(v: &Value) -> Result<[u8; 32]> {
    eylema_platform::sha256(&encoded(v)?).map_err(invalid)
}
fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).map_err(invalid)?;
    if !meta.is_file() || meta.len() > 8192 {
        return Err(ReportError::Limit);
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    std::fs::File::open(path)
        .map_err(invalid)?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() > 8192 {
        return Err(ReportError::Limit);
    }
    Ok(bytes)
}
impl Config {
    fn load(path: &Path) -> Result<Self> {
        let value = wire::decode_closed(&read_bounded(path)?)?;
        let rows = wire::list(&value)?;
        if rows.len() != 5 {
            return Err(ReportError::InvalidInput);
        }
        let project = ProjectId::new(wire::text(&rows[0])?).map_err(invalid)?;
        let task = wire::text(&rows[1])?.to_owned();
        TaskId::new(&task).map_err(invalid)?;
        let sections = wire::list(&rows[2])?
            .iter()
            .map(|v| {
                let v = wire::list(v)?;
                if v.len() != 5 {
                    return Err(ReportError::InvalidInput);
                }
                let s = Section {
                    language: wire::text(&v[0])?.into(),
                    site: wire::text(&v[1])?.into(),
                    entity: wire::text(&v[2])?.into(),
                    query: wire::text(&v[3])?.into(),
                    prompt: wire::text(&v[4])?.into(),
                };
                if s.query.len() > 1024 || s.prompt.len() > 8192 {
                    return Err(ReportError::Limit);
                }
                storage::id(&s.entity)?;
                Ok(s)
            })
            .collect::<Result<Vec<_>>>()?;
        if sections.len() != 2 {
            return Err(ReportError::InvalidInput);
        }
        let sources = wire::list(&rows[3])?;
        if sources.is_empty() || sources.len() > 16 {
            return Err(ReportError::Limit);
        }
        let rows = sources
            .iter()
            .map(|v| {
                let v = wire::list(v)?;
                if v.len() != 3 {
                    return Err(ReportError::InvalidInput);
                }
                Ok(SourceRow {
                    id: wire::text(&v[0])?.into(),
                    entity: wire::text(&v[1])?.into(),
                    body: wire::text(&v[2])?.as_bytes().to_vec(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let assets = AssetPaths::from_value(&wire::list(&value)?[4])?;
        Ok(Self {
            project,
            task,
            sections,
            rows,
            assets,
            value,
        })
    }
}
fn new_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| ReportError::OutcomeUnknown)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(ReportError::Unsupported)
    }
}
fn tool_grant(human: bool) -> Result<Grant> {
    Ok(Grant {
        principal: PrincipalRef::new("principal:local-report").map_err(invalid)?,
        resource_scope: ResourceScope::new(vec![ResourceId::new("state:report").map_err(invalid)?])
            .map_err(invalid)?,
        permissions: if human {
            vec![
                Permission::Read,
                Permission::ObserveEvents,
                Permission::RequestCancellation,
                Permission::ApproveAction,
            ]
        } else {
            vec![
                Permission::Read,
                Permission::CreateTask,
                Permission::ExecuteAction,
                Permission::ObserveEvents,
                Permission::RequestCancellation,
            ]
        },
        policy_revision: rev(1),
    })
}
fn tools(dir: &Path) -> Result<(Tools, Owner, DurableSession)> {
    let journal = Journal::create_new(
        NativeStorage::create_new(&dir.join("tools.eylj"), storage::sync).map_err(invalid)?,
        NativeHash,
        Default::default(),
    )
    .map_err(invalid)?;
    let (host, owner) =
        DurableHost::create_new(journal, NativeServices, Default::default()).map_err(invalid)?;
    let registry = ToolRegistry::new(vec![ToolProfile::new(
        semantic("tool:add")?,
        rev(1),
        OwnedTool::AddTaskValue,
    )])
    .map_err(invalid)?;
    let mut tools = ToolHost::new(host, registry).map_err(invalid)?;
    let actor = tools.enroll(&owner, tool_grant(false)?).map_err(invalid)?;
    Ok((tools, owner, actor))
}
fn context() -> Result<RequestContext> {
    Ok(RequestContext {
        request_id: RequestId::new(
            eylema_platform::random_id(eylema_platform::IdKind::Request).map_err(invalid)?,
        )
        .map_err(invalid)?,
        correlation_id: CorrelationId::new(
            eylema_platform::random_id(eylema_platform::IdKind::Correlation).map_err(invalid)?,
        )
        .map_err(invalid)?,
        contract_version: ContractVersion::CURRENT,
    })
}
fn task(need: &ReportNeed, deadline: Timestamp) -> Result<TaskSpec> {
    TaskSpec::new(TaskDraft {
        context: context()?,
        task_id: need.task.clone(),
        revision: rev(1),
        objective: "Verified project-private source-bound report".into(),
        resource_scope: tool_grant(false)?.resource_scope,
        budget: Budget::new(8, 32, 8192, 1, deadline).map_err(invalid)?,
        target: ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .map_err(invalid)?,
    })
    .map_err(invalid)
}
fn checkpoint(task: &TaskSpec, count: u64, source: DataRef) -> Result<ToolPlan> {
    let scope = tool_grant(false)?.resource_scope;
    let g = tool_grant(false)?;
    let action = ActionSpec::new(ActionDraft {
        context: task.fields().context.clone(),
        action_id: ActionId::new("action:report-count").map_err(invalid)?,
        task_id: task.fields().task_id.clone(),
        operation_id: OperationId::new("addTaskValue").map_err(invalid)?,
        operation_version: ContractVersion::CURRENT,
        side_effect_class: SideEffectClass::RecordMutation,
        resource_scope: scope.clone(),
        expected_revision: Some(rev(1)),
        authority: AuthorityRequirement::new(
            g.principal,
            rev(1),
            scope,
            vec![Permission::ExecuteAction],
            OversightRequirements::new(vec![], true, true, None).map_err(invalid)?,
        )
        .map_err(invalid)?,
        required_capabilities: vec![CapabilityId::new("task:state").map_err(invalid)?],
        budget: task.fields().budget,
        target: task.fields().target,
        idempotency_key: IdempotencyKey::new("report:count:once").map_err(invalid)?,
        payload: encoded(&Value::Unsigned(count))?,
    })
    .map_err(invalid)?;
    let binding = ToolBinding {
        action_id: action.fields().action_id.clone(),
        profile: semantic("tool:add")?,
        revision: rev(1),
        input_schema: semantic("eyl:u64:v1")?,
        output_schema: semantic("eyl:task-value:v1")?,
        source: Some(source),
    };
    let proposal = ActionProposal::new(
        ProposalDraft {
            context: task.fields().context.clone(),
            proposal_id: ProposalId::new("proposal:report-count").map_err(invalid)?,
            task_id: task.fields().task_id.clone(),
            plan_revision: rev(1),
            state_revision: rev(1),
            policy_revision: rev(1),
            actions: vec![action],
        },
        task,
    )
    .map_err(invalid)?;
    ToolPlan::new(task.clone(), proposal, vec![binding]).map_err(invalid)
}
fn check(
    tools: &Tools,
    person: &ReportController,
    need: &ReportNeed,
    data: &mut JobData,
    terminal: &control::Terminal,
    deadline: Instant,
) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(ReportError::Deadline);
    }
    terminal.poll_stop()?;
    tools
        .check_report_controller(person, &need.task, need.digest()?)
        .map_err(|_| ReportError::Forbidden)?;
    data.authorize_sources()
}
struct Authority<'a> {
    tools: &'a Tools,
    person: &'a ReportController,
    need: &'a ReportNeed,
    terminal: &'a control::Terminal,
    deadline: Instant,
}
fn dispatch(
    worker: &mut worker::Worker,
    request: &ModelRequest,
    ledger: &mut JobLedger,
    allocation: u64,
    data: &mut JobData,
    authority: Authority<'_>,
) -> Result<Vec<ModelReply>> {
    let Authority {
        tools,
        person,
        need,
        terminal,
        deadline,
    } = authority;
    check(tools, person, need, data, terminal, deadline)?;
    let replies = worker.dispatch(request, ledger, allocation, || {
        check(tools, person, need, data, terminal, deadline)
    })?;
    check(tools, person, need, data, terminal, deadline)?;
    for (i, reply) in replies.iter().enumerate() {
        let fields = wire::list(&reply.receipt)?;
        if fields.len() != 7 {
            return Err(ReportError::InvalidWitness);
        }
        let input = match request {
            ModelRequest::Documents(v) | ModelRequest::Queries(v) | ModelRequest::Plan(v) => {
                Value::Text(v[i].clone())
            }
            ModelRequest::Decide(_) => request.value(),
        };
        if wire::hash(&fields[2])? != digest(&input)?
            || wire::number(&fields[5])? > allocation
            || wire::number(&fields[6])? > models::RESIDENT
        {
            return Err(ReportError::InvalidWitness);
        }
    }
    Ok(replies)
}
fn vector(reply: &ModelReply) -> Result<Vec<f32>> {
    let Value::Bytes(bytes) = &reply.output else {
        return Err(ReportError::InvalidWitness);
    };
    if bytes.len() != 4096 {
        return Err(ReportError::InvalidWitness);
    }
    let v: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    if v.iter().any(|x| !x.is_finite()) {
        return Err(ReportError::InvalidWitness);
    }
    Ok(v)
}
fn receipts(v: &[ModelReply]) -> Value {
    Value::List(v.iter().map(|r| r.receipt.clone()).collect())
}
fn save_receipts(
    data: &mut JobData,
    name: &str,
    replies: &[ModelReply],
    parents: Vec<DataRef>,
) -> Result<SourcePin> {
    let value = if name == "model:plans" {
        Value::List(
            replies
                .iter()
                .map(|r| Value::List(vec![r.receipt.clone(), r.output.clone()]))
                .collect(),
        )
    } else {
        receipts(replies)
    };
    Ok(pin(&data.put(
        name,
        encoded(&value)?,
        parents,
        DataKind::Derived,
    )?))
}
fn selected(
    section: &Section,
    context: &eylema_host::memory_store::MemoryContext,
    sources: &[storage::SourceValue],
) -> Result<SelectedFacts> {
    if context.status() == MemoryStatus::Conflict {
        return Err(ReportError::Conflict);
    }
    for record in context.records() {
        let valid = sources.iter().any(|source| {
            source.entity == section.entity
                && source.document.language == section.language
                && source.document.site == section.site
                && record.draft.sources.contains(&source.pin)
                && match (
                    record.draft.predicate.as_ref().map(|p| p.as_str()),
                    &record.draft.object,
                ) {
                    (Some("owner"), Some(TypedObject::Text(v))) => {
                        source.document.owner.as_ref() == Some(v)
                    }
                    (Some("checklist"), Some(TypedObject::Text(v))) => {
                        source.document.checklist.as_ref() == Some(v)
                    }
                    _ => false,
                }
        });
        if !valid {
            return Err(ReportError::InvalidWitness);
        }
    }
    let field = |name: &str| -> Result<String> {
        let rows: Vec<_> = context
            .records()
            .iter()
            .filter(|r| {
                r.draft
                    .subject
                    .as_ref()
                    .is_some_and(|s| s.as_str() == section.entity)
                    && r.draft
                        .predicate
                        .as_ref()
                        .is_some_and(|p| p.as_str() == name)
            })
            .collect();
        if rows.len() != 1 {
            return Err(ReportError::NoEvidence);
        }
        match &rows[0].draft.object {
            Some(TypedObject::Text(s)) => Ok(s.clone()),
            _ => Err(ReportError::InvalidWitness),
        }
    };
    Ok(SelectedFacts {
        language: section.language.clone(),
        site: section.site.clone(),
        owner: field("owner")?,
        checklist: field("checklist")?,
    })
}
fn run_job(config: &Config, dir: &Path, terminal: &control::Terminal, stopped: bool) -> Result<()> {
    new_directory(dir)?;
    let mut data = JobData::create(dir, config.project.clone(), &config.rows)?;
    let mut need = ReportNeed::new(
        config.project.as_str(),
        &if stopped {
            format!("{}:stop", config.task)
        } else {
            config.task.clone()
        },
        if stopped { 2 } else { 1 },
        config
            .sections
            .iter()
            .map(|s| (s.language.clone(), s.site.clone()))
            .collect(),
    )?;
    let source_hash = data.source_digest()?;
    need.bind_sources(digest(&Value::List(vec![
        config.value.clone(),
        Value::Bytes(source_hash.to_vec()),
        Value::Bool(stopped),
    ]))?)?;
    let need_hash = need.digest()?;
    let need_record = data.put(
        "need",
        encoded(&Value::Bytes(need_hash.to_vec()))?,
        data.sources
            .iter()
            .map(|s| s.pin.reference.clone())
            .collect(),
        DataKind::Derived,
    )?;
    let (mut tools, owner, actor) = tools(dir)?;
    let start = Instant::now();
    let deadline = start + Duration::from_secs(300);
    let request = tools
        .request_report_controller(
            &owner,
            tool_grant(true)?,
            need.task.clone(),
            need_hash,
            Duration::from_secs(300),
        )
        .map_err(invalid)?;
    println!(
        "Review private source claims for job {} ({})",
        need.task.as_str(),
        control::hex(&need_hash)
    );
    for row in &config.rows {
        println!(
            "{:?}: {:?}",
            row.id,
            std::str::from_utf8(&row.body).map_err(invalid)?
        );
    }
    let proof = request
        .authenticate(Duration::from_secs(60))
        .map_err(|_| ReportError::Forbidden)?;
    let person = tools
        .enroll_report_controller(&owner, request, proof)
        .map_err(|_| ReportError::Forbidden)?;
    terminal.acknowledge("review", need_hash, deadline)?;
    check(&tools, &person, &need, &mut data, terminal, deadline)?;
    let review = pin(&data.put(
        "review",
        encoded(&Value::List(vec![
            Value::Text("explicit-source-review".into()),
            Value::Bytes(need_hash.to_vec()),
        ]))?,
        vec![need_record.reference.clone()],
        DataKind::Derived,
    )?);
    let task = task(&need, person.deadline())?;
    tools
        .create_report_task(&owner, &actor, &person, need_hash, task.clone())
        .map_err(invalid)?;
    let mut worker = worker::Worker::spawn(
        &config.assets,
        if stopped {
            100_000_000_000
        } else {
            2_500_000_000_000
        },
        deadline.min(Instant::now() + person.remaining()),
    )?;
    let mut ledger = JobLedger::new(if stopped {
        100_000_000_000
    } else {
        2_500_000_000_000
    })?;
    if stopped {
        let replies = dispatch(
            &mut worker,
            &ModelRequest::Plan(vec![config.sections[0].prompt.clone()]),
            &mut ledger,
            80_000_000_000,
            &mut data,
            Authority {
                tools: &tools,
                person: &person,
                need: &need,
                terminal,
                deadline,
            },
        )?;
        save_receipts(
            &mut data,
            "model:plans",
            &replies,
            vec![need_record.reference.clone(), review.reference.clone()],
        )?;
        terminal.wait_for_stop(deadline)?;
        worker.cancel_and_wait()?;
        tools.revoke_report_controller(&person).map_err(invalid)?;
        data.put(
            "outcome",
            encoded(&Value::Text("stopped_before_effect".into()))?,
            vec![need_record.reference],
            DataKind::Derived,
        )?;
        return Ok(());
    }
    let mut drafts = vec![];
    let mut seen_entities = std::collections::BTreeSet::new();
    for i in 0..data.sources.len() {
        if seen_entities.insert(data.sources[i].entity.clone()) {
            let entity = data.draft(i, None)?;
            data.store_reviewed(entity, None, review.clone())?;
        }
        for predicate in ["owner", "checklist"] {
            match data.draft(i, Some(predicate)) {
                Ok(d) => drafts.push(d),
                Err(ReportError::NoEvidence) => {}
                Err(e) => return Err(e),
            }
        }
    }
    let texts = drafts
        .iter()
        .map(|d| {
            Memory::preview_projection(&data.profile, d)
                .map_err(invalid)
                .and_then(|b| String::from_utf8(b).map_err(invalid))
        })
        .collect::<Result<Vec<_>>>()?;
    let documents = dispatch(
        &mut worker,
        &ModelRequest::Documents(texts),
        &mut ledger,
        650_000_000_000,
        &mut data,
        Authority {
            tools: &tools,
            person: &person,
            need: &need,
            terminal,
            deadline,
        },
    )?;
    let mut projections = vec![];
    for (draft, reply) in drafts.into_iter().zip(&documents) {
        let vector = vector(reply)?;
        projections.push(data.store_reviewed(draft, Some(&vector), review.clone())?);
    }
    let mut parents: Vec<_> = data
        .sources
        .iter()
        .map(|s| s.pin.reference.clone())
        .collect();
    parents.extend(projections.iter().map(|p| p.reference.clone()));
    let document_receipts = save_receipts(&mut data, "model:documents", &documents, parents)?;
    let mut queries = config
        .sections
        .iter()
        .map(|s| s.query.clone())
        .collect::<Vec<_>>();
    queries.push("Unknown owner checklist".into());
    let embedded_queries = dispatch(
        &mut worker,
        &ModelRequest::Queries(queries.clone()),
        &mut ledger,
        450_000_000_000,
        &mut data,
        Authority {
            tools: &tools,
            person: &person,
            need: &need,
            terminal,
            deadline,
        },
    )?;
    let query_receipts = save_receipts(
        &mut data,
        "model:queries",
        &embedded_queries,
        vec![need_record.reference.clone(), review.reference.clone()],
    )?;
    let mut contexts = vec![];
    for (i, section) in config.sections.iter().enumerate() {
        contexts.push(data.query(
            &section.entity,
            &section.query,
            vector(&embedded_queries[i])?,
        )?);
    }
    let unknown = data.query("unknown", &queries[2], vector(&embedded_queries[2])?)?;
    if unknown.status() != MemoryStatus::NoEvidence
        || unknown.record_count() != 0
        || unknown.source_count() != 0
    {
        return Err(ReportError::InvalidWitness);
    }
    let facts = config
        .sections
        .iter()
        .zip(&contexts)
        .map(|(s, c)| selected(s, c, &data.sources))
        .collect::<Result<Vec<_>>>()?;
    let mut citations = vec![];
    for c in &contexts {
        for r in c.records() {
            citations.push(r.projection.clone());
        }
        for s in c.citations() {
            citations.push(s.pin.clone());
        }
    }
    citations.sort_by(|a, b| a.reference.cmp(&b.reference));
    citations.dedup();
    let verified_pins = storage::verify_pins(&mut data.data, &data.session, &citations)?;
    let selection_generation = data.memory.generation();
    let plans = dispatch(
        &mut worker,
        &ModelRequest::Plan(config.sections.iter().map(|s| s.prompt.clone()).collect()),
        &mut ledger,
        160_000_000_000,
        &mut data,
        Authority {
            tools: &tools,
            person: &person,
            need: &need,
            terminal,
            deadline,
        },
    )?;
    let plan_receipts = save_receipts(
        &mut data,
        "model:plans",
        &plans,
        vec![
            need_record.reference.clone(),
            query_receipts.reference.clone(),
            review.reference.clone(),
        ],
    )?;
    let mut options = facts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                format!("section:{i}"),
                format!(
                    "site={};owner={};checklist={}",
                    s.site, s.owner, s.checklist
                ),
            )
        })
        .collect::<Vec<_>>();
    options.sort_by(|a, b| a.0.cmp(&b.0));
    let inputs = config
        .sections
        .iter()
        .zip(&plans)
        .map(|(s, p)| {
            Ok(DecisionInput {
                query: models::decision_query(&s.query, wire::text(&p.output)?)?,
                options: options.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let decisions = dispatch(
        &mut worker,
        &ModelRequest::Decide(inputs),
        &mut ledger,
        1_000_000_000_000,
        &mut data,
        Authority {
            tools: &tools,
            person: &person,
            need: &need,
            terminal,
            deadline,
        },
    )?;
    let answers = wire::list(&decisions[0].output)?;
    if answers.len() != config.sections.len() {
        return Err(ReportError::InvalidWitness);
    }
    for (i, answer) in answers.iter().enumerate() {
        let a = wire::list(answer)?;
        if a.len() != 3 {
            return Err(ReportError::InvalidWitness);
        }
        if matches!(a[1], Value::Null) {
            return Err(ReportError::Abstained);
        }
        if wire::text(&a[1])? != format!("section:{i}") {
            return Err(ReportError::InvalidWitness);
        }
    }
    let decision_receipts = save_receipts(
        &mut data,
        "model:decisions",
        &decisions,
        vec![
            need_record.reference.clone(),
            document_receipts.reference.clone(),
            query_receipts.reference.clone(),
            plan_receipts.reference.clone(),
        ],
    )?;
    if data.memory.generation() != selection_generation {
        return Err(ReportError::RevisionConflict);
    }
    verified_pins.recheck(&mut data.data, &data.session)?;
    let sources = data
        .sources
        .iter()
        .map(|s| s.document.clone())
        .collect::<Vec<_>>();
    let prepared = verify::prepare_report(
        &need,
        &sources,
        &facts,
        digest(&receipts(&plans))?,
        digest(&receipts(&decisions))?,
        true,
    )?
    .bind_citations(&verified_pins)?;
    // The actual model path has completed; pure verification is the only repairable phase.
    let mut synthetic = facts.clone();
    synthetic[0].owner.clear();
    if verify::prepare_report(
        &need,
        &sources,
        &synthetic,
        digest(&receipts(&plans))?,
        digest(&receipts(&decisions))?,
        true,
    )
    .is_ok()
    {
        return Err(ReportError::InvalidWitness);
    }
    ledger.repair()?;
    let expected = verify::prepare_report(
        &need,
        &sources,
        &facts,
        digest(&receipts(&plans))?,
        digest(&receipts(&decisions))?,
        true,
    )?
    .bind_citations(&verified_pins)?;
    if prepared.bytes() != expected.bytes() {
        return Err(ReportError::InvalidWitness);
    }
    worker.cancel_and_wait()?;
    commit_report(
        &mut tools,
        &mut data,
        CommitInput {
            actor: &actor,
            person: &person,
            need: &need,
            task: &task,
            prepared: &prepared,
            pins: verified_pins,
            decision: decision_receipts,
            review,
            generation: selection_generation,
            terminal,
            deadline,
        },
    )
}

struct CommitInput<'a> {
    actor: &'a DurableSession,
    person: &'a ReportController,
    need: &'a ReportNeed,
    task: &'a TaskSpec,
    prepared: &'a PreparedReport,
    pins: storage::VerifiedPins,
    decision: SourcePin,
    review: SourcePin,
    generation: u64,
    terminal: &'a control::Terminal,
    deadline: Instant,
}
fn commit_report(tools: &mut Tools, data: &mut JobData, input: CommitInput<'_>) -> Result<()> {
    let CommitInput {
        actor,
        person,
        need,
        task,
        prepared,
        pins,
        decision,
        review,
        generation,
        terminal,
        deadline,
    } = input;
    check(tools, person, need, data, terminal, deadline)?;
    pins.recheck(&mut data.data, &data.session)?;
    if prepared.need() != need.digest()? || data.memory.generation() != generation {
        return Err(ReportError::RevisionConflict);
    }
    let mut parents = pins.references();
    parents.push(decision.reference.clone());
    parents.sort();
    parents.dedup();
    let prepared_record = data.put(
        "prepared",
        prepared.bytes().to_vec(),
        parents,
        DataKind::Derived,
    )?;
    println!(
        "Verified report {:?}",
        codec::decode(prepared.bytes()).map_err(invalid)?
    );
    terminal.acknowledge("approve", prepared.digest()?, deadline)?;
    check(tools, person, need, data, terminal, deadline)?;
    pins.recheck(&mut data.data, &data.session)?;
    let approval = data.put(
        "approval",
        encoded(&Value::List(vec![
            Value::Bytes(need.digest()?.to_vec()),
            Value::Bytes(prepared.digest()?.to_vec()),
            Value::Unsigned(need.generation),
        ]))?,
        vec![prepared_record.reference.clone(), review.reference.clone()],
        DataKind::Derived,
    )?;
    let count = data.put(
        "count",
        encoded(&Value::Unsigned(prepared.fact_count() as u64))?,
        vec![prepared_record.reference.clone()],
        DataKind::Derived,
    )?;
    let plan = checkpoint(task, prepared.fact_count() as u64, count.reference.clone())?;
    let binding = tools
        .preview_approval(actor, &plan, person.deadline())
        .map_err(invalid)?;
    terminal.acknowledge(
        "checkpoint",
        digest(&Value::List(vec![
            Value::Bytes(prepared.digest()?.to_vec()),
            Value::Unsigned(prepared.fact_count() as u64),
        ]))?,
        deadline,
    )?;
    check(tools, person, need, data, terminal, deadline)?;
    let event = tools
        .approve_report_checkpoint(person, need.digest()?, &plan, binding)
        .map_err(|_| ReportError::Forbidden)?;
    let execution = tools
        .admit(&mut data.data, &data.session, actor, plan.clone(), &[event])
        .map_err(|_| ReportError::OutcomeUnknown)?;
    // Once admitted, an ambiguous effect is never retried or reported as rolled back.
    let commit = (|| -> Result<()> {
        for _ in 0..8 {
            check(tools, person, need, data, terminal, deadline)?;
            if tools
                .step(&mut data.data)
                .map_err(|_| ReportError::OutcomeUnknown)?
                .is_none()
            {
                break;
            }
        }
        let live = tools
            .report(&mut data.data, actor, &execution)
            .map_err(|_| ReportError::InvalidWitness)?;
        storage::validate_checkpoint(
            &live,
            &execution,
            &plan.bindings()[0].action_id,
            prepared.fact_count() as u64,
        )?;
        let current = data
            .data
            .authorized_metadata(&data.session, &count.reference, DataUse::Execute)
            .map_err(storage::data_error)?;
        if current.digest != count.digest
            || current.parents != vec![prepared_record.reference.clone()]
            || data
                .data
                .read(&data.session, &count.reference, DataUse::Execute)
                .map_err(storage::data_error)?
                != encoded(&Value::Unsigned(prepared.fact_count() as u64))?
        {
            return Err(ReportError::InvalidWitness);
        }
        if data.memory.generation() != generation {
            return Err(ReportError::RevisionConflict);
        }
        let now = data.data.trusted_time().map_err(storage::data_error)?;
        let episode = RecordDraft {
            project: data.profile.project.clone(),
            id: storage::id("episode")?,
            revision: rev(1),
            profile: data.profile.id.clone(),
            schema: data.profile.schema.clone(),
            kind: RecordKind::Episode,
            origin: Origin::Observation,
            observed_at: now,
            valid_from: now,
            expires_at: data.expires,
            confidence_bps: None,
            subject: None,
            predicate: None,
            object: None,
            episode_status: Some(EpisodeStatus::Pending),
            episode_text: Some("live report checkpoint".into()),
            sources: vec![pin(&count), pin(&prepared_record), decision, review],
        };
        let episode_projection = data.reference("episode:projection")?;
        data.reserve_memory()?;
        let mut control = storage::memory_control()?;
        data.memory
            .observe(
                tools,
                &mut data.data,
                actor,
                &data.memory_session,
                generation,
                episode,
                &execution,
                episode_projection.clone(),
                &mut control,
            )
            .map_err(|_| ReportError::OutcomeUnknown)?;
        data.data
            .set_policy(
                &data.owner,
                &episode_projection,
                rev(1),
                storage::permissions(),
                false,
            )
            .map_err(storage::data_error)?;
        if data.memory.generation() != generation + 1 {
            return Err(ReportError::RevisionConflict);
        }
        pins.recheck(&mut data.data, &data.session)?;
        check(tools, person, need, data, terminal, deadline)?;
        let permit = ReportPermit {
            prepared: pin(&prepared_record),
            approved: pin(&approval),
            episode: episode_projection,
            count: pin(&count),
            digest: prepared.digest()?,
        };
        permit.commit(data, prepared)?;
        check(tools, person, need, data, terminal, deadline)?;
        pins.recheck(&mut data.data, &data.session)?;
        tools.revoke_report_controller(person).map_err(invalid)?;
        Ok(())
    })();
    commit.map_err(|e| {
        if e == ReportError::OutcomeUnknown {
            e
        } else {
            ReportError::Partial
        }
    })
}
struct ReportPermit {
    prepared: SourcePin,
    approved: SourcePin,
    episode: DataRef,
    count: SourcePin,
    digest: [u8; 32],
}
impl ReportPermit {
    fn commit(self, data: &mut JobData, report: &PreparedReport) -> Result<()> {
        if report.digest() != Ok(self.digest) {
            return Err(ReportError::InvalidWitness);
        }
        for p in [&self.prepared, &self.approved, &self.count] {
            storage::check_pin(&mut data.data, &data.session, p)?;
        }
        let final_record = data.put(
            "final",
            report.bytes().to_vec(),
            vec![
                self.prepared.reference,
                self.approved.reference,
                self.episode,
                self.count.reference,
            ],
            DataKind::Derived,
        )?;
        let released = data
            .data
            .read(&data.session, &final_record.reference, DataUse::Execute)
            .map_err(storage::data_error)?;
        if released != report.bytes() {
            return Err(ReportError::OutcomeUnknown);
        }
        data.put(
            "outcome",
            encoded(&Value::List(vec![
                Value::Text("completed".into()),
                Value::Bytes(self.digest.to_vec()),
            ]))?,
            vec![final_record.reference],
            DataKind::Derived,
        )?;
        Ok(())
    }
}

pub(crate) fn command(args: &[std::ffi::OsString]) -> Result<()> {
    if args.len() != 3 {
        return Err(ReportError::InvalidInput);
    }
    match args[0].to_str() {
        Some("run") => {
            let config = Config::load(Path::new(&args[1]))?;
            run_job(
                &config,
                Path::new(&args[2]),
                &control::Terminal::new(),
                false,
            )
        }
        Some("proof") => {
            let config = Config::load(Path::new(&args[1]))?;
            let root = Path::new(&args[2]);
            new_directory(root)?;
            let terminal = control::Terminal::new();
            run_job(&config, &root.join("positive"), &terminal, false)?;
            run_job(&config, &root.join("stopped"), &terminal, true)?;
            fresh_reader(Path::new(&args[1]), root)
        }
        Some("inspect") => {
            let config = Config::load(Path::new(&args[1]))?;
            inspect_proof(&config, Path::new(&args[2]))
        }
        _ => Err(ReportError::InvalidInput),
    }
}
fn fresh_reader(config: &Path, root: &Path) -> Result<()> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(std::env::current_exe().map_err(invalid)?)
        .args([
            std::ffi::OsStr::new("report"),
            std::ffi::OsStr::new("inspect"),
            config.as_os_str(),
            root.as_os_str(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ReportError::WorkerFailed)?;
    let Some(output) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ReportError::WorkerFailed);
    };
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let result = output.take(4097).read_to_end(&mut bytes);
        let _ = tx.send((result.is_ok(), bytes));
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait().map_err(invalid)? {
            Some(s) => break s,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ReportError::Deadline);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    };
    let (ok, bytes) = rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| ReportError::Deadline)?;
    if !status.success() || !ok || bytes.len() > 4096 || bytes != b"G72 fresh inspection passed\n" {
        return Err(ReportError::InvalidWitness);
    }
    println!("G72 fresh inspection passed");
    Ok(())
}
fn inspect_proof(config: &Config, root: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let dir = root.join("positive");
    let (mut data, owner, session) =
        storage::reopen_data(&dir, config.project.clone(), &config.rows)?;
    let reference = |name: &str| -> Result<DataRef> {
        Ok(DataRef {
            project: config.project.clone(),
            id: storage::id(name)?,
        })
    };
    let final_ref = reference("final")?;
    let bytes = data
        .read(&session, &final_ref, DataUse::Execute)
        .map_err(storage::data_error)?;
    let fields = wire::decode_closed(&bytes)?;
    let fields = wire::list(&fields)?;
    if fields.len() != 7
        || wire::text(&fields[0])? != "report:v1"
        || wire::list(&fields[2])?.len() != 2
        || wire::list(&fields[6])?.is_empty()
    {
        return Err(ReportError::InvalidWitness);
    }
    let prepared = data
        .read(&session, &reference("prepared")?, DataUse::Execute)
        .map_err(storage::data_error)?;
    if prepared != bytes {
        return Err(ReportError::InvalidWitness);
    }
    let actual = digest(&wire::decode_closed(&bytes)?)?;
    let outcome = data
        .read(&session, &reference("outcome")?, DataUse::Execute)
        .map_err(storage::data_error)?;
    if codec::decode(&outcome).map_err(invalid)?
        != Value::List(vec![
            Value::Text("completed".into()),
            Value::Bytes(actual.to_vec()),
        ])
    {
        return Err(ReportError::InvalidWitness);
    }
    let withdrawn = reference(&config.rows[0].id)?;
    let revision = data
        .inspect(&owner, &withdrawn)
        .map_err(storage::data_error)?
        .revision;
    data.withdraw(&owner, &withdrawn, revision)
        .map_err(storage::data_error)?;
    if data.read(&session, &final_ref, DataUse::Execute).is_ok() {
        return Err(ReportError::InvalidWitness);
    }
    let now = data.trusted_time().map_err(storage::data_error)?;
    let expiry = now.checked_add(1000).ok_or(ReportError::Limit)?;
    let expired_source = reference("expire:source")?;
    let expired_output = reference("expire:output")?;
    for (r, parents) in [
        (expired_source.clone(), vec![]),
        (expired_output.clone(), vec![expired_source]),
    ] {
        let meta = data
            .collect(
                &session,
                DataDraft {
                    reference: r.clone(),
                    kind: DataKind::Derived,
                    parents,
                    payload: vec![1],
                    expires_at: expiry,
                },
            )
            .map_err(storage::data_error)?;
        data.set_policy(&owner, &r, meta.revision, storage::permissions(), false)
            .map_err(storage::data_error)?;
    }
    let wait = expiry.saturating_sub(data.trusted_time().map_err(storage::data_error)?);
    std::thread::sleep(Duration::from_millis(wait.saturating_add(1)));
    if data
        .read(&session, &expired_output, DataUse::Execute)
        .is_ok()
    {
        return Err(ReportError::InvalidWitness);
    }
    data.close();
    let (mut stopped, stop_owner, stop_session) =
        storage::reopen_data(&root.join("stopped"), config.project.clone(), &config.rows)?;
    let status = stopped
        .read(&stop_session, &reference("outcome")?, DataUse::Execute)
        .map_err(storage::data_error)?;
    if codec::decode(&status).map_err(invalid)? != Value::Text("stopped_before_effect".into())
        || stopped.inspect(&stop_owner, &final_ref).err()
            != Some(eylema_host::project_data::DataError::NotFound)
    {
        return Err(ReportError::InvalidWitness);
    }
    let journal = Journal::open(
        NativeStorage::open_existing(&root.join("stopped/tools.eylj"), storage::sync)
            .map_err(invalid)?,
        NativeHash,
        Default::default(),
    )
    .map_err(invalid)?;
    let (mut host, owner) = DurableHost::open(journal, NativeServices).map_err(invalid)?;
    let actor = host.enroll(&owner, tool_grant(false)?).map_err(invalid)?;
    if host
        .task(
            &actor,
            &TaskId::new(format!("{}:stop", config.task)).map_err(invalid)?,
        )
        .map_err(invalid)?
        .value
        != 0
        || Instant::now() >= deadline
    {
        return Err(ReportError::InvalidWitness);
    }
    println!("G72 fresh inspection passed");
    Ok(())
}
