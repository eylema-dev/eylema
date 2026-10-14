//! Native execution of the coordinator with explicitly synthetic host evidence.
//! No provider, reviewer, subprocess, filesystem mutation or GitHub call occurs.
use eylema_contracts::{RequestId, Revision};
use eylema_development::*;
use std::time::Instant;

struct FixtureHost(AcceptedTask);
impl AuthorityResolver for FixtureHost {
    fn resolve(&self, _: &NeedSpec) -> Option<AcceptedTask> {
        Some(self.0.clone())
    }
}
struct DeniedHost;
impl AuthorityResolver for DeniedHost {
    fn resolve(&self, _: &NeedSpec) -> Option<AcceptedTask> {
        None
    }
}

fn main() -> Result<()> {
    let start = Instant::now();
    let now = || 1 + u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX - 1);
    let base = Commit::new("a".repeat(40))?;
    let head = Commit::new("b".repeat(40))?;
    let need = NeedSpec::new(NeedDraft {
        key: TaskKey::new(
            "eyleme-dev/eyleme".into(),
            RequestId::new("native-coordinator-fixture").map_err(|_| Error::InvalidInput)?,
            Revision::new(1).map_err(|_| Error::InvalidInput)?,
        )?,
        source_repository: "eyleme-dev/eyleme".into(),
        issue_number: None,
        objective: "Exercise owned coordinator CPU transitions with fixture observations".into(),
        acceptance: vec![Acceptance {
            id: "A1".into(),
            scenario: "bounded native flow".into(),
            expected: "required stages and exact binding".into(),
        }],
        allowed_paths: vec!["src/".into()],
        out_of_scope: vec!["external effects".into()],
        targets: vec!["native_macos".into()],
        authorization_ref: "fixture-reference".into(),
        limits: Limits {
            model_calls: 1,
            repairs: 0,
            wall_ms: 60_000,
            usage_units: 1,
        },
    })?;
    let host = FixtureHost(AcceptedTask::new(
        need.clone(),
        base.clone(),
        "fixture-principal".into(),
        "fixture-policy".into(),
        60_001,
    )?);
    let mut registry = Registry::new(1)?;
    let run = registry.receive(need, now())?;
    if run.spec_ready(now(), &DeniedHost) != Err(Error::Forbidden) {
        return Err(Error::FailedEvidence);
    }
    run.spec_ready(now(), &host)?;
    run.set_plan(
        WorkPlan {
            base,
            implementer_context: "fixture-author".into(),
            checks: vec![CheckMapping {
                acceptance_id: "A1".into(),
                check: "fixture-behavior".into(),
            }],
        },
        now(),
        &host,
    )?;
    run.begin(now(), &host)?;
    run.submit_change(head.clone(), vec!["src/fixture.rs".into()], now(), &host)?;
    run.local_verified(
        CheckEvidence {
            head: head.clone(),
            passed_checks: vec!["fixture-behavior".into()],
        },
        now(),
        &host,
    )?;
    run.reviewed(
        ReviewEvidence {
            head,
            context: "fixture-review-context".into(),
            blocking_findings: 0,
            passed: true,
        },
        now(),
        &host,
    )?;
    if run.state() != State::Running(Stage::DraftPr) {
        return Err(Error::FailedEvidence);
    }
    println!(
        "{{\"kind\":\"native_coordinator_contract_proof\",\"status\":\"passed\",\"state\":\"draft_pr\",\"host_evidence\":\"synthetic_fixture\",\"actual_provider_review_pr\":\"not_run\",\"authority_denial\":\"passed\",\"model_calls\":{},\"elapsed_ms\":{}}}",
        run.usage().model_calls,
        start.elapsed().as_millis()
    );
    Ok(())
}
