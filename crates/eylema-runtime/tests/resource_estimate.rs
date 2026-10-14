use eylema_runtime::resource_estimate::*;

fn key<'a>(pool: &'a str, owner: &'a str) -> InstanceKey<'a> {
    InstanceKey {
        profile: "profile.v1",
        revision: 1,
        artifact: [1; 32],
        backend: "what-if",
        owner,
        pool,
    }
}
fn resident<'a>(id: &'a str, k: InstanceKey<'a>, weight: u64) -> Resident<'a> {
    Resident {
        id,
        key: k,
        expected_key: k,
        weights: known(Formula::Bytes(bytes(weight, weight))),
    }
}
fn pool(id: &'static str, kind: PoolKind, available: u64) -> Pool<'static> {
    Pool {
        id,
        physical_id: id,
        kind,
        capacity: known(10000),
        available: known(available),
        reserve: known(0),
    }
}
fn invocation(id: &'static str, state: u64, workspace: u64) -> Invocation<'static> {
    Invocation {
        id,
        resident: "r1",
        pool: "unified",
        state: known(Formula::Bytes(bytes(state, state))),
        workspace: known(Formula::Bytes(bytes(workspace, workspace))),
    }
}
fn auxiliary(
    id: &'static str,
    pool: &'static str,
    lifetime: Lifetime,
    lower: u64,
    upper: u64,
) -> Auxiliary<'static> {
    Auxiliary {
        id,
        pool,
        lifetime,
        kind: AuxiliaryKind::Modality,
        size: known(Formula::Bytes(bytes(lower, upper))),
    }
}
struct Portfolio<'a> {
    pools: Vec<Pool<'a>>,
    residents: Vec<Resident<'a>>,
    invocations: Vec<Invocation<'a>>,
    auxiliary: Vec<Auxiliary<'a>>,
    overlaps: Vec<Overlap<'a>>,
}
impl Portfolio<'static> {
    fn basic() -> Self {
        Self {
            pools: vec![pool("unified", PoolKind::Unified, 10000)],
            residents: vec![resident("r1", key("unified", "owner-a"), 1000)],
            invocations: vec![invocation("i1", 100, 10), invocation("i2", 200, 20)],
            auxiliary: vec![],
            overlaps: vec![Overlap {
                id: "joint",
                members: &[Member::Invocation("i1"), Member::Invocation("i2")],
            }],
        }
    }
}
impl Portfolio<'_> {
    fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            id: "snapshot.v1",
            expected_id: "snapshot.v1",
            revision: 1,
            expected_revision: 1,
            pools: &self.pools,
            residents: &self.residents,
            invocations: &self.invocations,
            auxiliary: &self.auxiliary,
            overlaps: Some(&self.overlaps),
        }
    }
}
fn total(p: &Portfolio<'_>, want: Option<ByteInterval>, verdict: Verdict) {
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.pools[0].unwrap().total, want);
    assert_eq!(report.pools[0].unwrap().verdict, verdict);
    assert!(report.planning_only);
}

#[test]
fn portfolio_overlap_and_sharing() {
    let mut p = Portfolio::basic();
    total(&p, Some(bytes(1330, 1330)), Verdict::EstimatedFit);
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.component_count, 5);
    assert!(std::ptr::eq(report.snapshot, &s));
    assert_eq!(report.components[0].unwrap().evidence, Some(evidence()));
    p.overlaps = vec![
        Overlap {
            id: "one",
            members: &[Member::Invocation("i1")],
        },
        Overlap {
            id: "two",
            members: &[Member::Invocation("i2")],
        },
    ];
    total(&p, Some(bytes(1220, 1220)), Verdict::EstimatedFit);
    p.overlaps = vec![Overlap {
        id: "joint",
        members: &[Member::Invocation("i1"), Member::Invocation("i2")],
    }];
    p.auxiliary.push(auxiliary(
        "persist",
        "unified",
        Lifetime::Persistent,
        30,
        30,
    ));
    total(&p, Some(bytes(1360, 1360)), Verdict::EstimatedFit);
    p.auxiliary.clear();
    p.residents
        .push(resident("r2", key("unified", "owner-b"), 1000));
    p.invocations[1].resident = "r2";
    total(&p, Some(bytes(2330, 2330)), Verdict::EstimatedFit);
    p.pools.push(pool("disk", PoolKind::Storage, 100));
    p.auxiliary
        .push(auxiliary("sources", "disk", Lifetime::Persistent, 50, 50));
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.pools[1].unwrap().total, Some(bytes(50, 50)));
    assert_eq!(report.pools[0].unwrap().total, Some(bytes(2330, 2330)));
    // Each part of the exact key independently distinguishes separate instances.
    for field in 0..4 {
        let mut p = Portfolio::basic();
        let mut k = key("unified", "owner-a");
        match field {
            0 => k.artifact = [2; 32],
            1 => k.revision = 2,
            2 => k.backend = "other",
            _ => k.profile = "profile.v2",
        }
        p.residents.push(resident("r2", k, 1000));
        p.invocations[1].resident = "r2";
        total(&p, Some(bytes(2330, 2330)), Verdict::EstimatedFit);
    }
}

#[test]
fn portfolio_uncertainty_and_pools() {
    let mut p = Portfolio::basic();
    p.residents.clear();
    p.invocations.clear();
    p.overlaps.clear();
    p.pools[0].available = known(100);
    p.auxiliary.push(auxiliary(
        "required",
        "unified",
        Lifetime::Persistent,
        90,
        110,
    ));
    total(&p, Some(bytes(90, 110)), Verdict::Unknown);
    p.auxiliary[0].size = known(Formula::Bytes(bytes(100, 100)));
    total(&p, Some(bytes(100, 100)), Verdict::EstimatedFit);
    p.pools[0].available = known(120);
    p.pools[0].reserve = known(20);
    total(&p, Some(bytes(100, 100)), Verdict::EstimatedFit);
    assert_eq!(
        estimate(&p.snapshot(), 15).unwrap().pools[0]
            .unwrap()
            .usable_budget,
        Some(100)
    );
    p.auxiliary[0].size = known(Formula::Bytes(bytes(101, 101)));
    total(&p, Some(bytes(101, 101)), Verdict::ExceedsDeclaredBudget);
    p.pools[0].available = Claim::Unknown {
        reason: UnknownReason::MissingBudget,
        source: "available",
    };
    total(&p, Some(bytes(101, 101)), Verdict::Unknown);
    let p = Portfolio::basic();
    let mut s = p.snapshot();
    s.overlaps = None;
    let r = estimate(&s, 15).unwrap();
    assert_eq!(r.pools[0].unwrap().total, None);
    assert!(matches!(
        r.snapshot_status,
        SizeStatus::Unknown {
            reason: UnknownReason::MissingOverlap,
            ..
        }
    ));
    let mut p = Portfolio::basic();
    p.overlaps[0].members = &[Member::Invocation("i1")];
    total(&p, None, Verdict::Unknown);
    let p = Portfolio::basic();
    let mut s = p.snapshot();
    s.expected_revision = 2;
    assert_eq!(estimate(&s, 15).unwrap().verdict, Verdict::Unknown);
    for change in 0..3 {
        let mut p = Portfolio::basic();
        match change {
            0 => p.residents[0].expected_key.revision = 2,
            1 => p.residents[0].key.artifact = [0; 32],
            _ => {
                p.residents[0].weights = Claim::Known {
                    value: Formula::Bytes(bytes(1000, 1000)),
                    evidence: Evidence {
                        valid_until: 14,
                        ..evidence()
                    },
                }
            }
        }
        total(&p, None, Verdict::Unknown);
        assert!(matches!(
            estimate(&p.snapshot(), 15).unwrap().components[1]
                .unwrap()
                .status,
            SizeStatus::Unknown { .. }
        ));
    }
    let mut p = Portfolio::basic();
    p.invocations[0].state = known(Formula::Weights {
        resident_parameters: 1,
        active_parameters: None,
        bits: 16,
        metadata: Claim::Known {
            value: bytes(0, 0),
            evidence: Evidence {
                valid_until: 14,
                ..evidence()
            },
        },
    });
    total(&p, None, Verdict::Unknown);
    let mut p = Portfolio::basic();
    p.invocations.clear();
    p.residents.clear();
    p.overlaps.clear();
    p.pools = vec![
        pool("ram", PoolKind::HostRam, 200),
        pool("vram", PoolKind::DeviceVram, 50),
    ];
    p.auxiliary = vec![
        auxiliary("host", "ram", Lifetime::Persistent, 10, 10),
        auxiliary("device", "vram", Lifetime::Persistent, 60, 60),
    ];
    let s = p.snapshot();
    let r = estimate(&s, 15).unwrap();
    assert_eq!(r.verdict, Verdict::ExceedsDeclaredBudget);
    assert_eq!(r.pools[0].unwrap().verdict, Verdict::EstimatedFit);
    assert_eq!(r.pools[1].unwrap().verdict, Verdict::ExceedsDeclaredBudget);
}

#[test]
fn portfolio_rejects_conflicts_and_overflow() {
    for case in 0..12 {
        let mut p = Portfolio::basic();
        let want = match case {
            0 => {
                p.pools.push(Pool {
                    id: "alias",
                    ..p.pools[0]
                });
                EstimateError::ConflictingIdentity
            }
            1 => {
                p.overlaps[0].members = &[Member::Invocation("i1"), Member::Invocation("i1")];
                EstimateError::DuplicateIdentity
            }
            2 => {
                p.residents.push(Resident {
                    id: "r2",
                    ..p.residents[0]
                });
                EstimateError::DuplicateIdentity
            }
            3 => {
                p.invocations[0].pool = "missing";
                EstimateError::DanglingReference
            }
            4 => {
                p.overlaps[0].members = &[Member::Invocation("missing")];
                EstimateError::DanglingReference
            }
            5 => {
                p.pools[0].capacity = known(10);
                EstimateError::ConflictingIdentity
            }
            6 => {
                p.auxiliary
                    .push(auxiliary("a", "unified", Lifetime::Persistent, 1, 1));
                p.overlaps[0].members = &[Member::Auxiliary("a")];
                EstimateError::InvalidInput
            }
            7 => {
                p.residents[0].weights = known(Formula::NotApplicable);
                EstimateError::InvalidInput
            }
            8 => {
                p.auxiliary.push(auxiliary(
                    "a",
                    "unified",
                    Lifetime::Persistent,
                    u64::MAX,
                    u64::MAX,
                ));
                EstimateError::Overflow
            }
            9 => {
                p.pools[0].reserve = known(10001);
                EstimateError::ConflictingIdentity
            }
            10 => {
                p.auxiliary
                    .push(auxiliary("i1", "unified", Lifetime::Persistent, 1, 1));
                EstimateError::DuplicateIdentity
            }
            _ => {
                p.pools.clear();
                EstimateError::InvalidInput
            }
        };
        assert_eq!(
            estimate(&p.snapshot(), 15).unwrap_err(),
            want,
            "case {case}"
        );
    }
    let mut p = Portfolio::basic();
    p.overlaps.clear();
    assert_eq!(
        estimate(&p.snapshot(), 15).unwrap_err(),
        EstimateError::InvalidInput
    );
    let mut p = Portfolio::basic();
    p.invocations.clear();
    p.residents.clear();
    p.overlaps.clear();
    total(&p, Some(bytes(0, 0)), Verdict::EstimatedFit);
    let mut s = p.snapshot();
    s.overlaps = None;
    assert_eq!(estimate(&s, 15).unwrap().verdict, Verdict::Unknown);
}

#[test]
fn portfolio_cardinality_bounds() {
    for case in 0..6 {
        let mut p = Portfolio::basic();
        match case {
            0 => p.residents = vec![p.residents[0]; 17],
            1 => p.invocations = vec![p.invocations[0]; 33],
            2 => p.overlaps = vec![p.overlaps[0]; 33],
            3 => p.auxiliary = vec![auxiliary("a", "unified", Lifetime::Persistent, 1, 1); 65],
            4 => p.pools = vec![p.pools[0]; 9],
            _ => p.overlaps[0].members = &[Member::Invocation("i1"); 33],
        }
        assert_eq!(
            estimate(&p.snapshot(), 15).unwrap_err(),
            EstimateError::LimitExceeded
        );
    }
    let ids: Vec<String> = (0..112).map(|i| format!("entry{i}")).collect();
    let group_ids: Vec<String> = (0..32).map(|i| format!("group{i}")).collect();
    let residents: Vec<_> = ids[..16]
        .iter()
        .map(|id| resident(id, key("unified", id), 1))
        .collect();
    let invocations: Vec<_> = ids[16..48]
        .iter()
        .map(|id| Invocation {
            id,
            resident: &ids[0],
            pool: "unified",
            state: known(Formula::Bytes(bytes(1, 1))),
            workspace: known(Formula::NotApplicable),
        })
        .collect();
    let aux: Vec<_> = ids[48..]
        .iter()
        .map(|id| Auxiliary {
            id,
            pool: "unified",
            kind: AuxiliaryKind::Modality,
            lifetime: Lifetime::Persistent,
            size: known(Formula::Bytes(bytes(1, 1))),
        })
        .collect();
    let members: Vec<_> = ids[16..48]
        .iter()
        .map(|id| Member::Invocation(id))
        .collect();
    let groups: Vec<_> = group_ids
        .iter()
        .map(|id| Overlap {
            id,
            members: &members,
        })
        .collect();
    let pools = [pool("unified", PoolKind::Unified, 1000)];
    let s = Snapshot {
        id: "max",
        expected_id: "max",
        revision: 1,
        expected_revision: 1,
        pools: &pools,
        residents: &residents,
        invocations: &invocations,
        auxiliary: &aux,
        overlaps: Some(&groups),
    };
    let r = estimate(&s, 15).unwrap();
    assert_eq!(r.component_count, 144);
    assert_eq!(r.pools[0].unwrap().total, Some(bytes(112, 112)));
    assert_eq!(r.verdict, Verdict::EstimatedFit);
}

#[test]
fn rag_qlora_and_modal_workload() {
    let mut p = Portfolio::basic();
    p.pools = vec![
        pool("unified", PoolKind::Unified, 1000),
        pool("disk", PoolKind::Storage, 400),
    ];
    p.residents[0].weights = known(Formula::Quantized {
        parameters: 10,
        bits: 4,
        group_size: 4,
        scale_bytes: 2,
        metadata: known(bytes(1, 3)),
    });
    p.invocations = vec![invocation("i1", 100, 10)];
    let targets = [LoraTarget {
        input: 4,
        output: 6,
        rank: 2,
    }];
    p.auxiliary = vec![
        auxiliary("adapter", "unified", Lifetime::Persistent, 40, 40),
        auxiliary("gradient", "unified", Lifetime::Persistent, 40, 40),
        auxiliary("optimizer", "unified", Lifetime::Persistent, 80, 80),
        auxiliary("recurrent", "unified", Lifetime::Persistent, 72, 72),
        auxiliary("vectors", "unified", Lifetime::Persistent, 48, 48),
        auxiliary("index", "unified", Lifetime::Persistent, 16, 16),
        auxiliary("checkpoint", "unified", Lifetime::Transient, 20, 20),
        auxiliary("modality", "unified", Lifetime::Transient, 8, 8),
        auxiliary("working", "unified", Lifetime::Transient, 4, 4),
        auxiliary("source", "disk", Lifetime::Persistent, 300, 300),
    ];
    p.auxiliary[0].kind = AuxiliaryKind::Adapter;
    p.auxiliary[1].kind = AuxiliaryKind::Gradient;
    p.auxiliary[2].kind = AuxiliaryKind::Optimizer;
    p.auxiliary[3].kind = AuxiliaryKind::ModelState;
    p.auxiliary[4].kind = AuxiliaryKind::RagIndex;
    p.auxiliary[0].size = known(Formula::Lora {
        targets: &targets,
        bits: 16,
        copies: Ratio {
            numerator: 1,
            denominator: 1,
        },
    });
    p.auxiliary[1].size = known(Formula::Lora {
        targets: &targets,
        bits: 16,
        copies: Ratio {
            numerator: 1,
            denominator: 1,
        },
    });
    p.auxiliary[2].size = known(Formula::Lora {
        targets: &targets,
        bits: 16,
        copies: Ratio {
            numerator: 2,
            denominator: 1,
        },
    });
    p.auxiliary[3].size = known(Formula::Recurrent {
        batch: 2,
        state_elements: 12,
        bits: 16,
        coefficient: Ratio {
            numerator: 3,
            denominator: 2,
        },
    });
    p.auxiliary[4].size = known(Formula::RagVectors {
        count: 3,
        dimensions: 4,
        bits: 32,
    });
    p.overlaps = vec![Overlap {
        id: "train",
        members: &[
            Member::Invocation("i1"),
            Member::Auxiliary("checkpoint"),
            Member::Auxiliary("modality"),
            Member::Auxiliary("working"),
        ],
    }];
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.pools[0].unwrap().total, Some(bytes(450, 452)));
    assert_eq!(report.pools[1].unwrap().total, Some(bytes(300, 300)));
    assert_eq!(report.verdict, Verdict::EstimatedFit);
    assert!(report.planning_only);
    p.auxiliary[2].size = Claim::Unknown {
        reason: UnknownReason::MissingCoefficient,
        source: "optimizer",
    };
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.pools[0].unwrap().total, None);
    assert_eq!(report.pools[0].unwrap().verdict, Verdict::Unknown);
    assert_eq!(report.pools[1].unwrap().verdict, Verdict::EstimatedFit);
    p.auxiliary[2].size = known(Formula::Bytes(bytes(80, 80)));
    p.auxiliary[3].size = known(Formula::Unsupported(UnknownReason::UnsupportedArchitecture));
    total(&p, None, Verdict::Unknown);

    let mut p = Portfolio::basic();
    p.residents.clear();
    p.invocations.clear();
    p.auxiliary = vec![
        auxiliary("source", "unified", Lifetime::Transient, 12, 12),
        auxiliary("destination", "unified", Lifetime::Transient, 14, 14),
        auxiliary("staging", "unified", Lifetime::Transient, 8, 8),
    ];
    p.overlaps = vec![Overlap {
        id: "convert",
        members: &[
            Member::Auxiliary("source"),
            Member::Auxiliary("destination"),
            Member::Auxiliary("staging"),
        ],
    }];
    total(&p, Some(bytes(34, 34)), Verdict::EstimatedFit);
    p.pools = vec![
        pool("ram", PoolKind::HostRam, 100),
        pool("vram", PoolKind::DeviceVram, 100),
    ];
    p.auxiliary[0].pool = "ram";
    p.auxiliary[1].pool = "vram";
    p.auxiliary[2].pool = "vram";
    let s = p.snapshot();
    let report = estimate(&s, 15).unwrap();
    assert_eq!(report.pools[0].unwrap().total, Some(bytes(12, 12)));
    assert_eq!(report.pools[1].unwrap().total, Some(bytes(22, 22)));
}

#[test]
fn maximum_pool_and_target_counts_remain_valid() {
    let targets = [LoraTarget {
        input: 4,
        output: 6,
        rank: 2,
    }; 32];
    assert_eq!(
        size(Formula::Lora {
            targets: &targets,
            bits: 16,
            copies: Ratio {
                numerator: 1,
                denominator: 1
            }
        }),
        Ok(SizeStatus::Known(bytes(1280, 1280)))
    );
    let ids = ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7"];
    let pools: Vec<_> = ids
        .iter()
        .map(|id| pool(id, PoolKind::HostRam, 100))
        .collect();
    let snapshot = Snapshot {
        id: "pools",
        expected_id: "pools",
        revision: 1,
        expected_revision: 1,
        pools: &pools,
        residents: &[],
        invocations: &[],
        auxiliary: &[],
        overlaps: Some(&[]),
    };
    let report = estimate(&snapshot, 15).unwrap();
    assert_eq!(report.pool_count, 8);
    assert_eq!(report.verdict, Verdict::EstimatedFit);
    assert_eq!(report.pools[7].unwrap().total, Some(bytes(0, 0)));
}

fn evidence() -> Evidence<'static> {
    Evidence {
        origin: Origin::SuppliedAssumption,
        source: "fixture.v1",
        revision: 1,
        valid_from: 10,
        valid_until: 20,
    }
}
fn known<T>(value: T) -> Claim<'static, T> {
    Claim::Known {
        value,
        evidence: evidence(),
    }
}
fn bytes(lower: u64, upper: u64) -> ByteInterval {
    ByteInterval { lower, upper }
}
fn size(f: Formula<'_>) -> Result<SizeStatus<'_>, EstimateError> {
    component_interval(
        Claim::Known {
            value: f,
            evidence: evidence(),
        },
        15,
    )
}

// Catches floor-packing, forgotten K/V duplication, active-MoE substitution,
// missing group-tail scales, and implicit optimizer/recurrent coefficients.
#[test]
fn component_arithmetic() {
    let targets = [LoraTarget {
        input: 4,
        output: 6,
        rank: 2,
    }];
    let fixtures = [
        (
            Formula::Packed {
                elements: 3,
                bits: 4,
            },
            bytes(2, 2),
        ),
        (
            Formula::Scaled {
                bytes: bytes(3, 5),
                ratio: Ratio {
                    numerator: 3,
                    denominator: 2,
                },
            },
            bytes(5, 8),
        ),
        (
            Formula::Weights {
                resident_parameters: 1000,
                active_parameters: None,
                bits: 16,
                metadata: known(bytes(10, 20)),
            },
            bytes(2010, 2020),
        ),
        (
            Formula::Weights {
                resident_parameters: 1000,
                active_parameters: Some(100),
                bits: 16,
                metadata: known(bytes(10, 20)),
            },
            bytes(2010, 2020),
        ),
        (
            Formula::Quantized {
                parameters: 10,
                bits: 4,
                group_size: 4,
                scale_bytes: 2,
                metadata: known(bytes(1, 3)),
            },
            bytes(12, 14),
        ),
        (
            Formula::Kv {
                context: 8,
                batch: 2,
                layers: 3,
                kv_heads: 2,
                head_width: 4,
                bits: 16,
            },
            bytes(1536, 1536),
        ),
        (
            Formula::Recurrent {
                batch: 2,
                state_elements: 12,
                bits: 16,
                coefficient: Ratio {
                    numerator: 3,
                    denominator: 2,
                },
            },
            bytes(72, 72),
        ),
        (
            Formula::RagVectors {
                count: 3,
                dimensions: 4,
                bits: 32,
            },
            bytes(48, 48),
        ),
        (
            Formula::Lora {
                targets: &targets,
                bits: 16,
                copies: Ratio {
                    numerator: 3,
                    denominator: 1,
                },
            },
            bytes(120, 120),
        ),
    ];
    for (formula, want) in fixtures {
        assert_eq!(size(formula), Ok(SizeStatus::Known(want)));
    }
    assert_eq!(
        size(Formula::Bytes(bytes(0, 0))),
        Ok(SizeStatus::Known(bytes(0, 0)))
    );
    assert_eq!(ByteInterval::new(3, 5), Ok(bytes(3, 5)));
    assert_eq!(ByteInterval::new(5, 3), Err(EstimateError::InvalidInput));
}

#[test]
fn component_rejection_and_unknown() {
    assert_eq!(
        size(Formula::Packed {
            elements: u64::MAX,
            bits: 16
        }),
        Err(EstimateError::Overflow)
    );
    let invalid_targets = [LoraTarget {
        input: 4,
        output: 6,
        rank: 5,
    }];
    let zero_targets = [LoraTarget {
        input: 4,
        output: 6,
        rank: 0,
    }];
    let too_many = [LoraTarget {
        input: 4,
        output: 6,
        rank: 2,
    }; 33];
    for f in [
        Formula::Packed {
            elements: 1,
            bits: 0,
        },
        Formula::Packed {
            elements: 1,
            bits: 65,
        },
        Formula::Packed {
            elements: 0,
            bits: 8,
        },
        Formula::Bytes(bytes(5, 3)),
        Formula::Scaled {
            bytes: bytes(1, 2),
            ratio: Ratio {
                numerator: 1,
                denominator: 0,
            },
        },
        Formula::Scaled {
            bytes: bytes(1, 2),
            ratio: Ratio {
                numerator: 0,
                denominator: 1,
            },
        },
        Formula::Lora {
            targets: &invalid_targets,
            bits: 16,
            copies: Ratio {
                numerator: 1,
                denominator: 1,
            },
        },
        Formula::Lora {
            targets: &zero_targets,
            bits: 16,
            copies: Ratio {
                numerator: 1,
                denominator: 1,
            },
        },
        Formula::Lora {
            targets: &[],
            bits: 16,
            copies: Ratio {
                numerator: 1,
                denominator: 1,
            },
        },
        Formula::Weights {
            resident_parameters: 2,
            active_parameters: Some(3),
            bits: 16,
            metadata: known(bytes(0, 0)),
        },
        Formula::Quantized {
            parameters: 10,
            bits: 4,
            group_size: 0,
            scale_bytes: 2,
            metadata: known(bytes(0, 0)),
        },
        Formula::Kv {
            context: 0,
            batch: 1,
            layers: 1,
            kv_heads: 1,
            head_width: 1,
            bits: 16,
        },
        Formula::Recurrent {
            batch: 1,
            state_elements: 0,
            bits: 16,
            coefficient: Ratio {
                numerator: 1,
                denominator: 1,
            },
        },
        Formula::RagVectors {
            count: 1,
            dimensions: 0,
            bits: 16,
        },
    ] {
        assert_eq!(size(f), Err(EstimateError::InvalidInput));
    }
    assert_eq!(
        size(Formula::Lora {
            targets: &too_many,
            bits: 16,
            copies: Ratio {
                numerator: 1,
                denominator: 1
            }
        }),
        Err(EstimateError::LimitExceeded)
    );
    let missing = Claim::Unknown {
        reason: UnknownReason::MissingCoefficient,
        source: "optimizer",
    };
    assert_eq!(
        size(Formula::Quantized {
            parameters: 10,
            bits: 4,
            group_size: 4,
            scale_bytes: 2,
            metadata: missing
        }),
        Ok(SizeStatus::Unknown {
            reason: UnknownReason::MissingCoefficient,
            source: "optimizer"
        })
    );
    assert_eq!(
        size(Formula::Unsupported(UnknownReason::UnsupportedArchitecture)),
        Ok(SizeStatus::Unknown {
            reason: UnknownReason::UnsupportedArchitecture,
            source: "fixture.v1"
        })
    );
    assert_eq!(size(Formula::NotApplicable), Ok(SizeStatus::NotApplicable));
    for now in [10, 20] {
        assert_eq!(
            component_interval(known(Formula::Bytes(bytes(1, 1))), now),
            Ok(SizeStatus::Known(bytes(1, 1)))
        );
    }
    for now in [9, 21] {
        assert_eq!(
            component_interval(known(Formula::Bytes(bytes(1, 1))), now),
            Ok(SizeStatus::Unknown {
                reason: UnknownReason::StaleEvidence,
                source: "fixture.v1"
            })
        );
    }
    for e in [
        Evidence {
            revision: 0,
            ..evidence()
        },
        Evidence {
            source: "bad space",
            ..evidence()
        },
        Evidence {
            valid_from: 21,
            ..evidence()
        },
    ] {
        assert_eq!(
            component_interval(
                Claim::Known {
                    value: Formula::Bytes(bytes(1, 1)),
                    evidence: e
                },
                15
            ),
            Err(EstimateError::InvalidInput)
        );
    }
    assert_eq!(
        component_interval(
            Claim::Unknown {
                reason: UnknownReason::MissingIdentity,
                source: ""
            },
            15
        ),
        Ok(SizeStatus::Unknown {
            reason: UnknownReason::MissingIdentity,
            source: ""
        })
    );
    // A stale declaration cannot hide a malformed numerical input.
    assert_eq!(
        component_interval(
            known(Formula::Packed {
                elements: 0,
                bits: 16
            }),
            21
        ),
        Err(EstimateError::InvalidInput)
    );
}
