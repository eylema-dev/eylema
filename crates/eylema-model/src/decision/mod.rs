//! Closed local relevance proposals; no effect or data authority.
pub(crate) mod profile;
mod request;
mod score;
pub use request::{DecisionBatch, DecisionOption, DecisionQuestion};
pub use score::{DecisionReason, DecisionReceipt, DecisionResult, OptionScore, QuestionScores};
#[cfg(test)]
mod tests;
use crate::cpu::profile::Dimensions;
use crate::{
    LoadedWeights,
    cpu::{CpuControl, CpuError as E, budget::Meter, weights::BoundWeights},
    tokenizer::DecisionTokenizer,
};
use eylema_contracts::model_contract::ModelManifest;
pub use profile::decision_layout;
#[derive(Clone, Copy, Debug)]
pub struct DecisionBudget {
    pub context: usize,
    pub max_resident_bytes: u64,
    pub max_work: u64,
}
impl DecisionBudget {
    pub fn validate(&self) -> Result<(), E> {
        if !(1..=4096).contains(&self.context) {
            return Err(E::ContextLimit);
        }
        if self.max_resident_bytes == 0 || self.max_resident_bytes > 2684354560 {
            return Err(E::ResidentLimit);
        }
        if self.max_work == 0 || self.max_work > 200000000000000 {
            return Err(E::WorkLimit);
        }
        Ok(())
    }
}
pub struct DecisionModel<'a> {
    pub(crate) weights: BoundWeights<'a>,
    pub(crate) manifest: &'a ModelManifest,
    pub(crate) tokenizer: &'a DecisionTokenizer,
    pub(crate) base: u64,
    pub(crate) admission_work: u64,
    pub(crate) control: CpuControl,
    pub(crate) dimensions: Dimensions,
}
impl<'a> DecisionModel<'a> {
    /// Bounded numerical diagnostics, never a role proposal or effect authority.
    pub fn diagnose(
        &self,
        tokens: &crate::tokenizer::EncodedTokens,
        budget: DecisionBudget,
        control: CpuControl,
    ) -> Result<DecisionDiagnostic, E> {
        if !self.control.same_execution(&control) {
            return Err(E::IdentityMismatch);
        }
        if tokens.manifest_sha256() != profile::SHA {
            return Err(E::IdentityMismatch);
        }
        let diagnostic_bytes = std::mem::size_of::<DecisionDiagnostic>() as u64;
        let diagnostic_base = self
            .base
            .checked_add(diagnostic_bytes)
            .ok_or(E::ResidentLimit)?;
        let mut engine = crate::cpu::session::Engine::new_decision(
            self.dimensions,
            self.weights,
            budget,
            control,
            diagnostic_base,
            [9693, 2152],
        )?;
        engine.prefill_decision(tokens.ids())?;
        let layer_bytes = (engine.ws.layers.len() as u64)
            .checked_mul(std::mem::size_of::<f32>() as u64)
            .ok_or(E::ResidentLimit)?;
        engine.meter.preflight(engine.ws.layers.len() as u64)?;
        engine.meter.hold(layer_bytes)?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(engine.ws.layers.len())
            .map_err(|_| E::AllocationFailed)?;
        let actual_layer_bytes = (layers.capacity() as u64)
            .checked_mul(std::mem::size_of::<f32>() as u64)
            .ok_or(E::ResidentLimit)?;
        if actual_layer_bytes > layer_bytes {
            engine.meter.hold(actual_layer_bytes - layer_bytes)?;
        }
        for chunk in engine.ws.layers.chunks(1024) {
            engine.meter.charge(chunk.len() as u64)?;
            layers.extend_from_slice(chunk);
        }
        engine.meter.control.check()?;
        Ok(DecisionDiagnostic {
            head: [engine.ws.logits[0], engine.ws.logits[1]],
            layers,
            work: engine.meter.control.observed_work(),
            peak: engine.meter.peak,
        })
    }
    pub fn source_commit(&self) -> [u8; 20] {
        self.manifest.fields().source_commit
    }
    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.tokenizer.tokenizer_sha256()
    }
    pub fn admit(
        manifest: &'a ModelManifest,
        weights: &'a LoadedWeights,
        tokenizer: &'a DecisionTokenizer,
        budget: DecisionBudget,
        control: &CpuControl,
    ) -> Result<Self, E> {
        budget.validate()?;
        control.check()?;
        tokenizer.ensure_execution(control)?;
        if tokenizer.manifest_sha256() != profile::SHA {
            return Err(E::IdentityMismatch);
        }
        let layout = decision_layout(manifest)?;
        if weights.manifest_sha256() != profile::SHA
            || weights.layout_sha256() != layout.layout_sha256()
        {
            return Err(E::IdentityMismatch);
        }
        for (name, sha) in [
            ("tokenizer.json", tokenizer.tokenizer_sha256()),
            ("tokenizer_config.json", tokenizer.config_sha256()),
        ] {
            if manifest
                .fields()
                .components
                .iter()
                .find(|x| x.name == name)
                .is_none_or(|x| x.sha256 != sha)
            {
                return Err(E::IdentityMismatch);
            }
        }
        let base = weights
            .resident_content_bytes()
            .checked_add(134217728 + 4194304 + 2097152)
            .ok_or(E::ResidentLimit)?;
        let mut meter = Meter::decision(budget, control.clone(), base)?;
        crate::cpu::weights::scan_stored(weights, &layout, &mut meter)?;
        let bound = BoundWeights::loaded(profile::dimensions(), weights)?;
        Ok(Self {
            weights: bound,
            manifest,
            tokenizer,
            base,
            admission_work: control.observed_work(),
            control: control.clone(),
            dimensions: profile::dimensions(),
        })
    }
    pub fn admission_work(&self) -> u64 {
        self.admission_work
    }
}
pub struct DecisionDiagnostic {
    head: [f32; 2],
    layers: Vec<f32>,
    work: u64,
    peak: u64,
}
impl DecisionDiagnostic {
    pub fn head(&self) -> [f32; 2] {
        self.head
    }
    pub fn layers(&self) -> &[f32] {
        &self.layers
    }
    pub fn work(&self) -> u64 {
        self.work
    }
    pub fn peak_logical_bytes(&self) -> u64 {
        self.peak
    }
}

fn token_error(e: crate::tokenizer::TokenizeError) -> E {
    use crate::tokenizer::TokenizeError as T;
    match e {
        T::Cancelled => E::Cancelled,
        T::Deadline => E::Deadline,
        T::WorkLimit => E::WorkLimit,
        T::ResidentLimit => E::ResidentLimit,
        T::TokenLimit => E::ContextLimit,
        _ => E::InvalidInput,
    }
}
impl DecisionModel<'_> {
    pub fn score(
        &self,
        batch: &DecisionBatch<'_>,
        budget: DecisionBudget,
        control: CpuControl,
    ) -> Result<DecisionResult, E> {
        if !self.control.same_execution(&control) {
            return Err(E::IdentityMismatch);
        }
        budget.validate()?;
        control.check()?;
        control.restrict_work(budget.max_work)?;
        let output_reservation = request::preflight(
            batch,
            self.base,
            self.dimensions,
            budget.context,
            budget.max_resident_bytes,
            budget.max_work,
            control.observed_work(),
        )?;
        control.charge(output_reservation)?;
        let request = request::identity(batch, &control)?;
        let mut questions = Vec::new();
        questions
            .try_reserve_exact(batch.questions.len())
            .map_err(|_| E::AllocationFailed)?;
        for q in batch.questions {
            let mut scores = Vec::new();
            scores
                .try_reserve_exact(q.options.len())
                .map_err(|_| E::AllocationFailed)?;
            for o in q.options {
                scores.push(OptionScore {
                    id: request::copy(o.id)?,
                    logits: [0.0; 2],
                    delta: 0.0,
                    probability: 0.0,
                    micros: 0,
                    cumulative: 0,
                });
            }
            let mut choice = String::new();
            let largest = q.options.iter().map(|o| o.id.len()).max().unwrap_or(0);
            choice
                .try_reserve_exact(largest)
                .map_err(|_| E::AllocationFailed)?;
            questions.push(QuestionScores {
                id: request::copy(q.id)?,
                scores,
                choice: Some(choice),
                reason: DecisionReason::MissingScore,
            });
        }
        let output_bytes = request::actual_output_bytes(&questions, questions.capacity())?;
        let live_base = self
            .base
            .checked_add(output_bytes)
            .ok_or(E::ResidentLimit)?;
        if live_base > budget.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        if output_bytes > output_reservation {
            control.charge(output_bytes - output_reservation)?;
        }
        let mut peak = live_base;
        for (question_index, q) in batch.questions.iter().enumerate() {
            for (option_index, o) in q.options.iter().enumerate() {
                let tokens = self
                    .tokenizer
                    .encode_pair(
                        q.instruction,
                        q.query,
                        o.candidate,
                        crate::tokenizer::TextBudget {
                            max_resident_bytes: 134217728,
                            max_tokens: budget.context,
                            max_work: 2000000,
                        },
                        &control,
                    )
                    .map_err(token_error)?;
                let mut engine = crate::cpu::session::Engine::new_decision(
                    self.dimensions,
                    self.weights,
                    budget,
                    control.clone(),
                    live_base,
                    [9693, 2152],
                )?;
                engine.prefill_decision(tokens.ids())?;
                let logits = [engine.ws.logits[0], engine.ws.logits[1]];
                control.charge(32)?;
                let (delta, probability, micros) = score::head(logits)?;
                let score = &mut questions[question_index].scores[option_index];
                score.logits = logits;
                score.delta = delta;
                score.probability = probability;
                score.micros = micros;
                score.cumulative = control.observed_work();
                peak = peak.max(engine.meter.peak);
            }
            let mut values = [None; 4];
            for (i, s) in questions[question_index].scores.iter().enumerate() {
                values[i] = Some(s.micros);
            }
            control.charge(32)?;
            let reason = score::choose(&values[..questions[question_index].scores.len()]);
            if reason == DecisionReason::Chosen {
                let result = &mut questions[question_index];
                let best_id = result
                    .scores
                    .iter()
                    .max_by_key(|s| s.micros)
                    .ok_or(E::InvalidOutput)?
                    .id
                    .as_str();
                let choice = result.choice.as_mut().ok_or(E::InvalidOutput)?;
                choice.push_str(best_id);
            } else {
                questions[question_index].choice = None;
            }
            questions[question_index].reason = reason;
        }
        control.check()?;
        let artifact = self
            .manifest
            .fields()
            .components
            .iter()
            .find(|x| x.name == "model.safetensors")
            .ok_or(E::IdentityMismatch)?
            .sha256;
        Ok(DecisionResult {
            questions,
            receipt: DecisionReceipt {
                identities: [
                    profile::SHA,
                    artifact,
                    self.tokenizer.tokenizer_sha256(),
                    self.tokenizer.config_sha256(),
                    crate::cpu::profile::hex(
                        b"c91af212ad3548b6a9f87c8b54901bb171e2f596e7d61f666634e44fc690a637",
                    ),
                    request,
                ],
                source: self.source_commit(),
                work: control.observed_work(),
                peak,
            },
        })
    }
}
