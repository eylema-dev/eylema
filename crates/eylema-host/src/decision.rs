//! Sealed snapshots of actual owned tool calls, not model or GPU benchmarks.
use crate::tool::{ToolError, ToolReport, ToolStatus};
use eylema_contracts::{TaskId, decision::DecisionDeclaration};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub const OWNED_LOOP_IMPLEMENTATION: &str = "owned:typed-tool-driver:v1";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunTiming {
    pub first_host_call: bool,
    pub admission: Duration,
    pub steps: Vec<Duration>,
    /// Includes caller delays between admission and terminal stepping.
    pub total: Option<Duration>,
    pub cancellation: Option<Duration>,
}
pub(crate) struct Measurement {
    pub started: Instant,
    pub ended: Option<Instant>,
    pub cancellation_started: Option<Instant>,
    pub timing: RunTiming,
}
impl Measurement {
    pub fn new(started: Instant, first: bool) -> Self {
        Self {
            started,
            ended: None,
            cancellation_started: None,
            timing: RunTiming {
                first_host_call: first,
                admission: started.elapsed(),
                steps: vec![],
                total: None,
                cancellation: None,
            },
        }
    }
    pub fn step(&mut self, started: Instant, terminal: bool) {
        // An eight-action plan plus one stop step is the fixed upper bound.
        if self.timing.steps.len() < 9 {
            self.timing.steps.push(started.elapsed());
        }
        if terminal {
            let end = Instant::now();
            self.ended = Some(end);
            self.timing.total = Some(end.duration_since(self.started));
            self.timing.cancellation = self.cancellation_started.map(|c| end.duration_since(c));
        }
    }
}
#[derive(Clone)]
pub struct DecisionEvidence {
    pub(crate) task: TaskId,
    pub(crate) declaration: DecisionDeclaration,
    pub(crate) outcome: ToolReport,
    pub(crate) timing: RunTiming,
    pub(crate) identity: Arc<()>,
    pub(crate) started: Instant,
    pub(crate) ended: Option<Instant>,
    // Private actual workload key; no arbitrary labels or exposed payloads.
    pub(crate) workload: Vec<u8>,
    pub(crate) declared_payload_budget: u32,
}
impl std::fmt::Debug for DecisionEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionEvidence")
            .field("status", &self.outcome.status)
            .finish()
    }
}
impl DecisionEvidence {
    pub fn task(&self) -> &TaskId {
        &self.task
    }
    pub fn declaration(&self) -> &DecisionDeclaration {
        &self.declaration
    }
    pub fn outcome(&self) -> &ToolReport {
        &self.outcome
    }
    pub fn timing(&self) -> &RunTiming {
        &self.timing
    }
    pub const fn loop_implementation(&self) -> &'static str {
        OWNED_LOOP_IMPLEMENTATION
    }
    pub fn declared_payload_budget(&self) -> u32 {
        self.declared_payload_budget
    }
    pub const fn accounted_reservation_bytes(&self) -> Option<u64> {
        None
    }
    pub const fn physical_rss_bytes(&self) -> Option<u64> {
        None
    }
    pub const fn physical_vram_bytes(&self) -> Option<u64> {
        None
    }
}
#[derive(Clone, Debug)]
pub struct CycleDistribution {
    pub samples: Vec<Duration>,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub first_host_call: Duration,
    pub successful_cycles_per_second: f64,
    pub wall_span: Duration,
}
impl CycleDistribution {
    /// Comparable completed snapshots from one live facade. Earlier released
    /// snapshots are not reauthorized; this method grants no persistence/egress.
    pub fn from_evidence(rows: &[&DecisionEvidence]) -> Result<Self, ToolError> {
        if !(2..=128).contains(&rows.len()) {
            return Err(ToolError::InvalidEvidence);
        }
        let first = rows[0];
        let mut ids = std::collections::BTreeSet::new();
        let mut cold = None;
        let mut samples = vec![];
        let mut start = first.started;
        let mut end = first.ended.ok_or(ToolError::InvalidEvidence)?;
        for row in rows {
            if !Arc::ptr_eq(&first.identity, &row.identity)
                || row.workload != first.workload
                || row.outcome.status != ToolStatus::Completed
                || !ids.insert(row.outcome.execution_id.clone())
                || row.timing.cancellation.is_some()
            {
                return Err(ToolError::InvalidEvidence);
            }
            let duration = row.timing.total.ok_or(ToolError::InvalidEvidence)?;
            if row.timing.first_host_call {
                if cold.replace(duration).is_some() {
                    return Err(ToolError::InvalidEvidence);
                }
            } else {
                samples.push(duration);
            }
            start = start.min(row.started);
            end = end.max(row.ended.ok_or(ToolError::InvalidEvidence)?);
        }
        if samples.is_empty() {
            return Err(ToolError::InvalidEvidence);
        }
        let span = end.duration_since(start);
        if span.is_zero() {
            return Err(ToolError::InvalidEvidence);
        }
        let mut sorted = samples.clone();
        sorted.sort();
        let rank = |p: usize| sorted[(p * sorted.len()).div_ceil(100) - 1];
        Ok(Self {
            p50: rank(50),
            p95: rank(95),
            p99: rank(99),
            samples,
            first_host_call: cold.ok_or(ToolError::InvalidEvidence)?,
            successful_cycles_per_second: rows.len() as f64 / span.as_secs_f64(),
            wall_span: span,
        })
    }
}
