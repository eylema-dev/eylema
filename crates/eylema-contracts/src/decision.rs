//! Submitted decision assertions. No score, model pin or reference grants authority.
use crate::{
    platform::{ModelProfile, SemanticProfileId},
    primitives::require,
    *,
};
use std::collections::BTreeSet;
pub const MAX_DECISION_ALTERNATIVES: usize = 8;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreMeaning {
    UtilityUnits,
    DeclaredProbabilityBasisPoints,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Score {
    meaning: ScoreMeaning,
    value: i64,
}
impl Score {
    pub fn new(meaning: ScoreMeaning, value: i64) -> Result<Self, ValidationError> {
        require(
            match meaning {
                ScoreMeaning::UtilityUnits => (-1_000_000_000..=1_000_000_000).contains(&value),
                ScoreMeaning::DeclaredProbabilityBasisPoints => (0..=10000).contains(&value),
            },
            "score",
            "score outside declared units",
        )?;
        Ok(Self { meaning, value })
    }
    pub fn meaning(self) -> ScoreMeaning {
        self.meaning
    }
    pub fn value(self) -> i64 {
        self.value
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintStatus {
    Unknown,
    DeclaredSatisfied,
    DeclaredRejected,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Calibration {
    Uncalibrated,
    DeclaredReference(SemanticProfileId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub route: SemanticProfileId,
    pub action: ActionId,
    pub profile: SemanticProfileId,
    pub revision: Revision,
    pub score: Score,
    pub constraints: ConstraintStatus,
}
#[derive(Clone, Eq, PartialEq)]
pub struct DecisionDeclaration {
    alternatives: Vec<Candidate>,
    selected: usize,
    calibration: Calibration,
    model: Option<ModelProfile>,
}
impl std::fmt::Debug for DecisionDeclaration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionDeclaration")
            .field("alternatives", &self.alternatives.len())
            .finish()
    }
}
impl DecisionDeclaration {
    pub fn new(
        alternatives: Vec<Candidate>,
        selected: usize,
        calibration: Calibration,
        model: Option<ModelProfile>,
    ) -> Result<Self, ValidationError> {
        require(
            !alternatives.is_empty()
                && alternatives.len() <= MAX_DECISION_ALTERNATIVES
                && selected < alternatives.len(),
            "alternatives",
            "bounded selected alternative required",
        )?;
        let mut routes = BTreeSet::new();
        let chosen = &alternatives[selected];
        for a in &alternatives {
            require(
                routes.insert(a.route.clone()),
                "alternatives",
                "duplicate route",
            )?;
            require(
                a.action == chosen.action && a.score.meaning() == chosen.score.meaning(),
                "alternatives",
                "same action and score units required",
            )?;
        }
        require(
            chosen.constraints != ConstraintStatus::DeclaredRejected,
            "selected",
            "selected constraint declaration rejects route",
        )?;
        Ok(Self {
            alternatives,
            selected,
            calibration,
            model,
        })
    }
    pub fn alternatives(&self) -> &[Candidate] {
        &self.alternatives
    }
    pub fn selected(&self) -> &Candidate {
        &self.alternatives[self.selected]
    }
    pub fn calibration(&self) -> &Calibration {
        &self.calibration
    }
    pub fn declared_model(&self) -> Option<&ModelProfile> {
        self.model.as_ref()
    }
    pub const fn calibration_verified(&self) -> bool {
        false
    }
    pub const fn model_executed(&self) -> bool {
        false
    }
    pub const fn causal_explanation(&self) -> bool {
        false
    }
}
