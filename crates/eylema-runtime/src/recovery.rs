//! Privileged host recovery, never a request-supplied authority surface.
use super::*;
impl<S: HostServices> Runtime<S> {
    /// Retire all historical enrollment and conservatively terminate unfinished
    /// executions. The host must durably commit this transition before exposing
    /// a restored instance. Confirmed terminal records are left unchanged.
    /// `inflight_step` means the first FIFO step has an uncommitted journal intent;
    /// its next effect cannot be inferred from the last confirmed owned state.
    pub fn recover_unfinished(
        &mut self,
        host: &HostKey,
        inflight_step: bool,
    ) -> Result<Vec<ExecutionId>> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        let now = self.services.now()?; // Fail before any recovery mutation.
        let pending: Vec<_> = self.queue.iter().cloned().collect();
        for (index, id) in pending.iter().enumerate() {
            let mut entry = self.executions.remove(id).expect("owned queued entry");
            let state = self
                .tasks
                .get(&entry.draft.task_id)
                .expect("owned leased task");
            let value = state.snapshot.value;
            if inflight_step && index == 0 && entry.draft.state != ExecutionState::CancelRequested {
                let action = &entry.proposal.fields().actions[entry.next];
                entry.draft.effects.push(
                    EffectRecord::new(EffectDraft {
                        action_id: action.fields().action_id.clone(),
                        outcome: EffectOutcome::Unknown,
                        evidence: vec![],
                    })
                    .expect("owned next action, unknown without invented evidence"),
                );
            }
            entry.draft.state = ExecutionState::OutcomeUnknown;
            entry.draft.effective_stop_evidence = None;
            entry.draft.error = Some(ProtocolError::new(ErrorCode::OutcomeUnknown,
                "Restart retained last confirmed owned state; unfinished outcome requires reconciliation".into(),
                None, false, entry.draft.context.correlation_id.clone(), None).expect("fixed recovery error"));
            let event = entry
                .event_ids
                .pop_back()
                .expect("reserved terminal recovery ID");
            let evidence = self.emit(
                event,
                EventMetadata {
                    task: &entry.draft.task_id,
                    execution: Some(id),
                    revision: entry.expected_live,
                    context: &entry.draft.context,
                    kind: EventKind::ExecutionOutcomeUnknown,
                },
                now,
                (value, value),
            );
            entry.draft.result_evidence = vec![evidence];
            ExecutionRecord::new(entry.draft.clone(), &entry.proposal)
                .expect("owned recovery invariants");
            self.tasks
                .get_mut(&entry.draft.task_id)
                .expect("owned leased task")
                .lease = None;
            self.executions.insert(id.clone(), entry);
        }
        self.queue.clear();
        // Retirement is an epoch boundary, not a fabricated human revocation.
        // Historical approvals/governance/revocation audits keep their identities.
        self.retired_slots.extend(0..self.grants.len());
        self.human_slots.clear();
        Ok(pending)
    }
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn number(n: usize) -> Value {
    Value::Unsigned(n as u64)
}
fn optional(s: Option<&str>) -> Value {
    s.map_or(Value::Null, text)
}
fn scope(s: &ResourceScope) -> Value {
    Value::List(s.resources().map(|id| text(id.as_str())).collect())
}
fn oversight(o: &OversightRequirements) -> Value {
    Value::List(vec![
        Value::List(o.checkpoints().iter().map(|c| text(c.as_str())).collect()),
        Value::Bool(o.requires_observation()),
        Value::Bool(o.requires_intervention()),
        optional(o.governance().map(|g| g.as_str())),
    ])
}
fn revocation(r: &Revocation) -> Value {
    Value::List(vec![
        text(r.verified_principal.as_str()),
        Value::Unsigned(r.timestamp.unix_ms()),
    ])
}
fn policy(p: &PolicyChange) -> Value {
    Value::List(vec![
        text(p.decision_id.as_str()),
        text(p.task_id.as_str()),
        text(p.verified_principal.as_str()),
        Value::Unsigned(p.previous_revision.get()),
        Value::Unsigned(p.accepted_revision.get()),
        Value::Unsigned(p.timestamp.unix_ms()),
    ])
}
fn receipt(r: &CancellationReceipt) -> Value {
    Value::List(vec![
        text(r.execution_id.as_str()),
        text(r.receipt_event.as_str()),
        text(r.verified_principal.as_str()),
        Value::Unsigned(r.requested_at.unix_ms()),
    ])
}
fn binding(b: &ApprovalBinding) -> Value {
    Value::List(vec![
        text(b.task_id.as_str()),
        text(b.proposal_id.as_str()),
        Value::Unsigned(b.plan_revision.get()),
        Value::Bytes(b.payload_sha256.to_vec()),
        Value::Unsigned(b.state_revision.get()),
        Value::Unsigned(b.policy_revision.get()),
        scope(&b.resource_scope),
        Value::List(b.checkpoints.iter().map(|c| text(c.as_str())).collect()),
        Value::Unsigned(b.expires_at.unix_ms()),
    ])
}
fn wire(record: &codec::WireRecord) -> Result<Value> {
    codec::encode_record(record)
        .and_then(|b| codec::decode(&b))
        .map_err(|_| ErrorCode::BudgetExceeded)
}
fn fold<S: HostServices>(
    service: &mut S,
    digest: &mut [u8; 32],
    section: &str,
    value: Value,
) -> Result<()> {
    let encoded = codec::encode(&Value::List(vec![text(section), value]))
        .map_err(|_| ErrorCode::BudgetExceeded)?;
    if encoded.len() > codec::MAX_WIRE_BYTES - 32 {
        return Err(ErrorCode::BudgetExceeded);
    }
    let mut input = Vec::with_capacity(32 + encoded.len());
    input.extend_from_slice(digest);
    input.extend_from_slice(&encoded);
    *digest = service.sha256(&input)?;
    Ok(())
}
impl<S: HostServices> Runtime<S> {
    /// Complete bounded semantic state fingerprint for trusted replay validation.
    /// Uses standard host SHA256 over canonical sectioned chunks, never Debug
    /// strings, credentials or a new hash implementation. Host must compare the
    /// committed fingerprint before exposing reconstructed state.
    pub fn recovery_digest(&mut self, host: &HostKey) -> Result<[u8; 32]> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        let mut digest = self
            .services
            .sha256(b"Eylema owned runtime recovery state v1")?;
        fold(
            &mut self.services,
            &mut digest,
            "limits",
            Value::List(vec![
                number(self.limits.tasks),
                number(self.limits.executions),
                number(self.limits.sessions),
                number(self.limits.queued),
                number(self.limits.events),
                number(self.limits.idempotency),
                number(self.limits.resident_wire_bytes),
                number(self.charged_event_slots),
                number(self.resident_wire_bytes),
            ]),
        )?;
        for g in &self.grants {
            fold(
                &mut self.services,
                &mut digest,
                "grant",
                Value::List(vec![
                    text(g.principal.as_str()),
                    scope(&g.resource_scope),
                    Value::List(
                        g.permissions
                            .iter()
                            .map(|p| Value::Unsigned(*p as u64))
                            .collect(),
                    ),
                    Value::Unsigned(g.policy_revision.get()),
                ]),
            )?;
        }
        for slot in &self.human_slots {
            fold(&mut self.services, &mut digest, "human", number(*slot))?;
        }
        for slot in &self.retired_slots {
            fold(&mut self.services, &mut digest, "retired", number(*slot))?;
        }
        for (slot, r) in &self.revoked_slots {
            fold(
                &mut self.services,
                &mut digest,
                "revoked",
                Value::List(vec![number(*slot), revocation(r)]),
            )?;
        }
        for (id, a) in &self.approvals {
            fold(
                &mut self.services,
                &mut digest,
                "approval",
                Value::List(vec![
                    text(id.as_str()),
                    binding(&a.binding),
                    text(a.verified_principal.as_str()),
                    Value::Unsigned(a.decided_at.unix_ms()),
                    a.revocation.as_ref().map_or(Value::Null, revocation),
                    number(a.issuer.slot),
                ]),
            )?;
        }
        for (id, g) in &self.governance {
            fold(
                &mut self.services,
                &mut digest,
                "governance",
                Value::List(vec![
                    text(id.as_str()),
                    text(g.task_id.as_str()),
                    scope(&g.resource_scope),
                    Value::Unsigned(g.policy_revision.get()),
                    number(g.issuer.slot),
                    policy(&g.decision),
                ]),
            )?;
        }
        for p in &self.policy_changes {
            fold(&mut self.services, &mut digest, "policy", policy(p))?;
        }
        for (id, t) in &self.tasks {
            fold(
                &mut self.services,
                &mut digest,
                "task",
                Value::List(vec![
                    text(id.as_str()),
                    wire(&codec::WireRecord::Task(t.snapshot.task.clone()))?,
                    Value::Unsigned(t.snapshot.value),
                    Value::Unsigned(t.snapshot.performed_steps),
                    Value::Unsigned(t.snapshot.charged_steps),
                    Value::Unsigned(t.snapshot.charged_events),
                    text(t.owner.as_str()),
                    optional(t.lease.as_ref().map(|v| v.as_str())),
                    Value::Unsigned(t.policy_revision.get()),
                    oversight(&t.oversight),
                ]),
            )?;
        }
        for (id, e) in &self.executions {
            let record = ExecutionRecord::new(e.draft.clone(), &e.proposal)
                .map_err(|_| ErrorCode::OutcomeUnknown)?;
            fold(
                &mut self.services,
                &mut digest,
                "execution",
                Value::List(vec![
                    text(id.as_str()),
                    wire(&codec::WireRecord::Execution {
                        task: e.task.clone(),
                        proposal: e.proposal.clone(),
                        execution: Box::new(record),
                    })?,
                    number(e.session.slot),
                    number(e.next),
                    Value::Unsigned(e.expected_live.get()),
                    Value::List(e.event_ids.iter().map(|i| text(i.as_str())).collect()),
                    Value::List(e.cancel_ids.iter().map(|i| text(i.as_str())).collect()),
                    e.cancellation.as_ref().map_or(Value::Null, receipt),
                    Value::List(e.approval_ids.iter().map(|i| text(i.as_str())).collect()),
                    Value::Bytes(e.payload_hash.to_vec()),
                ]),
            )?;
        }
        for id in &self.queue {
            fold(&mut self.services, &mut digest, "queue", text(id.as_str()))?;
        }
        for (key, r) in &self.replays {
            fold(
                &mut self.services,
                &mut digest,
                "replay",
                Value::List(vec![
                    text(key.0.as_str()),
                    text(key.1.as_str()),
                    Value::List(key.2.iter().map(|i| text(i.as_str())).collect()),
                    text(key.3.as_str()),
                    Value::Bytes(r.hash.to_vec()),
                    text(r.execution.as_str()),
                ]),
            )?;
        }
        for ((id, rev), hash) in &self.plans {
            fold(
                &mut self.services,
                &mut digest,
                "plan",
                Value::List(vec![
                    text(id.as_str()),
                    Value::Unsigned(*rev),
                    Value::Bytes(hash.to_vec()),
                ]),
            )?;
        }
        for event in &self.events {
            fold(
                &mut self.services,
                &mut digest,
                "event",
                wire(&codec::WireRecord::Event(event.clone()))?,
            )?;
        }
        for (id, o) in &self.observations {
            fold(
                &mut self.services,
                &mut digest,
                "observation",
                Value::List(vec![
                    text(id.as_str()),
                    text(o.event_id.as_str()),
                    text(o.task_id.as_str()),
                    optional(o.execution_id.as_ref().map(|v| v.as_str())),
                    Value::Unsigned(o.timestamp.unix_ms()),
                    Value::Unsigned(o.before),
                    Value::Unsigned(o.after),
                ]),
            )?;
        }
        for id in &self.event_ids {
            fold(
                &mut self.services,
                &mut digest,
                "reserved_event",
                text(id.as_str()),
            )?;
        }
        Ok(digest)
    }
}
