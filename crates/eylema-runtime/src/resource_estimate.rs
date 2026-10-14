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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolKind {
    Unified,
    HostRam,
    DeviceVram,
    Storage,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pool<'a> {
    pub id: &'a str,
    pub physical_id: &'a str,
    pub kind: PoolKind,
    pub capacity: Claim<'a, u64>,
    pub available: Claim<'a, u64>,
    pub reserve: Claim<'a, u64>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceKey<'a> {
    pub profile: &'a str,
    pub revision: u64,
    pub artifact: [u8; 32],
    pub backend: &'a str,
    pub owner: &'a str,
    pub pool: &'a str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resident<'a> {
    pub id: &'a str,
    pub key: InstanceKey<'a>,
    pub expected_key: InstanceKey<'a>,
    pub weights: Claim<'a, Formula<'a>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Invocation<'a> {
    pub id: &'a str,
    pub resident: &'a str,
    pub pool: &'a str,
    pub state: Claim<'a, Formula<'a>>,
    pub workspace: Claim<'a, Formula<'a>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuxiliaryKind {
    ModelState,
    ConversionSource,
    ConversionDestination,
    Staging,
    Modality,
    RagIndex,
    RagSource,
    RagWorking,
    TrainingBase,
    Adapter,
    Gradient,
    Optimizer,
    Activation,
    Checkpoint,
    Temporary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifetime {
    Persistent,
    Transient,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Auxiliary<'a> {
    pub id: &'a str,
    pub pool: &'a str,
    pub kind: AuxiliaryKind,
    pub lifetime: Lifetime,
    pub size: Claim<'a, Formula<'a>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Member<'a> {
    Invocation(&'a str),
    Auxiliary(&'a str),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Overlap<'a> {
    pub id: &'a str,
    pub members: &'a [Member<'a>],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot<'a> {
    pub id: &'a str,
    pub expected_id: &'a str,
    pub revision: u64,
    pub expected_revision: u64,
    pub pools: &'a [Pool<'a>],
    pub residents: &'a [Resident<'a>],
    pub invocations: &'a [Invocation<'a>],
    pub auxiliary: &'a [Auxiliary<'a>],
    pub overlaps: Option<&'a [Overlap<'a>]>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentPart {
    Weights,
    State,
    Workspace,
    Auxiliary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentEstimate<'a> {
    pub id: &'a str,
    pub part: ComponentPart,
    pub pool: &'a str,
    pub status: SizeStatus<'a>,
    pub evidence: Option<Evidence<'a>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    EstimatedFit,
    ExceedsDeclaredBudget,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEstimate<'a> {
    pub id: &'a str,
    pub total: Option<ByteInterval>,
    pub usable_budget: Option<u64>,
    pub verdict: Verdict,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EstimateReport<'a> {
    pub snapshot: &'a Snapshot<'a>,
    pub snapshot_status: SizeStatus<'a>,
    pub components: [Option<ComponentEstimate<'a>>; 144],
    pub component_count: usize,
    pub pools: [Option<PoolEstimate<'a>>; 8],
    pub pool_count: usize,
    pub verdict: Verdict,
    pub planning_only: bool,
}
fn validate_key(k: InstanceKey<'_>) -> Result<()> {
    for id in [k.profile, k.backend, k.owner, k.pool] {
        identity(id)?;
    }
    positive(&[k.revision])
}
fn declared_value(c: Claim<'_, u64>) -> Option<u64> {
    match c {
        Claim::Known { value, .. } => Some(value),
        _ => None,
    }
}
fn fresh_value(c: Claim<'_, u64>, now: u64) -> Option<u64> {
    match c {
        Claim::Known { value, evidence } if fresh(evidence, now) => Some(value),
        _ => None,
    }
}
fn pool_index(s: &Snapshot<'_>, id: &str) -> Result<usize> {
    s.pools
        .iter()
        .position(|p| p.id == id)
        .ok_or(EstimateError::DanglingReference)
}
fn resident_index(s: &Snapshot<'_>, id: &str) -> Result<usize> {
    s.residents
        .iter()
        .position(|r| r.id == id)
        .ok_or(EstimateError::DanglingReference)
}
fn invocation_index(s: &Snapshot<'_>, id: &str) -> Result<usize> {
    s.invocations
        .iter()
        .position(|i| i.id == id)
        .ok_or(EstimateError::DanglingReference)
}
fn auxiliary_index(s: &Snapshot<'_>, id: &str) -> Result<usize> {
    s.auxiliary
        .iter()
        .position(|a| a.id == id)
        .ok_or(EstimateError::DanglingReference)
}
fn validate_snapshot(s: &Snapshot<'_>) -> Result<()> {
    // Bound every slice before walking untrusted cross references or formulas.
    if s.pools.len() > 8
        || s.residents.len() > 16
        || s.invocations.len() > 32
        || s.auxiliary.len() > 64
        || s.overlaps
            .is_some_and(|g| g.len() > 32 || g.iter().any(|g| g.members.len() > 32))
    {
        return Err(EstimateError::LimitExceeded);
    }
    if s.pools.is_empty() {
        return Err(EstimateError::InvalidInput);
    }
    identity(s.id)?;
    identity(s.expected_id)?;
    positive(&[s.revision, s.expected_revision])?;
    for (n, p) in s.pools.iter().enumerate() {
        identity(p.id)?;
        identity(p.physical_id)?;
        if s.pools[..n].iter().any(|prior| prior.id == p.id) {
            return Err(EstimateError::DuplicateIdentity);
        }
        if s.pools[..n]
            .iter()
            .any(|prior| prior.physical_id == p.physical_id)
        {
            return Err(EstimateError::ConflictingIdentity);
        }
        for c in [p.capacity, p.available, p.reserve] {
            validate_claim(c, |_| Ok(()))?;
        }
        if let (Some(cap), Some(avail)) = (declared_value(p.capacity), declared_value(p.available))
            && avail > cap
        {
            return Err(EstimateError::ConflictingIdentity);
        }
        if let (Some(avail), Some(reserve)) =
            (declared_value(p.available), declared_value(p.reserve))
            && reserve > avail
        {
            return Err(EstimateError::ConflictingIdentity);
        }
    }
    // All entry IDs share one namespace; arrays are fixed and entirely bounded.
    let mut ids: [Option<&str>; 112] = [None; 112];
    for (count, id) in s
        .residents
        .iter()
        .map(|r| r.id)
        .chain(s.invocations.iter().map(|i| i.id))
        .chain(s.auxiliary.iter().map(|a| a.id))
        .enumerate()
    {
        identity(id)?;
        if ids[..count].contains(&Some(id)) {
            return Err(EstimateError::DuplicateIdentity);
        }
        ids[count] = Some(id);
    }
    for (n, r) in s.residents.iter().enumerate() {
        validate_key(r.key)?;
        validate_key(r.expected_key)?;
        pool_index(s, r.key.pool)?;
        if s.residents[..n].iter().any(|prior| prior.key == r.key) {
            return Err(EstimateError::DuplicateIdentity);
        }
        validate_claim(r.weights, validate_formula)?;
        if matches!(
            r.weights,
            Claim::Known {
                value: Formula::NotApplicable,
                ..
            }
        ) {
            return Err(EstimateError::InvalidInput);
        }
    }
    for i in s.invocations {
        identity(i.resident)?;
        identity(i.pool)?;
        resident_index(s, i.resident)?;
        pool_index(s, i.pool)?;
        validate_claim(i.state, validate_formula)?;
        validate_claim(i.workspace, validate_formula)?;
    }
    for a in s.auxiliary {
        identity(a.pool)?;
        pool_index(s, a.pool)?;
        validate_claim(a.size, validate_formula)?;
    }
    if let Some(groups) = s.overlaps {
        if groups.is_empty()
            && (!s.invocations.is_empty()
                || s.auxiliary
                    .iter()
                    .any(|a| a.lifetime == Lifetime::Transient))
        {
            return Err(EstimateError::InvalidInput);
        }
        for (n, g) in groups.iter().enumerate() {
            identity(g.id)?;
            if groups[..n].iter().any(|prior| prior.id == g.id) {
                return Err(EstimateError::DuplicateIdentity);
            }
            for (m, member) in g.members.iter().enumerate() {
                if g.members[..m].contains(member) {
                    return Err(EstimateError::DuplicateIdentity);
                }
                match member {
                    Member::Invocation(id) => {
                        identity(id)?;
                        invocation_index(s, id)?;
                    }
                    Member::Auxiliary(id) => {
                        identity(id)?;
                        let a = &s.auxiliary[auxiliary_index(s, id)?];
                        if a.lifetime == Lifetime::Persistent {
                            return Err(EstimateError::InvalidInput);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn source<'a>(c: Claim<'a, Formula<'a>>) -> Option<Evidence<'a>> {
    match c {
        Claim::Known { evidence, .. } => Some(evidence),
        _ => None,
    }
}
fn record<'a>(report: &mut EstimateReport<'a>, item: ComponentEstimate<'a>) {
    report.components[report.component_count] = Some(item);
    report.component_count += 1;
}
fn unknown(reason: UnknownReason, source: &str) -> SizeStatus<'_> {
    SizeStatus::Unknown { reason, source }
}
fn resident_status<'a>(r: Resident<'a>, now: u64) -> Result<SizeStatus<'a>> {
    // Calculate first: an identity mismatch cannot hide arithmetic errors.
    let status = component_interval(r.weights, now)?;
    if r.key.artifact == [0; 32] {
        Ok(unknown(UnknownReason::MissingIdentity, r.id))
    } else if r.key != r.expected_key {
        Ok(unknown(UnknownReason::IdentityMismatch, r.id))
    } else {
        Ok(status)
    }
}
#[derive(Clone, Copy)]
struct Subtotal {
    bytes: ByteInterval,
    complete: bool,
}
impl Subtotal {
    const ZERO: Self = Self {
        bytes: ByteInterval { lower: 0, upper: 0 },
        complete: true,
    };
    fn include(&mut self, status: SizeStatus<'_>) -> Result<()> {
        match status {
            SizeStatus::Known(b) => self.bytes = sum(self.bytes, b)?,
            SizeStatus::Unknown { .. } => self.complete = false,
            SizeStatus::NotApplicable => {}
        }
        Ok(())
    }
}
/// Estimate only the declared workload. The caller must supply every required
/// placement, overlap and coefficient. No result authorizes a model or workload.
pub fn estimate<'a>(s: &'a Snapshot<'a>, now: u64) -> Result<EstimateReport<'a>> {
    validate_snapshot(s)?;
    let mismatched = s.id != s.expected_id || s.revision != s.expected_revision;
    let mut report = EstimateReport {
        snapshot: s,
        snapshot_status: SizeStatus::Known(exact(0)),
        components: [None; 144],
        component_count: 0,
        pools: [None; 8],
        pool_count: s.pools.len(),
        verdict: Verdict::EstimatedFit,
        planning_only: true,
    };
    if mismatched {
        report.snapshot_status = unknown(UnknownReason::IdentityMismatch, s.id);
    }
    for r in s.residents {
        let status = resident_status(*r, now)?;
        record(
            &mut report,
            ComponentEstimate {
                id: r.id,
                part: ComponentPart::Weights,
                pool: r.key.pool,
                status: if mismatched { report_status(s) } else { status },
                evidence: source(r.weights),
            },
        );
    }
    for i in s.invocations {
        let resident = resident_status(s.residents[resident_index(s, i.resident)?], now)?;
        for (part, claim) in [
            (ComponentPart::State, i.state),
            (ComponentPart::Workspace, i.workspace),
        ] {
            let own = component_interval(claim, now)?;
            let status = if mismatched {
                report_status(s)
            } else if matches!(resident, SizeStatus::Unknown { .. }) {
                resident
            } else {
                own
            };
            record(
                &mut report,
                ComponentEstimate {
                    id: i.id,
                    part,
                    pool: i.pool,
                    status,
                    evidence: source(claim),
                },
            );
        }
    }
    for a in s.auxiliary {
        let status = component_interval(a.size, now)?;
        record(
            &mut report,
            ComponentEstimate {
                id: a.id,
                part: ComponentPart::Auxiliary,
                pool: a.pool,
                status: if mismatched { report_status(s) } else { status },
                evidence: source(a.size),
            },
        );
    }
    let mut persistent = [Subtotal::ZERO; 8];
    let mut peak = [Subtotal::ZERO; 8];
    let mut seen_inv = [false; 32];
    let mut seen_aux = [false; 64];
    for (n, r) in s.residents.iter().enumerate() {
        persistent[pool_index(s, r.key.pool)?].include(report.components[n].unwrap().status)?;
    }
    for (n, a) in s.auxiliary.iter().enumerate() {
        if a.lifetime == Lifetime::Persistent {
            persistent[pool_index(s, a.pool)?].include(
                report.components[s.residents.len() + s.invocations.len() * 2 + n]
                    .unwrap()
                    .status,
            )?;
        }
    }
    match s.overlaps {
        None => {
            for p in &mut peak[..s.pools.len()] {
                p.complete = false;
            }
            if !mismatched {
                report.snapshot_status = unknown(UnknownReason::MissingOverlap, s.id);
            }
        }
        Some(groups) => {
            for g in groups {
                let mut group = [Subtotal::ZERO; 8];
                for member in g.members {
                    match member {
                        Member::Invocation(id) => {
                            let n = invocation_index(s, id)?;
                            seen_inv[n] = true;
                            let pi = pool_index(s, s.invocations[n].pool)?;
                            for offset in 0..2 {
                                group[pi].include(
                                    report.components[s.residents.len() + n * 2 + offset]
                                        .unwrap()
                                        .status,
                                )?;
                            }
                        }
                        Member::Auxiliary(id) => {
                            let n = auxiliary_index(s, id)?;
                            seen_aux[n] = true;
                            let pi = pool_index(s, s.auxiliary[n].pool)?;
                            group[pi].include(
                                report.components[s.residents.len() + s.invocations.len() * 2 + n]
                                    .unwrap()
                                    .status,
                            )?;
                        }
                    }
                }
                for n in 0..s.pools.len() {
                    peak[n].bytes.lower = peak[n].bytes.lower.max(group[n].bytes.lower);
                    peak[n].bytes.upper = peak[n].bytes.upper.max(group[n].bytes.upper);
                    peak[n].complete &= group[n].complete;
                }
            }
            for (n, i) in s.invocations.iter().enumerate() {
                if !seen_inv[n] {
                    peak[pool_index(s, i.pool)?].complete = false;
                    if !mismatched {
                        report.snapshot_status = unknown(UnknownReason::MissingOverlap, s.id);
                    }
                }
            }
            for (n, a) in s.auxiliary.iter().enumerate() {
                if a.lifetime == Lifetime::Transient && !seen_aux[n] {
                    peak[pool_index(s, a.pool)?].complete = false;
                    if !mismatched {
                        report.snapshot_status = unknown(UnknownReason::MissingOverlap, s.id);
                    }
                }
            }
        }
    }
    for (n, p) in s.pools.iter().enumerate() {
        let b = sum(persistent[n].bytes, peak[n].bytes)?;
        let total = if mismatched || !persistent[n].complete || !peak[n].complete {
            None
        } else {
            Some(b)
        };
        let budget = match (
            fresh_value(p.capacity, now),
            fresh_value(p.available, now),
            fresh_value(p.reserve, now),
        ) {
            (Some(_), Some(avail), Some(reserve)) => Some(avail - reserve),
            _ => None,
        };
        let verdict = match (total, budget) {
            (Some(b), Some(v)) if b.upper <= v => Verdict::EstimatedFit,
            (Some(b), Some(v)) if b.lower > v => Verdict::ExceedsDeclaredBudget,
            _ => Verdict::Unknown,
        };
        report.pools[n] = Some(PoolEstimate {
            id: p.id,
            total,
            usable_budget: budget,
            verdict,
        });
        match verdict {
            Verdict::ExceedsDeclaredBudget => report.verdict = verdict,
            Verdict::Unknown if report.verdict != Verdict::ExceedsDeclaredBudget => {
                report.verdict = verdict
            }
            _ => {}
        }
    }
    Ok(report)
}
fn report_status<'a>(s: &Snapshot<'a>) -> SizeStatus<'a> {
    unknown(UnknownReason::IdentityMismatch, s.id)
}
