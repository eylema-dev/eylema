use eylema_runtime::resource_estimate::*;

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
