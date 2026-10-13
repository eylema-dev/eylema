use eylema_contracts::{decision::*, platform::SemanticProfileId, *};
fn candidate(id: &str, score: i64) -> Candidate {
    Candidate {
        route: SemanticProfileId::new(id).unwrap(),
        action: ActionId::new("action:0").unwrap(),
        profile: SemanticProfileId::new("tool:add").unwrap(),
        revision: Revision::new(1).unwrap(),
        score: Score::new(ScoreMeaning::DeclaredProbabilityBasisPoints, score).unwrap(),
        constraints: ConstraintStatus::DeclaredSatisfied,
    }
}
#[test]
fn selection_is_bounded_unique_and_cannot_claim_rejected_route() {
    let c = candidate("route:1", 9000);
    let make = |v, i| DecisionDeclaration::new(v, i, Calibration::Uncalibrated, None);
    assert!(make(vec![c.clone()], 0).is_ok());
    assert!(make(vec![], 0).is_err());
    assert!(make(vec![c.clone()], 1).is_err());
    assert!(make(vec![c.clone(), c.clone()], 0).is_err());
    assert!(
        make(
            (0..9)
                .map(|i| candidate(&format!("route:{i}"), 5000))
                .collect(),
            0
        )
        .is_err()
    );
    let mut rejected = c;
    rejected.constraints = ConstraintStatus::DeclaredRejected;
    assert!(make(vec![rejected], 0).is_err());
}
#[test]
fn declared_scores_are_not_probabilities_truth_or_calibration_proofs() {
    assert!(Score::new(ScoreMeaning::DeclaredProbabilityBasisPoints, -1).is_err());
    assert!(Score::new(ScoreMeaning::DeclaredProbabilityBasisPoints, 10001).is_err());
    assert!(Score::new(ScoreMeaning::UtilityUnits, 1_000_000_001).is_err());
    assert!(Score::new(ScoreMeaning::UtilityUnits, -100).is_ok());
    let d = DecisionDeclaration::new(
        vec![candidate("route:a", 10000)],
        0,
        Calibration::DeclaredReference(SemanticProfileId::new("eval:unverified").unwrap()),
        None,
    )
    .unwrap();
    assert!(!d.calibration_verified());
    assert!(!d.model_executed());
    assert!(!d.causal_explanation());
}
