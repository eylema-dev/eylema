use super::{source::*, types::*};
#[test]
fn source_parser_is_closed_and_bounded() {
    let source = parse_document(b"language=en;site=North;owner=Mira;checklist=C7\n").unwrap();
    assert_eq!(source.owner.as_deref(), Some("Mira"));
    assert_eq!(source.checklist.as_deref(), Some("C7"));
    for invalid in [
        "language=en;site=North;owner=Mira;owner=Leon",
        "language=en;site=North;run=stop",
        "language=en;site=North",
        "language=en;site=North;owner=\u{1b}stop",
        "language=zz;site=North;owner=Mira",
    ] {
        assert_eq!(
            parse_document(invalid.as_bytes()).err(),
            Some(ReportError::InvalidInput)
        );
    }
    assert_eq!(
        parse_document(&vec![b'x'; 1025]).err(),
        Some(ReportError::Limit)
    );
    assert!(parse_document(b"language=tr;site=G\xc3\xbcney;owner=Ece;checklist=C9\n").is_ok());
    assert!(parse_document(b"language=en;site=West;owner=Leon\n").is_ok());
}
#[test]
fn need_is_closed_and_repair_is_finite() {
    let need = ReportNeed::new(
        "project:a",
        "task:report-a",
        1,
        vec![("en".into(), "North".into())],
    )
    .unwrap();
    assert_ne!(need.digest().unwrap(), [0; 32]);
    assert!(ReportNeed::new("project:a", "task:report-a", 0, vec![]).is_err());
    let mut budget = JobLedger::new(100).unwrap();
    assert!(budget.reserve(60).is_ok());
    assert!(budget.reserve(41).is_err());
    assert!(budget.repair().is_ok());
    assert!(budget.repair().is_err());
}
#[test]
fn wire_extents_precede_allocation() {
    use super::wire::*;
    use eylema_contracts::codec::{self, Value};
    let valid = codec::encode(&Value::List(vec![Value::Text("ok".into())])).unwrap();
    assert!(decode_closed(&valid).is_ok());
    assert!(decode_closed(&codec::encode(&Value::Map(vec![])).unwrap()).is_err());
    assert!(decode_closed(&codec::encode(&Value::List(vec![Value::Null; 33])).unwrap()).is_err());
    let mut nested = Value::Null;
    for _ in 0..10 {
        nested = Value::List(vec![nested]);
    }
    assert!(decode_closed(&codec::encode(&nested).unwrap()).is_err());
    let mut io = FrameBudget::default();
    assert!(
        io.read(&mut std::io::Cursor::new(8193u32.to_be_bytes()))
            .is_err()
    );
    let mut io = FrameBudget::default();
    assert!(io.read(&mut std::io::Cursor::new([0, 0, 0, 8, 1])).is_err());
    for _ in 0..255 {
        assert!(io.write(&mut std::io::sink(), &valid).is_ok());
    }
    assert!(io.write(&mut std::io::sink(), &valid).is_err());
}
#[test]
fn planner_input_dependency() {
    use super::models::decision_query;
    assert!(decision_query("North owner checklist", "").is_err());
    let query = decision_query("North owner checklist", "Check sources.").unwrap();
    assert_eq!(
        query,
        "Request: North owner checklist\nPlanning proposal: Check sources."
    );
    assert!(decision_query("North owner checklist", &"x".repeat(8193)).is_err());
}
#[test]
fn terminal_and_worker_failure_preserve_budget() {
    use super::control::Terminal;
    assert_eq!(
        Terminal::synthetic_input(vec![]).poll_stop().err(),
        Some(ReportError::Stopped)
    );
    assert_eq!(
        Terminal::synthetic_input(vec![Ok("stop".into())])
            .poll_stop()
            .err(),
        Some(ReportError::Stopped)
    );
    assert_eq!(
        Terminal::synthetic_input(vec![Ok("approve forged".into())])
            .poll_stop()
            .err(),
        Some(ReportError::InvalidInput)
    );
    assert_eq!(
        Terminal::synthetic_input(vec![])
            .acknowledge("review", [1; 32], std::time::Instant::now())
            .err(),
        Some(ReportError::Deadline)
    );
    let mut b = JobLedger::new(100).unwrap();
    b.reserve(80).unwrap();
    assert!(b.reserve(21).is_err());
    assert!(b.reserve(20).is_ok());
    assert!(b.reserve(1).is_err());
}
#[test]
fn one_profile_resident_and_root_budget() {
    use super::models::*;
    assert!(
        ModelRequest::Plan(vec!["x".repeat(8193)])
            .validate()
            .is_err()
    );
    assert!(
        ModelRequest::Documents(vec!["x".into(); 10])
            .validate()
            .is_err()
    );
    assert!(ModelRequest::Queries(vec![]).validate().is_err());
    assert!(ModelRequest::Plan(vec!["North".into()]).validate().is_ok());
}
#[test]
fn worker_identity_and_replay_are_closed() {
    use super::worker::validate_envelope;
    use eylema_contracts::codec::Value;
    let packet = Value::List(vec![
        Value::Bytes(vec![7; 32]),
        Value::Unsigned(1),
        Value::Null,
    ]);
    assert!(validate_envelope(&packet, [7; 32], 1).is_ok());
    assert!(validate_envelope(&packet, [8; 32], 1).is_err());
    assert!(validate_envelope(&packet, [7; 32], 2).is_err());
}
#[test]
fn stop_cancels_registered_controls_and_prevents_dispatch() {
    use super::control::Controls;
    use eylema_model::cpu::{CpuControl, CpuWorkGroup};
    use std::time::{Duration, Instant};
    let c = Controls::default();
    let cpu = CpuControl::new_job(
        Instant::now() + Duration::from_secs(1),
        100,
        CpuWorkGroup::new(100).unwrap(),
    )
    .unwrap();
    c.register(cpu).unwrap();
    c.stop();
    assert!(c.check().is_err());
    assert!(
        c.register(CpuControl::new(Instant::now() + Duration::from_secs(1)).unwrap())
            .is_err()
    );
}
#[test]
fn literal_bilingual_facts_and_citations() {
    use super::verify::*;
    let need = ReportNeed::new(
        "project:a",
        "task:report-a",
        1,
        vec![("en".into(), "North".into()), ("tr".into(), "Güney".into())],
    )
    .unwrap();
    let sources = vec![
        SourceDocument {
            language: "en".into(),
            site: "North".into(),
            owner: Some("Mira".into()),
            checklist: Some("C7".into()),
        },
        SourceDocument {
            language: "tr".into(),
            site: "Güney".into(),
            owner: Some("Ece".into()),
            checklist: Some("C9".into()),
        },
    ];
    let selected = vec![
        SelectedFacts {
            language: "en".into(),
            site: "North".into(),
            owner: "Mira".into(),
            checklist: "C7".into(),
        },
        SelectedFacts {
            language: "tr".into(),
            site: "Güney".into(),
            owner: "Ece".into(),
            checklist: "C9".into(),
        },
    ];
    let prepared = prepare_report(&need, &sources, &selected, [1; 32], [2; 32], true).unwrap();
    assert_eq!(prepared.fact_count(), 4);
    assert_ne!(prepared.digest().unwrap(), [0; 32]);
    let mut wrong = selected.clone();
    wrong[0].owner = "Leon".into();
    assert_eq!(
        prepare_report(&need, &sources, &wrong, [1; 32], [2; 32], true).err(),
        Some(ReportError::InvalidWitness)
    );
    assert_eq!(
        prepare_report(&need, &sources, &selected, [0; 32], [2; 32], true).err(),
        Some(ReportError::InvalidWitness)
    );
}
#[test]
fn no_evidence_conflict_and_unreviewed_are_explicit() {
    use super::verify::*;
    let need = ReportNeed::new(
        "project:a",
        "task:report-a",
        1,
        vec![("en".into(), "North".into())],
    )
    .unwrap();
    let selected = vec![SelectedFacts {
        language: "en".into(),
        site: "North".into(),
        owner: "Mira".into(),
        checklist: "C7".into(),
    }];
    assert_eq!(
        prepare_report(&need, &[], &selected, [1; 32], [2; 32], true).err(),
        Some(ReportError::NoEvidence)
    );
    let mut source = SourceDocument {
        language: "en".into(),
        site: "North".into(),
        owner: Some("Mira".into()),
        checklist: Some("C7".into()),
    };
    let mut sources = vec![source.clone()];
    source.owner = Some("Leon".into());
    sources.push(source);
    assert_eq!(
        prepare_report(&need, &sources, &selected, [1; 32], [2; 32], true).err(),
        Some(ReportError::Conflict)
    );
    assert_eq!(
        prepare_report(&need, &sources, &selected, [1; 32], [2; 32], false).err(),
        Some(ReportError::NeedsSourceReview)
    );
}
#[test]
fn foreign_live_witness_is_rejected() {
    use super::storage::validate_checkpoint;
    use eylema_contracts::{
        ActionId, EvidenceRef, ExecutionId,
        codec::{self, Value},
    };
    use eylema_host::tool::{ToolReport, ToolStatus};
    let e = ExecutionId::new("execution:one").unwrap();
    let a = ActionId::new("action:count").unwrap();
    let mut report = ToolReport {
        execution_id: e.clone(),
        status: ToolStatus::Completed,
        outputs: vec![eylema_host::tool::ToolOutput {
            action_id: a.clone(),
            evidence: EvidenceRef::new("evidence:one").unwrap(),
            bytes: codec::encode(&Value::Unsigned(4)).unwrap(),
        }],
    };
    assert!(validate_checkpoint(&report, &e, &a, 4).is_ok());
    report.execution_id = ExecutionId::new("execution:other").unwrap();
    assert!(validate_checkpoint(&report, &e, &a, 4).is_err());
}
#[test]
fn source_binding_changes_need_identity() {
    let mut n = ReportNeed::new(
        "project:a",
        "task:a",
        1,
        vec![("en".into(), "North".into())],
    )
    .unwrap();
    let before = n.digest().unwrap();
    n.bind_sources([9; 32]).unwrap();
    assert_ne!(before, n.digest().unwrap());
    assert!(n.bind_sources([8; 32]).is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn reopened_record_and_citations_reauthorize() {
    use super::storage::*;
    use eylema_contracts::{platform::ProjectId, project_data::*};
    use std::{fs::DirBuilder, os::unix::fs::DirBuilderExt};
    let root = std::env::temp_dir().join(format!(
        "eyleme-g72-unit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    let row = SourceRow {
        id: "en-main".into(),
        entity: "north".into(),
        body: b"language=en;site=North;owner=Mira;checklist=C7\n".to_vec(),
    };
    let mut d =
        JobData::create(&root, ProjectId::new("project:g72-unit").unwrap(), &[row]).unwrap();
    let source = d.sources[0].pin.clone();
    let out = d
        .put(
            "final",
            b"private verified output".to_vec(),
            vec![source.reference.clone()],
            DataKind::Derived,
        )
        .unwrap();
    assert!(
        d.data
            .read(&d.session, &out.reference, DataUse::Execute)
            .is_ok()
    );
    let rev = d
        .data
        .inspect(&d.owner, &source.reference)
        .unwrap()
        .revision;
    d.data.withdraw(&d.owner, &source.reference, rev).unwrap();
    assert!(
        d.data
            .read(&d.session, &out.reference, DataUse::Execute)
            .is_err()
    );
    assert!(check_pin(&mut d.data, &d.session, &source).is_err());
    d.memory.close();
    d.data.close();
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
mod reopen_regressions {
    use super::super::*;
    struct Fixture {
        root: std::path::PathBuf,
        config: Config,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "g72-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        new_directory(&root).unwrap();
        let config = Config {
            project: ProjectId::new("project:reopen").unwrap(),
            task: "task:report".into(),
            sections: vec![
                Section {
                    language: "en".into(),
                    site: "North".into(),
                    entity: "north".into(),
                    query: "owner".into(),
                    prompt: "source".into(),
                },
                Section {
                    language: "tr".into(),
                    site: "Güney".into(),
                    entity: "south".into(),
                    query: "owner".into(),
                    prompt: "source".into(),
                },
            ],
            rows: vec![
                SourceRow {
                    id: "en-main".into(),
                    entity: "north".into(),
                    body: b"language=en;site=North;owner=Mira;checklist=C7\n".to_vec(),
                },
                SourceRow {
                    id: "tr-main".into(),
                    entity: "south".into(),
                    body: "language=tr;site=Güney;owner=Ece;checklist=C9\n"
                        .as_bytes()
                        .to_vec(),
                },
            ],
            assets: AssetPaths::from_value(&Value::List(vec![Value::Text("unused".into()); 9]))
                .unwrap(),
            value: Value::Text("synthetic-config".into()),
        };
        let mut d = JobData::create(&root, config.project.clone(), &config.rows).unwrap();
        let need = report_need(&config, &mut d.data, &d.session, false).unwrap();
        let parent = d.sources[0].pin.reference.clone();
        let review = d
            .put("review", vec![1], vec![parent], DataKind::Derived)
            .unwrap();
        // Two genuine memory journal events; structural fixture, no model/checkpoint acceptance.
        let draft = d.draft(0, None).unwrap();
        d.store_reviewed(draft, None, storage::pin(&review))
            .unwrap();
        assert_eq!(d.memory.generation(), 2);
        let pins = storage::verify_pins(
            &mut d.data,
            &d.session,
            &d.sources.iter().map(|s| s.pin.clone()).collect::<Vec<_>>(),
        )
        .unwrap();
        let facts = vec![
            SelectedFacts {
                language: "en".into(),
                site: "North".into(),
                owner: "Mira".into(),
                checklist: "C7".into(),
            },
            SelectedFacts {
                language: "tr".into(),
                site: "Güney".into(),
                owner: "Ece".into(),
                checklist: "C9".into(),
            },
        ];
        let report = verify::prepare_report(
            &need,
            &d.sources
                .iter()
                .map(|s| s.document.clone())
                .collect::<Vec<_>>(),
            &facts,
            [1; 32],
            [2; 32],
            true,
        )
        .unwrap()
        .bind_citations(&pins)
        .unwrap()
        .bind_generation(1)
        .unwrap();
        d.put(
            "need",
            encoded(&Value::Bytes(need.digest().unwrap().to_vec())).unwrap(),
            pins.references(),
            DataKind::Derived,
        )
        .unwrap();
        let prepared = d
            .put(
                "prepared",
                report.bytes().to_vec(),
                pins.references(),
                DataKind::Derived,
            )
            .unwrap();
        let final_record = d
            .put(
                "final",
                report.bytes().to_vec(),
                vec![prepared.reference],
                DataKind::Derived,
            )
            .unwrap();
        d.put(
            "outcome",
            encoded(&Value::List(vec![
                Value::Text("completed".into()),
                Value::Bytes(report.digest().unwrap().to_vec()),
            ]))
            .unwrap(),
            vec![final_record.reference],
            DataKind::Derived,
        )
        .unwrap();
        d.memory.close();
        d.data.close();
        Fixture { root, config }
    }
    #[test]
    fn normal_inspection_is_repeatable_and_preserves_data_and_policy() {
        fn semantic_records(root: &std::path::Path) -> Vec<Value> {
            let mut journal = Journal::open(
                NativeStorage::open_existing(&root.join("data.eylj"), storage::sync).unwrap(),
                NativeHash,
                eylema_host::project_data::project_journal_limits(),
            )
            .unwrap();
            journal.read_records().unwrap().into_iter().map(|r|r.payload).filter(|v|!matches!(v,Value::List(rows) if matches!(rows.first(),Some(Value::Text(tag)) if tag=="clock"))).collect()
        }
        let f = fixture();
        let before = (
            semantic_records(&f.root),
            std::fs::read(f.root.join("memory.eylj")).unwrap(),
        );
        inspect_report(&f.config, &f.root).unwrap();
        inspect_report(&f.config, &f.root).unwrap();
        assert_eq!(
            before,
            (
                semantic_records(&f.root),
                std::fs::read(f.root.join("memory.eylj")).unwrap()
            )
        );
    }
    #[test]
    fn fresh_inspection_rejects_changed_need_and_source_revision() {
        let mut f = fixture();
        f.config.sections[0].site = "Other".into();
        assert!(inspect_report(&f.config, &f.root).is_err());
        f.config.sections[0].site = "North".into();
        let (mut data, owner, _) =
            storage::reopen_data(&f.root, f.config.project.clone(), &f.config.rows).unwrap();
        let r = DataRef {
            project: f.config.project.clone(),
            id: storage::id("en-main").unwrap(),
        };
        let m = data.inspect(&owner, &r).unwrap();
        data.set_policy(&owner, &r, m.revision, storage::permissions(), false)
            .unwrap();
        data.close();
        assert_eq!(
            inspect_report(&f.config, &f.root).err(),
            Some(ReportError::RevisionConflict)
        );
    }
    #[test]
    fn fresh_inspection_rejects_genuine_memory_generation_change() {
        let f = fixture();
        let (mut data, owner, session) =
            storage::reopen_data(&f.root, f.config.project.clone(), &f.config.rows).unwrap();
        let mut c = storage::memory_control().unwrap();
        let (mut memory, mo) = Memory::open(
            NativeStorage::open_existing(&f.root.join("memory.eylj"), storage::sync).unwrap(),
            NativeHash,
            &mut data,
            &owner,
            storage::profile(f.config.project.clone()).unwrap(),
            &mut c,
        )
        .unwrap();
        let ms = memory.enroll(&mo, &mut data, &session, &mut c).unwrap();
        let now = data.trusted_time().unwrap();
        let r = DataRef {
            project: f.config.project.clone(),
            id: storage::id("en-main").unwrap(),
        };
        let pin = storage::pin(&data.inspect(&owner, &r).unwrap());
        let draft = RecordDraft {
            project: f.config.project.clone(),
            id: storage::id("south").unwrap(),
            revision: rev(1),
            profile: semantic("report-memory:v1").unwrap(),
            schema: semantic("report-records:v1").unwrap(),
            kind: RecordKind::Entity,
            origin: Origin::DocumentClaim,
            observed_at: now,
            valid_from: now,
            expires_at: now + 60000,
            confidence_bps: None,
            subject: Some(storage::id("south").unwrap()),
            predicate: None,
            object: None,
            episode_status: None,
            episode_text: None,
            sources: vec![pin],
        };
        let projection = DataRef {
            project: f.config.project.clone(),
            id: storage::id("m:south:1").unwrap(),
        };
        memory
            .propose(&mut data, &ms, 2, None, draft, None, projection, &mut c)
            .unwrap();
        memory.close();
        data.close();
        assert_eq!(
            inspect_report(&f.config, &f.root).err(),
            Some(ReportError::RevisionConflict)
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn checkpoint_commit_gap_is_not_retryable() {
    use super::*;
    for failure in [
        ReportError::Stopped,
        ReportError::Deadline,
        ReportError::Forbidden,
        ReportError::OutcomeUnknown,
    ] {
        let root = std::env::temp_dir().join(format!(
            "g72-release-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        new_directory(&root).unwrap();
        let rows = vec![SourceRow {
            id: "source".into(),
            entity: "north".into(),
            body: b"language=en;site=North;owner=Mira;checklist=C7\n".to_vec(),
        }];
        let mut d = JobData::create(
            &root,
            ProjectId::new("project:release-test").unwrap(),
            &rows,
        )
        .unwrap();
        let source = d.sources[0].pin.reference.clone();
        let final_record = d
            .put(
                "final",
                b"synced private output".to_vec(),
                vec![source],
                DataKind::Derived,
            )
            .unwrap();
        let r = final_record.reference;
        assert_eq!(
            finalize_report(&mut d, r.clone(), [8; 32], |data| {
                // Fault injected only after the real final collect and policy sync.
                assert_eq!(
                    data.data.read(&data.session, &r, DataUse::Execute).unwrap(),
                    b"synced private output"
                );
                Err(failure)
            })
            .err(),
            Some(if failure == ReportError::OutcomeUnknown {
                failure
            } else {
                ReportError::Partial
            })
        );
        d.memory.close();
        d.data.close();
        let (mut reopened, _, session) = storage::reopen_data(
            &root,
            ProjectId::new("project:release-test").unwrap(),
            &rows,
        )
        .unwrap();
        let outcome = DataRef {
            project: ProjectId::new("project:release-test").unwrap(),
            id: storage::id("outcome").unwrap(),
        };
        assert_eq!(
            codec::decode(&reopened.read(&session, &outcome, DataUse::Execute).unwrap()).unwrap(),
            Value::List(vec![
                Value::Text("partial".into()),
                Value::Bytes(vec![8; 32])
            ])
        );
        reopened.close();
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[path = "../../../../crates/eylema-runtime/tests/support/mod.rs"]
mod composition_support;

#[cfg(target_os = "macos")]
#[test]
fn genuine_foreign_execution_witness_is_rejected() {
    // Real owned ToolHost execution with a synthetic clock/controller, not OS proof.
    use super::*;
    use eylema_host::project_data::{DataClock, DataResult, ProjectStore};
    use std::{cell::Cell, rc::Rc};
    struct Clock;
    impl DataClock for Clock {
        fn now(&mut self) -> DataResult<u64> {
            Ok(1000)
        }
    }
    let root = std::env::temp_dir().join(format!(
        "g72-witness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    new_directory(&root).unwrap();
    let services = composition_support::Services {
        clock: Rc::new(Cell::new(1000)),
        next: 0,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let (host, owner) = DurableHost::create_new(
        Journal::create_new(
            NativeStorage::create_new(&root.join("tools.eylj"), storage::sync).unwrap(),
            NativeHash,
            Default::default(),
        )
        .unwrap(),
        services,
        Default::default(),
    )
    .unwrap();
    let mut tools = ToolHost::new(
        host,
        ToolRegistry::new(vec![ToolProfile::new(
            semantic("tool:add").unwrap(),
            rev(1),
            OwnedTool::AddTaskValue,
        )])
        .unwrap(),
    )
    .unwrap();
    let actor = tools.enroll(&owner, composition_support::grant()).unwrap();
    let mut grant = composition_support::grant();
    grant.permissions.push(Permission::RequestCancellation);
    tools.enroll_verified_human(&owner, grant).unwrap();
    let (mut data, owner) = ProjectStore::create_new(
        Journal::create_new(
            NativeStorage::create_new(&root.join("data.eylj"), storage::sync).unwrap(),
            NativeHash,
            eylema_host::project_data::project_journal_limits(),
        )
        .unwrap(),
        ProjectId::new("project:witness").unwrap(),
        Clock,
    )
    .unwrap();
    let ds = data
        .enroll(
            &owner,
            DataGrant {
                permissions: storage::permissions(),
                scope: vec![storage::id("unused").unwrap()],
                expires_at: 10000,
            },
        )
        .unwrap();
    let mut reports = vec![];
    for name in ["task:first", "task:other"] {
        let task = composition_support::task(name, 20, 50);
        tools.create_task(&actor, task.clone()).unwrap();
        let proposal = composition_support::proposal(&task, &[4]);
        let binding = ToolBinding {
            action_id: proposal.fields().actions[0].fields().action_id.clone(),
            profile: semantic("tool:add").unwrap(),
            revision: rev(1),
            input_schema: semantic("eyl:u64:v1").unwrap(),
            output_schema: semantic("eyl:task-value:v1").unwrap(),
            source: None,
        };
        let plan = ToolPlan::new(task, proposal, vec![binding]).unwrap();
        let id = tools
            .admit(&mut data, &ds, &actor, plan.clone(), &[])
            .unwrap();
        tools.step(&mut data).unwrap();
        tools.step(&mut data).unwrap();
        let report = tools.report(&mut data, &actor, &id).unwrap();
        storage::validate_checkpoint(&report, &id, &plan.bindings()[0].action_id, 4).unwrap();
        reports.push((report, id, plan.bindings()[0].action_id.clone()));
    }
    assert_eq!(
        storage::validate_checkpoint(&reports[1].0, &reports[0].1, &reports[0].2, 4).err(),
        Some(ReportError::InvalidWitness)
    );
    data.close();
    drop(tools);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn single_domain_repair_and_second_failure_stop() {
    let mut ledger = JobLedger::new(2_500_000_000_000).unwrap();
    ledger.reserve(650_000_000_000).unwrap();
    ledger.repair().unwrap();
    assert_eq!(ledger.repair().err(), Some(ReportError::Limit));
    assert_eq!(
        ledger.reserve(2_500_000_000_000).err(),
        Some(ReportError::Limit)
    );
}
