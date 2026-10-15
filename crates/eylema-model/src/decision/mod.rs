//! Closed local relevance proposals; no effect or data authority.
pub(crate) mod profile;
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
}
impl<'a> DecisionModel<'a> {
    /// Bounded numerical diagnostics, never a role proposal or effect authority.
    pub fn diagnose(
        &self,
        tokens: &crate::tokenizer::EncodedTokens,
        budget: DecisionBudget,
        control: CpuControl,
    ) -> Result<DecisionDiagnostic, E> {
        if tokens.manifest_sha256() != profile::SHA {
            return Err(E::IdentityMismatch);
        }
        let mut engine = crate::cpu::session::Engine::new_decision(
            profile::dimensions(),
            self.weights,
            budget,
            control,
            self.base,
            [9693, 2152],
        )?;
        engine.prefill_decision(tokens.ids())?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(engine.ws.layers.len())
            .map_err(|_| E::AllocationFailed)?;
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
