use eylema_contracts::{
    Revision,
    platform::{ProjectId, SemanticProfileId},
};
use eylema_runtime::routing::*;
fn id(s: &str) -> RouteId {
    SemanticProfileId::new(s).unwrap()
}
fn pin(s: &str) -> Pin {
    Pin {
        id: id(s),
        revision: Revision::new(1).unwrap(),
    }
}
fn entry(s: &str) -> Entry {
    Entry {
        id: id(s),
        revision: Revision::new(1).unwrap(),
        intent: Intent::Retrieval,
        backend: RouteBackend::Retrieval,
        sources: vec![],
        models: vec![],
        input_schema: id("schema:query"),
        output_schema: id("schema:hits"),
        quality: Stamp {
            pin: pin("quality:a"),
            valid_from: 500,
            valid_until: 2000,
        },
        context_limit: 4096,
        resource: pin("capacity:a"),
        grounding: SourceStatus::DeclaredConsistent,
    }
}
fn request() -> Request {
    Request {
        project: ProjectId::new("project:a").unwrap(),
        registry: pin("registry:a"),
        policy: pin("policy:a"),
        resource: pin("capacity:a"),
        quality: pin("quality:a"),
        intent: Intent::Retrieval,
        candidates: vec![],
        local_only: true,
        context: 4,
        response: 0,
        verified_tokens_required: false,
    }
}
fn run(scores: &[u32]) -> Choice {
    let entries: Vec<_> = scores
        .iter()
        .enumerate()
        .map(|(i, _)| entry(&format!("route:r{i}")))
        .collect();
    let mut r = request();
    r.candidates = entries.iter().map(|e| e.id.clone()).collect();
    let a: Vec<_> = entries
        .iter()
        .zip(scores)
        .map(|(e, s)| Assessment {
            route: e.id.clone(),
            utility: *s,
            score_profile: pin("score:a"),
            provenance: None,
        })
        .collect();
    let admitted: Vec<_> = entries
        .iter()
        .map(|e| Admission {
            route: e.id.clone(),
            rejection: None,
        })
        .collect();
    select(&entries, &r, &a, &admitted).unwrap()
}
#[test]
fn utility_boundaries_are_literal() {
    assert_eq!(run(&[600000, 400000]), Choice::Selected(id("route:r0")));
    assert_eq!(run(&[600000, 500000]), Choice::Selected(id("route:r0")));
    assert_eq!(run(&[600000, 500001]), Choice::Abstain);
    assert_eq!(run(&[500000]), Choice::Selected(id("route:r0")));
    assert_eq!(run(&[499999]), Choice::Abstain);
    assert_eq!(run(&[600000, 600000]), Choice::Abstain);
    assert_eq!(run(&[0]), Choice::Abstain);
    assert_eq!(run(&[1000000]), Choice::Selected(id("route:r0")));
}
#[test]
fn invalid_and_unsupported_routes_never_select() {
    let mut e = entry("route:a");
    let mut r = request();
    r.candidates = vec![e.id.clone()];
    let a = Assessment {
        route: e.id.clone(),
        utility: 600000,
        score_profile: pin("score:a"),
        provenance: None,
    };
    let ad = Admission {
        route: e.id.clone(),
        rejection: None,
    };
    e.grounding = SourceStatus::Contradictory;
    assert_eq!(
        select(&[e.clone()], &r, &[a.clone()], &[ad.clone()]).unwrap(),
        Choice::Abstain
    );
    e.grounding = SourceStatus::Unknown;
    assert_eq!(
        select(&[e.clone()], &r, &[a.clone()], &[ad.clone()]).unwrap(),
        Choice::Abstain
    );
    e.grounding = SourceStatus::DeclaredConsistent;
    e.backend = RouteBackend::Unsupported;
    assert_eq!(
        select(&[e.clone()], &r, &[a.clone()], &[ad.clone()]).unwrap(),
        Choice::Unavailable
    );
    e.backend = RouteBackend::Retrieval;
    r.context = 4096;
    r.response = 1;
    assert_eq!(
        select(&[e.clone()], &r, &[a.clone()], &[ad.clone()]).unwrap(),
        Choice::Unavailable
    );
    r.context = 4;
    r.response = 0;
    r.verified_tokens_required = true;
    assert_eq!(
        select(&[e.clone()], &r, &[a.clone()], &[ad.clone()]),
        Err(RoutingError::TokenCounterUnavailable)
    );
    r.verified_tokens_required = false;
    let mut bad = a.clone();
    bad.utility = 1000001;
    assert!(select(&[e.clone()], &r, &[bad], &[ad.clone()]).is_err());
    r.candidates.push(e.id.clone());
    assert!(select(&[e], &r, &[a], &[ad]).is_err());
}

#[test]
fn resource_identity_context_and_overlap_literals() {
    use eylema_runtime::resource_estimate::*;
    let evidence = Evidence {
        origin: Origin::SuppliedAssumption,
        source: "source:test",
        revision: 1,
        valid_from: 500,
        valid_until: 2000,
    };
    let known = |v| Claim::Known { value: v, evidence };
    let pools = [Pool {
        id: "pool:a",
        physical_id: "physical:a",
        kind: PoolKind::Unified,
        capacity: known(2000),
        available: known(1450),
        reserve: known(50),
    }];
    let key = InstanceKey {
        profile: "model:a",
        revision: 1,
        artifact: [1; 32],
        backend: "cpu",
        owner: "owner:a",
        pool: "pool:a",
    };
    let residents = [Resident {
        id: "resident:a",
        key,
        expected_key: key,
        weights: Claim::Known {
            value: Formula::Weights {
                resident_parameters: 1000,
                active_parameters: Some(1000),
                bits: 8,
                metadata: Claim::Known {
                    value: ByteInterval { lower: 0, upper: 0 },
                    evidence,
                },
            },
            evidence,
        },
    }];
    let state = Claim::Known {
        value: Formula::Kv {
            context: 4,
            batch: 1,
            layers: 1,
            kv_heads: 2,
            head_width: 4,
            bits: 16,
        },
        evidence,
    };
    let workspace = Claim::Known {
        value: Formula::Bytes(ByteInterval {
            lower: 64,
            upper: 64,
        }),
        evidence,
    };
    let invocations = [
        Invocation {
            id: "invoke:a",
            resident: "resident:a",
            pool: "pool:a",
            state,
            workspace,
        },
        Invocation {
            id: "invoke:b",
            resident: "resident:a",
            pool: "pool:a",
            state,
            workspace,
        },
    ];
    let aux = [Auxiliary {
        id: "aux:a",
        pool: "pool:a",
        kind: AuxiliaryKind::RagIndex,
        lifetime: Lifetime::Persistent,
        size: Claim::Known {
            value: Formula::Bytes(ByteInterval {
                lower: 16,
                upper: 16,
            }),
            evidence,
        },
    }];
    let members = [
        Member::Invocation("invoke:a"),
        Member::Invocation("invoke:b"),
    ];
    let overlaps = [Overlap {
        id: "overlap:a",
        members: &members,
    }];
    let mut snapshot = Snapshot {
        id: "capacity:a",
        expected_id: "capacity:a",
        revision: 1,
        expected_revision: 1,
        pools: &pools,
        residents: &residents,
        invocations: &invocations,
        auxiliary: &aux,
        overlaps: Some(&overlaps),
    };
    let mut e = entry("route:a");
    let r = request();
    let view = ResourceView {
        snapshot: &snapshot,
        required_invocations: &["invoke:a", "invoke:b"],
        required_auxiliary: &["aux:a"],
        valid_from: 500,
        valid_until: 2000,
    };
    assert_eq!(
        estimate(&snapshot, 1000).unwrap().pools[0].unwrap().total,
        Some(ByteInterval {
            lower: 1400,
            upper: 1400
        })
    );
    assert_eq!(check_resources(&view, &e, &r, 1000), Ok(()));
    assert!(check_resources(&view, &e, &r, 2001).is_err());
    e.resource.revision = Revision::new(2).unwrap();
    assert!(check_resources(&view, &e, &r, 1000).is_err());
    e.resource.revision = Revision::new(1).unwrap();
    let mut small = pools;
    small[0].available = known(1449);
    snapshot.pools = &small;
    let view = ResourceView {
        snapshot: &snapshot,
        required_invocations: &["invoke:a", "invoke:b"],
        required_auxiliary: &["aux:a"],
        valid_from: 500,
        valid_until: 2000,
    };
    assert!(check_resources(&view, &e, &r, 1000).is_err());
    snapshot.pools = &pools;
    snapshot.invocations = &invocations[..1];
    let one_member = [Member::Invocation("invoke:a")];
    let one = [Overlap {
        id: "overlap:a",
        members: &one_member,
    }];
    snapshot.overlaps = Some(&one);
    assert_eq!(
        estimate(&snapshot, 1000).unwrap().pools[0].unwrap().total,
        Some(ByteInterval {
            lower: 1208,
            upper: 1208
        })
    );
    snapshot.invocations = &invocations;
    let view = ResourceView {
        snapshot: &snapshot,
        required_invocations: &["invoke:a", "invoke:b"],
        required_auxiliary: &["aux:a"],
        valid_from: 500,
        valid_until: 2000,
    };
    assert!(check_resources(&view, &e, &r, 1000).is_err());
}
#[test]
fn permutation_and_profile_mismatch_are_not_tiebreak_shortcuts() {
    let es = vec![entry("route:a"), entry("route:b")];
    let mut r = request();
    r.candidates = vec![id("route:b"), id("route:a")];
    let mut a = vec![
        Assessment {
            route: id("route:a"),
            utility: 700000,
            score_profile: pin("score:a"),
            provenance: None,
        },
        Assessment {
            route: id("route:b"),
            utility: 500000,
            score_profile: pin("score:a"),
            provenance: None,
        },
    ];
    let ad = vec![
        Admission {
            route: id("route:b"),
            rejection: None,
        },
        Admission {
            route: id("route:a"),
            rejection: None,
        },
    ];
    assert_eq!(
        select(&es, &r, &a, &ad),
        Ok(Choice::Selected(id("route:a")))
    );
    a[1].score_profile.revision = Revision::new(2).unwrap();
    assert_eq!(select(&es, &r, &a, &ad), Err(RoutingError::InvalidInput));
    a[1].score_profile = pin("score:a");
    r.context = u64::MAX;
    r.response = 1;
    assert_eq!(select(&es, &r, &a, &ad), Err(RoutingError::ContextLimit));
    r.context = 4;
    r.response = 0;
    r.candidates = vec![id("route:a"); 9];
    assert!(select(&es, &r, &a, &ad).is_err());
}
