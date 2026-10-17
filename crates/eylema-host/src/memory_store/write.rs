//! Source projections synchronize before canonical memory events; no implicit policy grant.
use super::*;
use eylema_contracts::{
    codec::{self, Value},
    project_data::{DataDraft, DataId, DataKind},
};
impl<T: Storage, H: Sha256> MemoryStore<T, H> {
    /// Render exact canonical projection bytes without granting authority or mutating storage.
    pub fn preview_projection(
        profile: &StoreProfile,
        draft: &RecordDraft,
    ) -> MemoryResult<Vec<u8>> {
        profile.validate()?;
        draft.validate(profile)?;
        wire::projection(draft)
    }
    fn cas(
        &self,
        generation: u64,
        id: &RecordId,
        expected: Option<Revision>,
        next: Revision,
    ) -> MemoryResult<()> {
        if self.poisoned {
            return Err(MemoryStoreError::NeedsReopen);
        }
        let actual = self.records.get(id).map(|r| r.record.draft.revision);
        if generation != self.generation
            || actual != expected
            || expected.map_or(Some(1), |r| r.get().checked_add(1)) != Some(next.get())
        {
            return Err(MemoryStoreError::Conflict);
        }
        Ok(())
    }
    fn joins(&self, d: &RecordDraft) -> MemoryResult<()> {
        let entity = |id: &RecordId| {
            self.records
                .get(id)
                .is_some_and(|r| !r.deleted && r.record.draft.kind == RecordKind::Entity)
        };
        if matches!(d.kind, RecordKind::Fact | RecordKind::Relation)
            && d.subject.as_ref().is_none_or(|s| !entity(s))
        {
            return Err(MemoryStoreError::InvalidRequest);
        }
        if let Some(TypedObject::Entity(id)) = &d.object
            && !entity(id)
        {
            return Err(MemoryStoreError::InvalidRequest);
        }
        Ok(())
    }
    fn state_capacity(&self, s: &Stored, c: &mut MemoryControl, prepaid: bool) -> MemoryResult<()> {
        if !prepaid {
            c.inner.charge(
                (self.records.len() + 1)
                    .checked_mul(32768)
                    .ok_or(MemoryStoreError::Limit)?,
            )?;
        }
        c.inner.check()?;
        let mut active = usize::from(!s.deleted);
        let mut entities = usize::from(!s.deleted && s.record.draft.kind == RecordKind::Entity);
        let mut bytes = 0usize;
        for (old_id, old) in &self.records {
            if old_id == &s.record.draft.id {
                continue;
            }
            active += usize::from(!old.deleted);
            entities += usize::from(!old.deleted && old.record.draft.kind == RecordKind::Entity);
            bytes = bytes
                .checked_add(
                    codec::encode(&wire::record(old))
                        .map_err(|_| MemoryStoreError::InvalidRequest)?
                        .len()
                        .checked_mul(4)
                        .ok_or(MemoryStoreError::Limit)?
                        + 2048,
                )
                .ok_or(MemoryStoreError::Limit)?;
        }
        let n = codec::encode(&wire::record(s))
            .map_err(|_| MemoryStoreError::InvalidRequest)?
            .len();
        bytes = bytes
            .checked_add(n.checked_mul(4).ok_or(MemoryStoreError::Limit)? + 2048)
            .ok_or(MemoryStoreError::Limit)?;
        if active > 128 || entities > 64 || bytes > 8 * 1024 * 1024 {
            return Err(MemoryStoreError::Limit);
        }
        Ok(())
    }
    pub(super) fn apply(
        &mut self,
        v: &Value,
        c: &mut MemoryControl,
        validated_live_write: bool,
    ) -> MemoryResult<()> {
        let a = wire::array(v, 8)?;
        if wire::string(&a[0])? != "memory.event"
            || wire::uint(&a[1])? != 1
            || wire::uint(&a[2])?
                != self
                    .generation
                    .checked_add(1)
                    .ok_or(MemoryStoreError::Limit)?
        {
            return Err(MemoryStoreError::InvalidRequest);
        }
        let op = wire::string(&a[3])?;
        let id = DataId::new(wire::string(&a[4])?).map_err(|_| MemoryStoreError::InvalidRequest)?;
        let expected = wire::expected(&a[5])?;
        let next = wire::revision(&a[6])?;
        self.cas(self.generation, &id, expected, next)?;
        if op == "tombstone" {
            if a[7] != Value::Null {
                return Err(MemoryStoreError::InvalidRequest);
            }
            let old = self
                .records
                .get_mut(&id)
                .ok_or(MemoryStoreError::InvalidRequest)?;
            if old.deleted {
                return Err(MemoryStoreError::InvalidRequest);
            }
            old.deleted = true;
            old.record.draft.revision = next;
        } else {
            let s = wire::decode(&a[7], &self.profile)?;
            if s.record.draft.id != id || s.record.draft.revision != next {
                return Err(MemoryStoreError::InvalidRequest);
            }
            match op {
                "put" if s.record.admission == Admission::Proposed => (),
                "review" if s.record.admission == Admission::ExplicitlyReviewed => {
                    let old = self
                        .records
                        .get(&id)
                        .ok_or(MemoryStoreError::InvalidRequest)?;
                    if old.deleted {
                        return Err(MemoryStoreError::InvalidRequest);
                    }
                    let mut previous = old.record.draft.clone();
                    previous.revision = next;
                    previous.sources = s.record.draft.sources.clone();
                    if previous != s.record.draft
                        || old
                            .record
                            .draft
                            .sources
                            .iter()
                            .any(|p| !s.record.draft.sources.contains(p))
                        || s.vector != old.vector
                    {
                        return Err(MemoryStoreError::InvalidRequest);
                    }
                }
                "observe"
                    if s.record.admission == Admission::VerifiedOutcome
                        && s.record.draft.origin == Origin::Observation
                        && s.record.draft.kind == RecordKind::Episode => {}
                _ => return Err(MemoryStoreError::InvalidRequest),
            };
            self.joins(&s.record.draft)?;
            if !validated_live_write && let Some(vector) = &s.vector {
                eylema_retrieval::score::unit_norm(vector, 1024, c.inner_mut())?;
            }
            self.state_capacity(&s, c, validated_live_write)?;
            self.records.insert(id, s);
        }
        self.generation += 1;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn propose<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        expected_generation: u64,
        expected_revision: Option<Revision>,
        draft: RecordDraft,
        vector: Option<&[f32]>,
        projection: DataRef,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryReceipt> {
        self.persist(
            data,
            session,
            expected_generation,
            expected_revision,
            draft,
            vector,
            projection,
            Admission::Proposed,
            "put",
            c,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn persist<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        expected_generation: u64,
        expected_revision: Option<Revision>,
        draft: RecordDraft,
        vector: Option<&[f32]>,
        projection: DataRef,
        admission: Admission,
        op: &str,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryReceipt> {
        self.session(data, session)?;
        self.cas(
            expected_generation,
            &draft.id,
            expected_revision,
            draft.revision,
        )?;
        draft.validate(&self.profile)?;
        if self
            .records
            .get(&draft.id)
            .is_some_and(|old| old.record.draft.kind != draft.kind)
        {
            return Err(MemoryStoreError::InvalidRequest);
        }
        self.joins(&draft)?;
        if projection.project != self.profile.project {
            return Err(MemoryStoreError::Forbidden);
        }
        if let Some(v) = vector {
            if v.len() != 1024 {
                return Err(MemoryStoreError::Limit);
            }
            eylema_retrieval::score::unit_norm(v, 1024, c.inner_mut())?;
        }
        c.inner.charge(8192)?;
        let body = wire::projection(&draft)?;
        let mut s = Stored {
            record: Record {
                draft,
                admission,
                projection: SourcePin {
                    reference: projection.clone(),
                    put_digest: [0; 32],
                },
            },
            vector: vector.map(|v| v.to_vec()),
            deleted: false,
        };
        self.state_capacity(&s, c, false)?;
        let next = self
            .generation
            .checked_add(1)
            .ok_or(MemoryStoreError::Limit)?;
        let preflight = wire::event(
            next,
            op,
            &s.record.draft.id,
            expected_revision,
            s.record.draft.revision,
            Some(&s),
        );
        let encoded = codec::encode(&preflight).map_err(|_| MemoryStoreError::InvalidRequest)?;
        guard::PayloadGuard::validate(&encoded)?;
        self.journal.check_append_capacity(&[encoded.len()])?;
        c.prepay_calls(s.record.draft.sources.len() * 2 + 3)?;
        // Reserve the second state scan before either projection or event IO.
        // Vector admission above is retained unchanged for this live write.
        c.inner.charge(
            (self.records.len() + 1)
                .checked_mul(32768)
                .ok_or(MemoryStoreError::Limit)?,
        )?;
        let reserved = self.gate.begin(Mode::Append, c)?;
        let result = (|| -> MemoryResult<MemoryReceipt> {
            let now = c.call(|| data.trusted_time())?;
            if s.record.draft.observed_at > now || s.record.draft.expires_at <= now {
                return Err(MemoryStoreError::Forbidden);
            }
            for p in &s.record.draft.sources {
                self.pin(data, &session.source, p, DataUse::Collect, c)?
            }
            let collected = c.call(|| {
                data.collect(
                    &session.source,
                    DataDraft {
                        reference: projection,
                        kind: DataKind::Derived,
                        parents: s
                            .record
                            .draft
                            .sources
                            .iter()
                            .map(|p| p.reference.clone())
                            .collect(),
                        payload: body,
                        expires_at: s.record.draft.expires_at,
                    },
                )
            });
            let meta = match collected {
                Ok(meta) => meta,
                Err(_) => {
                    self.poisoned = true;
                    return Err(MemoryStoreError::WriteOutcomeUnknown);
                }
            };
            s.record.projection.put_digest = meta.digest;
            let event = wire::event(
                next,
                op,
                &s.record.draft.id,
                expected_revision,
                s.record.draft.revision,
                Some(&s),
            );
            let receipt = match self.journal.append(&event) {
                Ok(r) => r,
                Err(_) => {
                    self.poisoned = true;
                    return Err(MemoryStoreError::WriteOutcomeUnknown);
                }
            };
            if self.apply(&event, c, true).is_err() {
                self.poisoned = true;
                return Err(MemoryStoreError::WriteOutcomeUnknown);
            }
            let final_check = (|| -> MemoryResult<()> {
                let now = c.call(|| data.trusted_time())?;
                if now >= s.record.draft.expires_at {
                    return Err(MemoryStoreError::Forbidden);
                }
                for p in &s.record.draft.sources {
                    self.pin(data, &session.source, p, DataUse::Collect, c)?
                }
                c.inner.check()?;
                Ok(())
            })();
            if final_check.is_err() {
                self.poisoned = true;
                return Err(MemoryStoreError::WriteOutcomeUnknown);
            }
            Ok(MemoryReceipt {
                generation: next,
                revision: s.record.draft.revision,
                journal_digest: receipt.digest,
            })
        })();
        self.gate.end(reserved, c);
        c.credits = 0;
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub fn review<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        owner: &MemoryOwner,
        data: &mut ProjectStore<D, F, C>,
        data_owner: &Owner,
        session: &MemorySession,
        expected_generation: u64,
        id: &RecordId,
        expected_revision: Revision,
        review_source: SourcePin,
        projection: DataRef,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryReceipt> {
        self.owner(owner)?;
        self.session(data, session)?;
        c.call(|| data.retrieval_binding(data_owner).map(|_| ()))?;
        let next = Revision::new(
            expected_revision
                .get()
                .checked_add(1)
                .ok_or(MemoryStoreError::Limit)?,
        )
        .map_err(|_| MemoryStoreError::Limit)?;
        self.cas(expected_generation, id, Some(expected_revision), next)?;
        c.inner.charge(8192)?;
        let old = self
            .records
            .get(id)
            .filter(|s| !s.deleted)
            .ok_or(MemoryStoreError::Conflict)?;
        let mut draft = old.record.draft.clone();
        let vector = old.vector.clone();
        draft.revision = next;
        if !draft.sources.contains(&review_source) {
            draft.sources.push(review_source)
        }
        draft.sources.sort_by(|a, b| a.reference.cmp(&b.reference));
        self.persist(
            data,
            session,
            expected_generation,
            Some(expected_revision),
            draft,
            vector.as_deref(),
            projection,
            Admission::ExplicitlyReviewed,
            "review",
            c,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn tombstone<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        owner: &MemoryOwner,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        expected_generation: u64,
        id: &RecordId,
        expected_revision: Revision,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryReceipt> {
        self.owner(owner)?;
        self.session(data, session)?;
        let next = Revision::new(
            expected_revision
                .get()
                .checked_add(1)
                .ok_or(MemoryStoreError::Limit)?,
        )
        .map_err(|_| MemoryStoreError::Limit)?;
        self.cas(expected_generation, id, Some(expected_revision), next)?;
        let old = self
            .records
            .get(id)
            .filter(|s| !s.deleted)
            .ok_or(MemoryStoreError::Conflict)?;
        c.inner.charge(8192)?;
        let pins = old.record.draft.sources.clone();
        c.prepay_calls(pins.len() * 2 + 2)?;
        let event = wire::event(
            self.generation + 1,
            "tombstone",
            id,
            Some(expected_revision),
            next,
            None,
        );
        let encoded = codec::encode(&event).map_err(|_| MemoryStoreError::InvalidRequest)?;
        self.journal.check_append_capacity(&[encoded.len()])?;
        let reserve = self.gate.begin(Mode::Append, c)?;
        let result = (|| -> MemoryResult<_> {
            c.call(|| data.trusted_time())?;
            for p in &pins {
                self.pin(data, &session.source, p, DataUse::Collect, c)?
            }
            let receipt = match self.journal.append(&event) {
                Ok(r) => r,
                Err(_) => {
                    self.poisoned = true;
                    return Err(MemoryStoreError::WriteOutcomeUnknown);
                }
            };
            if self.apply(&event, c, true).is_err() {
                self.poisoned = true;
                return Err(MemoryStoreError::WriteOutcomeUnknown);
            }
            let handoff = (|| -> MemoryResult<()> {
                c.call(|| data.trusted_time())?;
                for p in &pins {
                    self.pin(data, &session.source, p, DataUse::Collect, c)?
                }
                Ok(())
            })();
            if handoff.is_err() {
                self.poisoned = true;
                return Err(MemoryStoreError::WriteOutcomeUnknown);
            }
            Ok(MemoryReceipt {
                generation: self.generation,
                revision: next,
                journal_digest: receipt.digest,
            })
        })();
        self.gate.end(reserve, c);
        c.credits = 0;
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub fn observe<
        TT: Storage,
        TH: Sha256,
        S: eylema_runtime::HostServices,
        D: Storage,
        F: Sha256,
        C: DataClock,
    >(
        &mut self,
        tools: &crate::tool::ToolHost<TT, TH, S>,
        data: &mut ProjectStore<D, F, C>,
        tool_session: &crate::DurableSession,
        session: &MemorySession,
        expected_generation: u64,
        mut draft: RecordDraft,
        execution: &eylema_contracts::ExecutionId,
        projection: DataRef,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryReceipt> {
        self.session(data, session)?;
        draft.validate(&self.profile)?;
        let observed = c.call(|| {
            tools
                .memory_observation(data, tool_session, execution)
                .map_err(|_| MemoryStoreError::Forbidden)
        })?;
        for reference in observed.controls() {
            if !draft.sources.iter().any(|p| &p.reference == reference) {
                if draft.sources.len() >= 8 {
                    return Err(MemoryStoreError::Limit);
                }
                let m = c.call(|| {
                    data.authorized_metadata(&session.source, reference, DataUse::Collect)
                })?;
                draft.sources.push(SourcePin {
                    reference: reference.clone(),
                    put_digest: m.digest,
                })
            }
        }
        draft.sources.sort_by(|a, b| a.reference.cmp(&b.reference));
        draft.kind = RecordKind::Episode;
        draft.origin = Origin::Observation;
        draft.subject = None;
        draft.predicate = None;
        draft.object = None;
        draft.episode_status = Some(match observed.status() {
            crate::tool::ToolStatus::Completed => EpisodeStatus::Completed,
            crate::tool::ToolStatus::Failed => EpisodeStatus::Failed,
            crate::tool::ToolStatus::Partial => EpisodeStatus::Partial,
            crate::tool::ToolStatus::Stopped => EpisodeStatus::Stopped,
            crate::tool::ToolStatus::Pending => EpisodeStatus::Pending,
            crate::tool::ToolStatus::Unknown => EpisodeStatus::Unknown,
        });
        draft.episode_text = Some(observed.text().into());
        draft.observed_at = c.call(|| data.trusted_time())?;
        let expected = self.records.get(&draft.id).map(|r| r.record.draft.revision);
        self.persist(
            data,
            session,
            expected_generation,
            expected,
            draft,
            None,
            projection,
            Admission::VerifiedOutcome,
            "observe",
            c,
        )
    }
}
