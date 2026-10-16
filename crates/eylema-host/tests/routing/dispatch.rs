use super::fixtures::*;
use eylema_host::routing::*;
#[test]
fn actual_search_consumes_discovery_selection_and_shared_budget() {
    let (mut data, owner, session, _, pins, index, q) = retrieval_fixture();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    assert_eq!(
        router
            .catalog(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap()
            .entry_count(),
        1
    );
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let a = vec![assessment(pins[0].clone())];
    let r = request();
    let mut c = control();
    let Selection::Selected(ticket) = router
        .select(
            &mut data,
            &session,
            &desc,
            &r,
            &a,
            CallPayload::Retrieval(&q),
            &mut c,
        )
        .unwrap()
    else {
        panic!("selected")
    };
    let before = c.work_used();
    assert!(before > 0);
    // Selection and call are separate synchronous operations; each explicit caller
    // control is bounded. The actual call must keep its wrapper+child counters.
    let mut direct = eylema_retrieval::Control::new(
        eylema_retrieval::RetrievalBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
        },
        eylema_retrieval::CancelToken::new(),
    )
    .unwrap();
    index.search(&mut data, &session, &q, &mut direct).unwrap();
    let direct_work = direct.work_used();
    let mut c = control();
    let result = router
        .call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut c)
        .unwrap();
    assert!(
        c.work_used() > direct_work + 2_000_000,
        "router work must survive the real child search"
    );
    let DispatchReceipt::Retrieval { response, no_match } = result else {
        panic!("retrieval")
    };
    assert!(!no_match);
    assert_eq!(response.hit_count(), 2);
    assert_eq!(response.source_count(), 2);
    assert!(c.work_used() > before);
}
#[test]
fn changed_query_or_revoked_source_cannot_reuse_selection() {
    let (mut data, owner, session, _, pins, index, mut q) = retrieval_fixture();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let a = vec![assessment(pins[0].clone())];
    let r = request();
    let Selection::Selected(ticket) = router
        .select(
            &mut data,
            &session,
            &desc,
            &r,
            &a,
            CallPayload::Retrieval(&q),
            &mut control(),
        )
        .unwrap()
    else {
        panic!("select")
    };
    q.top_k = 2;
    assert!(matches!(
        router.call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut control()),
        Err(RouteError::Stale)
    ));
    q.top_k = 1;
    let Selection::Selected(ticket) = router
        .select(
            &mut data,
            &session,
            &desc,
            &r,
            &a,
            CallPayload::Retrieval(&q),
            &mut control(),
        )
        .unwrap()
    else {
        panic!("select")
    };
    data.revoke(&owner, &session).unwrap();
    assert!(matches!(
        router.call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut control()),
        Err(RouteError::Forbidden)
    ));
}
#[test]
fn actual_routed_skill_admission_preserves_inner_effect_and_stop() {
    use eylema_contracts::codec::{self, Value};
    use eylema_host::tool::ToolStatus;
    use eylema_runtime::routing::Intent;
    let mut f = skill_fixture();
    let (router, _) =
        Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
    let desc = router
        .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
        .unwrap();
    let mut r = request();
    r.intent = Intent::TypedSkill;
    let a = vec![assessment(f.pins[0].clone())];
    let Selection::Selected(ticket) = router
        .select(
            &mut f.data,
            &f.session,
            &desc,
            &r,
            &a,
            CallPayload::Skill {
                plan: &f.plan,
                definition: &f.definition,
                approvals: &[],
            },
            &mut control(),
        )
        .unwrap()
    else {
        panic!("skill selected")
    };
    let DispatchReceipt::SkillAdmitted { execution } = router
        .call_skill(
            &mut f.data,
            ticket,
            &r,
            &a,
            f.plan,
            f.definition,
            &[],
            &mut f.tools,
            &f.actor,
            &mut control(),
        )
        .unwrap()
    else {
        panic!("admitted")
    };
    f.tools.step(&mut f.data).unwrap();
    f.tools.step(&mut f.data).unwrap();
    let report = f.tools.report(&mut f.data, &f.actor, &execution).unwrap();
    assert_eq!(report.status, ToolStatus::Completed);
    assert_eq!(
        codec::decode(&report.outputs[1].bytes).unwrap(),
        Value::Unsigned(2)
    );
    let mut f = skill_fixture();
    let (router, _) =
        Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
    let desc = router
        .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
        .unwrap();
    let a = vec![assessment(f.pins[0].clone())];
    let Selection::Selected(ticket) = router
        .select(
            &mut f.data,
            &f.session,
            &desc,
            &r,
            &a,
            CallPayload::Skill {
                plan: &f.plan,
                definition: &f.definition,
                approvals: &[],
            },
            &mut control(),
        )
        .unwrap()
    else {
        panic!("selected")
    };
    let DispatchReceipt::SkillAdmitted { execution } = router
        .call_skill(
            &mut f.data,
            ticket,
            &r,
            &a,
            f.plan,
            f.definition,
            &[],
            &mut f.tools,
            &f.actor,
            &mut control(),
        )
        .unwrap()
    else {
        panic!("admitted")
    };
    f.tools.step(&mut f.data).unwrap();
    f.tools.request_cancellation(&f.human, &execution).unwrap();
    f.tools.step(&mut f.data).unwrap();
    let report = f.tools.report(&mut f.data, &f.actor, &execution).unwrap();
    assert_ne!(report.status, ToolStatus::Completed);
    assert_eq!(report.outputs.len(), 1);
    assert_eq!(
        codec::decode(&report.outputs[0].bytes).unwrap(),
        Value::Unsigned(2)
    );
}
#[test]
fn combined_operation_envelope_is_not_silently_reset() {
    let (mut data, owner, session, _, pins, index, q) = retrieval_fixture();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let a = vec![assessment(pins[0].clone())];
    let r = request();
    let mut c = control();
    let Selection::Selected(ticket) = router
        .select(
            &mut data,
            &session,
            &desc,
            &r,
            &a,
            CallPayload::Retrieval(&q),
            &mut c,
        )
        .unwrap()
    else {
        panic!("selected")
    };
    assert!(matches!(
        router.call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut c),
        Err(RouteError::Limit)
    ));
    assert!(c.work_used() <= 10_000_000);
}
#[test]
fn lexical_no_overlap_is_no_match_not_unavailable() {
    let (mut data, owner, session, _, pins, index, mut q) = retrieval_fixture();
    q.input = eylema_retrieval::QueryInput::Lexical("banana".into());
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let a = vec![assessment(pins[0].clone())];
    let r = request();
    let Selection::Selected(ticket) = router
        .select(
            &mut data,
            &session,
            &desc,
            &r,
            &a,
            CallPayload::Retrieval(&q),
            &mut control(),
        )
        .unwrap()
    else {
        panic!("selected")
    };
    assert!(matches!(
        router
            .call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut control())
            .unwrap(),
        DispatchReceipt::NoMatch
    ));
}
#[test]
fn changed_skill_schema_and_forged_approval_do_not_admit() {
    use eylema_contracts::{EventId, tool::ToolPlan};
    use eylema_runtime::routing::Intent;
    for forged in [false, true] {
        let mut f = skill_fixture();
        let (router, _) =
            Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
        let desc = router
            .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
            .unwrap();
        let mut r = request();
        r.intent = Intent::TypedSkill;
        let a = vec![assessment(f.pins[0].clone())];
        let Selection::Selected(ticket) = router
            .select(
                &mut f.data,
                &f.session,
                &desc,
                &r,
                &a,
                CallPayload::Skill {
                    plan: &f.plan,
                    definition: &f.definition,
                    approvals: &[],
                },
                &mut control(),
            )
            .unwrap()
        else {
            panic!("select")
        };
        if forged {
            let fake = EventId::new("event:forged").unwrap();
            assert!(matches!(
                router.call_skill(
                    &mut f.data,
                    ticket,
                    &r,
                    &a,
                    f.plan,
                    f.definition,
                    &[fake],
                    &mut f.tools,
                    &f.actor,
                    &mut control()
                ),
                Err(RouteError::Stale)
            ));
        } else {
            let mut bindings = f.plan.bindings().to_vec();
            bindings[0].input_schema = id("schema:wrong");
            let bad =
                ToolPlan::new(f.plan.task().clone(), f.plan.proposal().clone(), bindings).unwrap();
            assert!(matches!(
                router.call_skill(
                    &mut f.data,
                    ticket,
                    &r,
                    &a,
                    bad.clone(),
                    f.definition.clone(),
                    &[],
                    &mut f.tools,
                    &f.actor,
                    &mut control()
                ),
                Err(RouteError::Stale)
            ));
            assert!(
                f.tools
                    .admit_skill(&mut f.data, &f.session, &f.actor, (bad, f.definition), &[])
                    .is_err()
            );
        }
        assert!(f.tools.step(&mut f.data).unwrap().is_none());
    }
}
#[test]
fn existing_skill_control_ancestry_still_gates_each_effect() {
    use eylema_contracts::project_data::DataPermissions;
    use eylema_runtime::routing::Intent;
    let mut f = skill_fixture();
    let (router, _) =
        Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
    let desc = router
        .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
        .unwrap();
    let mut r = request();
    r.intent = Intent::TypedSkill;
    let a = vec![assessment(f.pins[0].clone())];
    let Selection::Selected(ticket) = router
        .select(
            &mut f.data,
            &f.session,
            &desc,
            &r,
            &a,
            CallPayload::Skill {
                plan: &f.plan,
                definition: &f.definition,
                approvals: &[],
            },
            &mut control(),
        )
        .unwrap()
    else {
        panic!("selected")
    };
    let DispatchReceipt::SkillAdmitted { execution } = router
        .call_skill(
            &mut f.data,
            ticket,
            &r,
            &a,
            f.plan,
            f.definition,
            &[],
            &mut f.tools,
            &f.actor,
            &mut control(),
        )
        .unwrap()
    else {
        panic!("admitted")
    };
    let m = f.data.inspect(&f.owner, &f.pins[0].reference).unwrap();
    f.data
        .set_policy(
            &f.owner,
            &m.reference,
            m.revision,
            DataPermissions::NONE,
            false,
        )
        .unwrap();
    assert!(f.tools.step(&mut f.data).is_err());
    f.tools.request_cancellation(&f.human, &execution).unwrap();
    f.tools.step(&mut f.data).unwrap();
}
#[test]
fn positive_lexical_and_hybrid_rankings_keep_actual_hits() {
    for hybrid in [false, true] {
        let (mut data, owner, session, _, pins, index, mut q) = retrieval_fixture();
        q.input = if hybrid {
            let mut v = vec![0.; 1024];
            v[0] = 1.;
            eylema_retrieval::QueryInput::Hybrid {
                text: "banana".into(),
                vector: v,
                vector_weight: 500000,
            }
        } else {
            eylema_retrieval::QueryInput::Lexical("apple".into())
        };
        let mut cfg = config(pins[0].clone());
        cfg.entries[0].sources = pins.clone();
        let (router, _) =
            Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
        let desc = router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap();
        let a = vec![assessment(pins[0].clone())];
        let r = request();
        let Selection::Selected(ticket) = router
            .select(
                &mut data,
                &session,
                &desc,
                &r,
                &a,
                CallPayload::Retrieval(&q),
                &mut control(),
            )
            .unwrap()
        else {
            panic!("selected")
        };
        let DispatchReceipt::Retrieval { response, no_match } = router
            .call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut control())
            .unwrap()
        else {
            panic!("actual hits")
        };
        assert!(!no_match);
        assert_eq!(response.hit_count(), if hybrid { 2 } else { 3 });
    }
}
#[test]
fn late_registry_expiry_reports_unknown_admission_without_releasing_payload() {
    use eylema_contracts::{ActionProposal, ActionSpec, Budget, Timestamp, tool::ToolPlan};
    use eylema_host::tool::ToolStatus;
    use eylema_runtime::routing::Intent;
    let mut measured_ticks = 0;
    let mut expected_execution = None;
    for inject in [false, true] {
        let mut f = skill_fixture();
        f.config.policy.valid_until = 1500;
        let mut proposal = f.plan.proposal().fields().clone();
        for action in &mut proposal.actions {
            let mut d = action.fields().clone();
            let b = d.budget;
            d.budget = Budget::new(
                b.max_steps(),
                b.max_events(),
                b.max_payload_bytes(),
                b.max_concurrency(),
                Timestamp::new(1200).unwrap(),
            )
            .unwrap();
            *action = ActionSpec::new(d).unwrap();
        }
        f.plan = ToolPlan::new(
            f.plan.task().clone(),
            ActionProposal::new(proposal, f.plan.task()).unwrap(),
            f.plan.bindings().to_vec(),
        )
        .unwrap();
        let (router, _) =
            Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
        let desc = router
            .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
            .unwrap();
        let mut r = request();
        r.intent = Intent::TypedSkill;
        let a = vec![assessment(f.pins[0].clone())];
        let Selection::Selected(ticket) = router
            .select(
                &mut f.data,
                &f.session,
                &desc,
                &r,
                &a,
                CallPayload::Skill {
                    plan: &f.plan,
                    definition: &f.definition,
                    approvals: &[],
                },
                &mut control(),
            )
            .unwrap()
        else {
            panic!("selected")
        };
        let start = f.clock.2.get();
        if inject {
            f.clock.3.set(Some((start + measured_ticks, 1501)));
        }
        let result = router.call_skill(
            &mut f.data,
            ticket,
            &r,
            &a,
            f.plan,
            f.definition,
            &[],
            &mut f.tools,
            &f.actor,
            &mut control(),
        );
        if inject {
            assert!(matches!(result, Err(RouteError::AdmissionOutcomeUnknown)));
            let report = f
                .tools
                .report(&mut f.data, &f.actor, expected_execution.as_ref().unwrap())
                .unwrap();
            assert_eq!(report.status, ToolStatus::Pending);
            assert!(report.outputs.is_empty());
            assert_eq!(f.clock.0.get(), 1501);
        } else {
            let DispatchReceipt::SkillAdmitted { execution } = result.unwrap() else {
                panic!("admitted")
            };
            measured_ticks = f.clock.2.get() - start;
            assert!(measured_ticks > 1);
            expected_execution = Some(execution);
        }
    }
}
#[test]
fn uncertainty_stale_capacity_and_foreign_description_cannot_select() {
    for mode in 0..4 {
        let (mut data, owner, session, _, pins, _index, q) = retrieval_fixture();
        let mut cfg = config(pins[0].clone());
        cfg.entries[0].sources = pins.clone();
        if mode == 0 {
            cfg.entries[0].grounding = eylema_runtime::routing::SourceStatus::Unknown;
        }
        if mode == 1 {
            cfg.resources[0].valid_until = 999;
        }
        if mode == 2 {
            cfg.entries[0].intent = eylema_runtime::routing::Intent::Code;
            cfg.entries[0].backend = eylema_runtime::routing::RouteBackend::Unsupported;
        }
        let (router, _) =
            Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
        let desc = router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap();
        let mut r = request();
        if mode == 2 {
            r.intent = eylema_runtime::routing::Intent::Code;
        }
        let mut a = vec![assessment(pins[0].clone())];
        if mode == 3 {
            a[0].utility = 499999;
        }
        let result = router
            .select(
                &mut data,
                &session,
                &desc,
                &r,
                &a,
                CallPayload::Retrieval(&q),
                &mut control(),
            )
            .unwrap();
        assert!(matches!(
            result,
            Selection::Abstain | Selection::Unavailable
        ));
    }
    let (mut data, owner, session, _, pins, _index, q) = retrieval_fixture();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins.clone();
    let (other, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    assert!(matches!(
        other.select(
            &mut data,
            &session,
            &desc,
            &request(),
            &[assessment(pins[0].clone())],
            CallPayload::Retrieval(&q),
            &mut control()
        ),
        Err(RouteError::Forbidden)
    ));
}
#[test]
fn nonancestor_private_control_and_unbound_deadlines_cannot_admit() {
    use eylema_contracts::project_data::*;
    use eylema_runtime::routing::Intent;
    use eylema_runtime::routing::SourcePin;
    for extra in [false, true] {
        let mut f = skill_fixture();
        if extra {
            let reference = reference("source:extra");
            f.session = f
                .data
                .enroll(
                    &f.owner,
                    DataGrant {
                        permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                            .unwrap(),
                        scope: f
                            .pins
                            .iter()
                            .map(|p| p.reference.id.clone())
                            .chain(std::iter::once(reference.id.clone()))
                            .collect(),
                        expires_at: 10000,
                    },
                )
                .unwrap();
            let m = f
                .data
                .collect(
                    &f.session,
                    DataDraft {
                        reference: reference.clone(),
                        kind: DataKind::Artifact,
                        parents: vec![],
                        payload: b"separate control".to_vec(),
                        expires_at: 5000,
                    },
                )
                .unwrap();
            f.data
                .set_policy(
                    &f.owner,
                    &reference,
                    m.revision,
                    DataPermissions::new(vec![DataUse::Execute]).unwrap(),
                    false,
                )
                .unwrap();
            let p = SourcePin {
                reference,
                digest: m.digest,
            };
            f.config.entries[0].sources.push(p);
        } else {
            f.config.policy.valid_until = 1500;
        }
        let (router, _) =
            Router::register(&mut f.data, &f.owner, &f.session, f.config, &mut control()).unwrap();
        let desc = router
            .describe(&mut f.data, &f.session, &[id("route:a")], &mut control())
            .unwrap();
        let mut r = request();
        r.intent = Intent::TypedSkill;
        let a = vec![assessment(f.pins[0].clone())];
        let Selection::Selected(ticket) = router
            .select(
                &mut f.data,
                &f.session,
                &desc,
                &r,
                &a,
                CallPayload::Skill {
                    plan: &f.plan,
                    definition: &f.definition,
                    approvals: &[],
                },
                &mut control(),
            )
            .unwrap()
        else {
            panic!("selected")
        };
        assert!(
            router
                .call_skill(
                    &mut f.data,
                    ticket,
                    &r,
                    &a,
                    f.plan,
                    f.definition,
                    &[],
                    &mut f.tools,
                    &f.actor,
                    &mut control()
                )
                .is_err()
        );
        assert!(f.tools.step(&mut f.data).unwrap().is_none());
    }
}
#[test]
fn bit_pattern_and_representation_changes_invalidate_query_ticket() {
    for representation in [false, true] {
        let (mut data, owner, session, _, pins, index, mut q) = retrieval_fixture();
        let mut cfg = config(pins[0].clone());
        cfg.entries[0].sources = pins.clone();
        let (router, _) =
            Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
        let desc = router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap();
        let a = vec![assessment(pins[0].clone())];
        let r = request();
        let Selection::Selected(ticket) = router
            .select(
                &mut data,
                &session,
                &desc,
                &r,
                &a,
                CallPayload::Retrieval(&q),
                &mut control(),
            )
            .unwrap()
        else {
            panic!("selected")
        };
        if representation {
            q.binding.artifact_sha256[0] = 9;
        } else if let eylema_retrieval::QueryInput::Vector(v) = &mut q.input {
            v[0] = 0.5;
        }
        assert!(matches!(
            router.call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut control()),
            Err(RouteError::Stale)
        ));
    }
}
#[test]
fn cancel_and_deadline_after_real_ranking_release_no_result() {
    use eylema_retrieval::{CancelToken, RetrievalBudget};
    use std::time::{Duration, Instant};
    let mut ticks = 0;
    for mode in 0..3 {
        let (mut data, owner, session, clock, pins, index, q) = retrieval_fixture();
        let mut cfg = config(pins[0].clone());
        cfg.entries[0].sources = pins.clone();
        let (router, _) =
            Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
        let desc = router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap();
        let a = vec![assessment(pins[0].clone())];
        let r = request();
        let Selection::Selected(ticket) = router
            .select(
                &mut data,
                &session,
                &desc,
                &r,
                &a,
                CallPayload::Retrieval(&q),
                &mut control(),
            )
            .unwrap()
        else {
            panic!("selected")
        };
        let start = clock.2.get();
        let token = CancelToken::new();
        let mut c = RouteControl::new(
            RetrievalBudget {
                max_work: 10_000_000,
                max_live_bytes: 64 * 1024 * 1024,
                deadline: Instant::now() + Duration::from_secs(2),
            },
            token.clone(),
        )
        .unwrap();
        if mode == 1 {
            clock.4.borrow_mut().cancel = Some((start + ticks - 1, token));
        }
        if mode == 2 {
            clock.4.borrow_mut().delay = Some((start + ticks - 1, Duration::from_millis(2100)));
        }
        let result = router.call_retrieval(&mut data, ticket, &r, &a, &q, &index, &mut c);
        match mode {
            0 => {
                assert!(matches!(result, Ok(DispatchReceipt::Retrieval { .. })));
                ticks = clock.2.get() - start;
                assert!(ticks > 2);
            }
            1 => {
                assert!(matches!(result, Err(RouteError::Cancelled)));
                assert!(c.work_used() > 6_000_000);
                assert!(clock.4.borrow().cancel.is_none());
            }
            _ => {
                assert!(matches!(result, Err(RouteError::Deadline)));
                assert!(c.work_used() > 6_000_000);
                assert!(clock.4.borrow().delay.is_none());
            }
        }
    }
}
