use crate::cpu::CpuError as E;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    Chosen,
    Tie,
    BelowFloor,
    InsufficientMargin,
    MissingScore,
}
pub(crate) fn micros(p: f64) -> Result<u32, E> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(E::InvalidOutput);
    }
    Ok((p * 1000000.).round_ties_even() as u32)
}
pub(crate) fn head(v: [f32; 2]) -> Result<(f64, f64, u32), E> {
    if v.iter().any(|x| !x.is_finite()) {
        return Err(E::NonFinite);
    }
    let d = v[0] as f64 - v[1] as f64;
    let p = if d >= 0. {
        1. / (1. + (-d).exp())
    } else {
        let e = d.exp();
        e / (1. + e)
    };
    Ok((d, p, micros(p)?))
}
pub(crate) fn choose(scores: &[Option<u32>]) -> DecisionReason {
    if scores.is_empty() || scores.iter().any(Option::is_none) {
        return DecisionReason::MissingScore;
    }
    let mut top = 0;
    let mut second = 0;
    for x in scores.iter().flatten() {
        if *x >= top {
            second = top;
            top = *x
        } else if *x > second {
            second = *x
        }
    }
    if scores.len() > 1 && top == second {
        DecisionReason::Tie
    } else if top < 750000 {
        DecisionReason::BelowFloor
    } else if top - second < 100000 {
        DecisionReason::InsufficientMargin
    } else {
        DecisionReason::Chosen
    }
}
pub struct OptionScore {
    pub(crate) id: String,
    pub(crate) logits: [f32; 2],
    pub(crate) delta: f64,
    pub(crate) probability: f64,
    pub(crate) micros: u32,
    pub(crate) cumulative: u64,
}
impl OptionScore {
    pub fn option_id(&self) -> &str {
        &self.id
    }
    pub fn logits(&self) -> [f32; 2] {
        self.logits
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }
    pub fn probability(&self) -> f64 {
        self.probability
    }
    pub fn score_micros(&self) -> u32 {
        self.micros
    }
    pub fn yes_no(&self) -> bool {
        self.micros >= 500000
    }
    pub fn cumulative_work(&self) -> u64 {
        self.cumulative
    }
}
pub struct QuestionScores {
    pub(crate) id: String,
    pub(crate) scores: Vec<OptionScore>,
    pub(crate) choice: Option<String>,
    pub(crate) reason: DecisionReason,
}
impl QuestionScores {
    pub fn question_id(&self) -> &str {
        &self.id
    }
    pub fn scores(&self) -> &[OptionScore] {
        &self.scores
    }
    pub fn choice(&self) -> Option<&str> {
        self.choice.as_deref()
    }
    pub fn reason(&self) -> DecisionReason {
        self.reason
    }
}
pub struct DecisionReceipt {
    pub(crate) identities: [[u8; 32]; 6],
    pub(crate) source: [u8; 20],
    pub(crate) work: u64,
    pub(crate) peak: u64,
}
impl DecisionReceipt {
    pub fn identities(&self) -> &[[u8; 32]; 6] {
        &self.identities
    }
    pub fn source_commit(&self) -> [u8; 20] {
        self.source
    }
    pub fn work(&self) -> u64 {
        self.work
    }
    pub fn peak_logical_bytes(&self) -> u64 {
        self.peak
    }
    pub fn calibrated_confidence(&self) -> bool {
        false
    }
}
pub struct DecisionResult {
    pub(crate) questions: Vec<QuestionScores>,
    pub(crate) receipt: DecisionReceipt,
}
impl DecisionResult {
    pub fn questions(&self) -> &[QuestionScores] {
        &self.questions
    }
    pub fn receipt(&self) -> &DecisionReceipt {
        &self.receipt
    }
}
