//! Pure declared resource sizing. No device allocation, scheduling, inference,
//! training, source authentication, or execution permission is supplied here.
//! Intervals are conservative input declarations, not measured performance.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteInterval {
    pub lower: u64,
    pub upper: u64,
}
impl ByteInterval {
    pub fn new(lower: u64, upper: u64) -> Result<Self> {
        if lower > upper {
            return Err(EstimateError::InvalidInput);
        }
        Ok(Self { lower, upper })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EstimateError {
    InvalidInput,
    LimitExceeded,
    DuplicateIdentity,
    DanglingReference,
    ConflictingIdentity,
    Overflow,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    MissingCoefficient,
    UnsupportedLayout,
    UnsupportedArchitecture,
    MissingOverlap,
    MissingBudget,
    StaleEvidence,
    IdentityMismatch,
    MissingIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    MeasuredArtifact,
    PublisherDeclaration,
    SuppliedAssumption,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Evidence<'a> {
    pub origin: Origin,
    pub source: &'a str,
    pub revision: u64,
    pub valid_from: u64,
    pub valid_until: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claim<'a, T> {
    Known {
        value: T,
        evidence: Evidence<'a>,
    },
    Unknown {
        reason: UnknownReason,
        source: &'a str,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SizeStatus<'a> {
    Known(ByteInterval),
    Unknown {
        reason: UnknownReason,
        source: &'a str,
    },
    NotApplicable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoraTarget {
    pub input: u64,
    pub output: u64,
    pub rank: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Formula<'a> {
    Bytes(ByteInterval),
    Packed {
        elements: u64,
        bits: u32,
    },
    Scaled {
        bytes: ByteInterval,
        ratio: Ratio,
    },
    Weights {
        resident_parameters: u64,
        active_parameters: Option<u64>,
        bits: u32,
        metadata: Claim<'a, ByteInterval>,
    },
    Quantized {
        parameters: u64,
        bits: u32,
        group_size: u64,
        scale_bytes: u64,
        metadata: Claim<'a, ByteInterval>,
    },
    Kv {
        context: u64,
        batch: u64,
        layers: u64,
        kv_heads: u64,
        head_width: u64,
        bits: u32,
    },
    Recurrent {
        batch: u64,
        state_elements: u64,
        bits: u32,
        coefficient: Ratio,
    },
    RagVectors {
        count: u64,
        dimensions: u64,
        bits: u32,
    },
    Lora {
        targets: &'a [LoraTarget],
        bits: u32,
        copies: Ratio,
    },
    Unsupported(UnknownReason),
    NotApplicable,
}
type Result<T> = std::result::Result<T, EstimateError>;
fn identity(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/@-".contains(&b))
    {
        return Err(EstimateError::InvalidInput);
    }
    Ok(())
}
fn unknown_identity(reason: UnknownReason, source: &str) -> Result<()> {
    if reason == UnknownReason::MissingIdentity && source.is_empty() {
        Ok(())
    } else {
        identity(source)
    }
}
fn validate_evidence(e: Evidence<'_>) -> Result<()> {
    identity(e.source)?;
    if e.revision == 0 || e.valid_from > e.valid_until {
        return Err(EstimateError::InvalidInput);
    }
    Ok(())
}
fn fresh(e: Evidence<'_>, now: u64) -> bool {
    e.valid_from <= now && now <= e.valid_until
}
fn validate_claim<T>(c: Claim<'_, T>, validate: impl FnOnce(T) -> Result<()>) -> Result<()> {
    match c {
        Claim::Known { value, evidence } => {
            validate_evidence(evidence)?;
            validate(value)
        }
        Claim::Unknown { reason, source } => unknown_identity(reason, source),
    }
}
fn interval(b: ByteInterval) -> Result<()> {
    ByteInterval::new(b.lower, b.upper).map(|_| ())
}
fn positive(values: &[u64]) -> Result<()> {
    if values.contains(&0) {
        Err(EstimateError::InvalidInput)
    } else {
        Ok(())
    }
}
fn precision(bits: u32) -> Result<()> {
    if (1..=64).contains(&bits) {
        Ok(())
    } else {
        Err(EstimateError::InvalidInput)
    }
}
fn ratio(r: Ratio) -> Result<()> {
    positive(&[r.numerator, r.denominator])
}
fn validate_formula(f: Formula<'_>) -> Result<()> {
    match f {
        Formula::Bytes(b) => interval(b),
        Formula::Packed { elements, bits } => {
            positive(&[elements])?;
            precision(bits)
        }
        Formula::Scaled { bytes, ratio: r } => {
            interval(bytes)?;
            ratio(r)
        }
        Formula::Weights {
            resident_parameters,
            active_parameters,
            bits,
            metadata,
        } => {
            positive(&[resident_parameters])?;
            precision(bits)?;
            if active_parameters.is_some_and(|v| v == 0 || v > resident_parameters) {
                return Err(EstimateError::InvalidInput);
            }
            validate_claim(metadata, interval)
        }
        Formula::Quantized {
            parameters,
            bits,
            group_size,
            metadata,
            ..
        } => {
            positive(&[parameters, group_size])?;
            precision(bits)?;
            validate_claim(metadata, interval)
        }
        Formula::Kv {
            context,
            batch,
            layers,
            kv_heads,
            head_width,
            bits,
        } => {
            positive(&[context, batch, layers, kv_heads, head_width])?;
            precision(bits)
        }
        Formula::Recurrent {
            batch,
            state_elements,
            bits,
            coefficient,
        } => {
            positive(&[batch, state_elements])?;
            precision(bits)?;
            ratio(coefficient)
        }
        Formula::RagVectors {
            count,
            dimensions,
            bits,
        } => {
            positive(&[count, dimensions])?;
            precision(bits)
        }
        Formula::Lora {
            targets,
            bits,
            copies,
        } => {
            if targets.len() > 32 {
                return Err(EstimateError::LimitExceeded);
            }
            if targets.is_empty() {
                return Err(EstimateError::InvalidInput);
            }
            precision(bits)?;
            ratio(copies)?;
            for t in targets {
                positive(&[t.input, t.output, t.rank])?;
                if t.rank > t.input.min(t.output) {
                    return Err(EstimateError::InvalidInput);
                }
            }
            Ok(())
        }
        Formula::Unsupported(_) | Formula::NotApplicable => Ok(()),
    }
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(EstimateError::Overflow)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(EstimateError::Overflow)
}
fn ceil(a: u64, b: u64) -> Result<u64> {
    add(a / b, u64::from(!a.is_multiple_of(b)))
}
fn packed(n: u64, bits: u32) -> Result<u64> {
    ceil(mul(n, u64::from(bits))?, 8)
}
fn exact(n: u64) -> ByteInterval {
    ByteInterval { lower: n, upper: n }
}
fn sum(a: ByteInterval, b: ByteInterval) -> Result<ByteInterval> {
    Ok(ByteInterval {
        lower: add(a.lower, b.lower)?,
        upper: add(a.upper, b.upper)?,
    })
}
fn scaled(b: ByteInterval, r: Ratio) -> Result<ByteInterval> {
    Ok(ByteInterval {
        lower: ceil(mul(b.lower, r.numerator)?, r.denominator)?,
        upper: ceil(mul(b.upper, r.numerator)?, r.denominator)?,
    })
}
fn metadata(c: Claim<'_, ByteInterval>, now: u64) -> SizeStatus<'_> {
    match c {
        Claim::Known { value, evidence } if fresh(evidence, now) => SizeStatus::Known(value),
        Claim::Known { evidence, .. } => SizeStatus::Unknown {
            reason: UnknownReason::StaleEvidence,
            source: evidence.source,
        },
        Claim::Unknown { reason, source } => SizeStatus::Unknown { reason, source },
    }
}
/// Calculate one explicit component. Invalid declarations fail; incomplete or
/// stale declarations remain named unknowns. Inputs and outputs allocate nothing.
pub fn component_interval<'a>(claim: Claim<'a, Formula<'a>>, now: u64) -> Result<SizeStatus<'a>> {
    validate_claim(claim, validate_formula)?;
    let (f, e) = match claim {
        Claim::Known { value, evidence } => (value, evidence),
        Claim::Unknown { reason, source } => return Ok(SizeStatus::Unknown { reason, source }),
    };
    if !fresh(e, now) {
        return Ok(SizeStatus::Unknown {
            reason: UnknownReason::StaleEvidence,
            source: e.source,
        });
    }
    let b = match f {
        Formula::NotApplicable => return Ok(SizeStatus::NotApplicable),
        Formula::Unsupported(reason) => {
            return Ok(SizeStatus::Unknown {
                reason,
                source: e.source,
            });
        }
        Formula::Bytes(b) => b,
        Formula::Packed { elements, bits } => exact(packed(elements, bits)?),
        Formula::Scaled { bytes, ratio } => scaled(bytes, ratio)?,
        Formula::Weights {
            resident_parameters,
            bits,
            metadata: m,
            ..
        } => match metadata(m, now) {
            SizeStatus::Known(overhead) => {
                sum(exact(packed(resident_parameters, bits)?), overhead)?
            }
            u => return Ok(u),
        },
        Formula::Quantized {
            parameters,
            bits,
            group_size,
            scale_bytes,
            metadata: m,
        } => match metadata(m, now) {
            SizeStatus::Known(overhead) => sum(
                exact(add(
                    packed(parameters, bits)?,
                    mul(ceil(parameters, group_size)?, scale_bytes)?,
                )?),
                overhead,
            )?,
            u => return Ok(u),
        },
        Formula::Kv {
            context,
            batch,
            layers,
            kv_heads,
            head_width,
            bits,
        } => {
            let n = [context, batch, layers, kv_heads, head_width, 2]
                .into_iter()
                .try_fold(1, mul)?;
            exact(packed(n, bits)?)
        }
        Formula::Recurrent {
            batch,
            state_elements,
            bits,
            coefficient,
        } => scaled(
            exact(packed(mul(batch, state_elements)?, bits)?),
            coefficient,
        )?,
        Formula::RagVectors {
            count,
            dimensions,
            bits,
        } => exact(packed(mul(count, dimensions)?, bits)?),
        Formula::Lora {
            targets,
            bits,
            copies,
        } => {
            let n = targets
                .iter()
                .try_fold(0, |acc, t| add(acc, mul(t.rank, add(t.input, t.output)?)?))?;
            scaled(exact(packed(n, bits)?), copies)?
        }
    };
    Ok(SizeStatus::Known(b))
}
