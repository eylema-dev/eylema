#[path = "routing/fixtures.rs"]
mod fixtures;
use eylema_host::routing::*;
use fixtures::*;
#[test]
fn authorized_discovery_preserves_pins_and_hides_denied_existence() {
    let (mut data, owner, session, _, source) = fixture(b"hello".to_vec());
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
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
    assert_eq!(desc.source_count(), 1);
    assert!(!desc.bytes().is_empty());
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:missing")], &mut control()),
        Err(RouteError::Forbidden)
    ));
    data.revoke(&owner, &session).unwrap();
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut control()),
        Err(RouteError::Forbidden)
    ));
}
#[test]
fn live_store_owner_and_current_ttl_are_required() {
    let (mut data, owner, session, clock, source) = fixture(b"hello".to_vec());
    let (mut other, wrong, foreign, _, _) = fixture(b"hello".to_vec());
    assert!(
        Router::register(
            &mut data,
            &wrong,
            &session,
            config(source.clone()),
            &mut control()
        )
        .is_err()
    );
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    assert!(matches!(
        router.describe(&mut other, &foreign, &[id("route:a")], &mut control()),
        Err(RouteError::Forbidden)
    ));
    clock.0.set(5001);
    assert!(
        router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .is_err()
    );
}
#[test]
fn aggregate_source_limit_precedes_body_handoff() {
    let (mut data, owner, session, _, source) = fixture(vec![b'x'; 32769]);
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut control()),
        Err(RouteError::Limit)
    ));
}
#[test]
fn description_keeps_kind_purpose_schema_and_expiry() {
    use eylema_contracts::codec::{self, Value};
    let (mut data, owner, session, _, source) = fixture(b"hello".to_vec());
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    let desc = router
        .describe(&mut data, &session, &[id("route:a")], &mut control())
        .unwrap();
    let Value::List(root) = codec::decode(desc.bytes()).unwrap() else {
        panic!("description list")
    };
    assert_eq!(root.len(), 7);
    let Value::List(entries) = &root[5] else {
        panic!("entries")
    };
    let Value::List(entry) = &entries[0] else {
        panic!("entry")
    };
    assert_eq!(entry[4], Value::Text("schema:query".into()));
    assert_eq!(entry[5], Value::Text("schema:hits".into()));
    let Value::List(sources) = &root[6] else {
        panic!("sources")
    };
    let Value::List(source) = &sources[0] else {
        panic!("source")
    };
    assert_eq!(source[4], Value::Text("artifact".into()));
    assert_eq!(source[5], Value::Unsigned(5000));
    assert_eq!(source[6], Value::Text("execute".into()));
    assert_eq!(source[7], Value::Bytes(b"hello".to_vec()));
}
#[test]
fn final_shared_time_rejects_expiry_after_staging() {
    let (mut data, owner, session, clock, source) = fixture(b"hello".to_vec());
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    clock.1.set(1400);
    assert!(
        router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .is_err()
    );
    assert!(clock.0.get() > 5000);
}
#[test]
fn exact_serialized_limit_and_pre_read_source_limit() {
    let (mut data, owner, session, _, source) = fixture(vec![b'x'; 16087]);
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    assert_eq!(
        router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap()
            .bytes()
            .len(),
        16384
    );
    let (mut data, owner, session, _, source) = fixture(vec![b'x'; 16088]);
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut control()),
        Err(RouteError::Limit)
    ));
    let (mut data, owner, session, clock, source) = fixture(vec![b'x'; 32769]);
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    clock.1.set(1);
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut control()),
        Err(RouteError::Limit)
    ));
    assert_eq!(
        clock.0.get(),
        1002,
        "only metadata and size authorization; no body read observation"
    );
}
#[test]
fn malformed_payload_and_unavailable_skill_never_describe() {
    use eylema_contracts::{codec, platform::ModelProfileId, skill::*, tool::*};
    for body in [vec![255], {
        let mut b = codec::HEADER.to_vec();
        b.push(7);
        b.extend(65536u32.to_be_bytes());
        b
    }] {
        let (mut data, owner, session, _, source) = fixture(body);
        let (router, _) =
            Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
        assert!(
            router
                .describe(&mut data, &session, &[id("route:a")], &mut control())
                .is_err()
        );
    }
    let model = ModelDependency::new(
        ModelProfileId::new("model:unknown").unwrap(),
        rev(1),
        [1; 32],
        id("adapter:model"),
    )
    .unwrap();
    let package = SkillDefinition::new(
        SkillId::new("skill:add").unwrap(),
        rev(1),
        ToolProfile::new(id("tool:add"), rev(1), OwnedTool::AddTaskValue),
        id(OWNED_SKILL_ADAPTER),
        Some(model),
        id("acceptance:declared"),
    )
    .unwrap();
    let (mut data, owner, session, _, source) =
        fixture(codec::encode(&package.to_value()).unwrap());
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut control()),
        Err(RouteError::Unavailable)
    ));
}
#[test]
fn collect_only_and_cancelled_operations_cannot_discover() {
    use eylema_contracts::project_data::*;
    use eylema_retrieval::{CancelToken, RetrievalBudget};
    use std::time::{Duration, Instant};
    let (mut data, owner, session, _, source) = fixture(b"hello".to_vec());
    let (router, _) =
        Router::register(&mut data, &owner, &session, config(source), &mut control()).unwrap();
    let collect = data
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect]).unwrap(),
                scope: vec![reference("source:a").id],
                expires_at: 5000,
            },
        )
        .unwrap();
    assert!(matches!(
        router.catalog(&mut data, &collect, &[id("route:a")], &mut control()),
        Err(RouteError::Forbidden)
    ));
    let token = CancelToken::new();
    let mut c = RouteControl::new(
        RetrievalBudget {
            max_work: 10_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        token.clone(),
    )
    .unwrap();
    token.cancel();
    assert!(matches!(
        router.describe(&mut data, &session, &[id("route:a")], &mut c),
        Err(RouteError::Cancelled)
    ));
}

#[path = "routing/dispatch.rs"]
mod dispatch;
#[test]
fn eight_unique_description_sources_fit_and_nine_reject_without_partial_result() {
    use eylema_contracts::project_data::*;
    use eylema_runtime::routing::SourcePin;
    let (mut data, owner, _, _, source) = fixture(b"hello".to_vec());
    let ids: Vec<_> = (0..8).map(|n| reference(&format!("extra:{n}"))).collect();
    let session = data
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: ids
                    .iter()
                    .map(|r| r.id.clone())
                    .chain(std::iter::once(source.reference.id.clone()))
                    .collect(),
                expires_at: 10000,
            },
        )
        .unwrap();
    let mut pins = vec![source];
    for r in ids {
        let m = data
            .collect(
                &session,
                DataDraft {
                    reference: r.clone(),
                    kind: DataKind::Artifact,
                    parents: vec![],
                    payload: b"hello".to_vec(),
                    expires_at: 5000,
                },
            )
            .unwrap();
        data.set_policy(
            &owner,
            &r,
            m.revision,
            DataPermissions::new(vec![DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
        pins.push(SourcePin {
            reference: r,
            digest: m.digest,
        });
    }
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].sources = pins[..8].to_vec();
    let mut second = cfg.entries[0].clone();
    second.id = id("route:b");
    second.sources = vec![pins[8].clone()];
    cfg.entries.push(second);
    let (router, _) = Router::register(&mut data, &owner, &session, cfg, &mut control()).unwrap();
    assert_eq!(
        router
            .describe(&mut data, &session, &[id("route:a")], &mut control())
            .unwrap()
            .source_count(),
        8
    );
    assert!(matches!(
        router.describe(
            &mut data,
            &session,
            &[id("route:a"), id("route:b")],
            &mut control()
        ),
        Err(RouteError::Limit)
    ));
}

// Removing registration preflight must admit the malformed borrowed snapshots.
#[test]
fn resource_registration_bounds_equal_ids_before_comparison() {
    use eylema_runtime::{resource_estimate::Snapshot, routing::ResourceView};
    for (length, admitted) in [(128, true), (129, false), (8 * 1024 * 1024, false)] {
        let (mut data, owner, session, _, source) = fixture(b"hello".to_vec());
        let mut cfg = config(source);
        let first = "x".repeat(length);
        let second = first.clone();
        let base = *cfg.resources[0].snapshot;
        let snapshots: Vec<Snapshot<'_>> = (0..16)
            .map(|n| Snapshot {
                id: if n % 2 == 0 { &first } else { &second },
                revision: n + 1,
                ..base
            })
            .collect();
        cfg.resources = snapshots
            .iter()
            .map(|snapshot| ResourceView {
                snapshot,
                required_invocations: &[],
                required_auxiliary: &["aux:a"],
                valid_from: 500,
                valid_until: 5000,
            })
            .collect();
        let mut c = control();
        let result = Router::register(&mut data, &owner, &session, cfg, &mut c);
        assert_eq!(result.is_ok(), admitted, "snapshot ID bytes={length}");
        if !admitted {
            assert!(matches!(result, Err(RouteError::Limit)));
            assert!(
                c.work_used() <= 16_384,
                "must reject before source observation or comparisons"
            );
        }
    }
}

#[test]
fn resource_registration_bounds_required_map_before_source_observation() {
    use eylema_runtime::routing::ResourceView;
    let (mut data, owner, session, clock, source) = fixture(b"hello".to_vec());
    let mut cfg = config(source);
    let snapshot = cfg.resources[0].snapshot;
    let long = "x".repeat(129);
    let required = [long.as_str()];
    cfg.resources = vec![ResourceView {
        snapshot,
        required_invocations: &required,
        required_auxiliary: &[],
        valid_from: 500,
        valid_until: 5000,
    }];
    let observed = clock.2.get();
    assert!(matches!(
        Router::register(&mut data, &owner, &session, cfg, &mut control()),
        Err(RouteError::Limit)
    ));
    assert_eq!(clock.2.get(), observed);
}
