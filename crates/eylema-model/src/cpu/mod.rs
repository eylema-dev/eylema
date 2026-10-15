//! Closed, explicitly bounded owned CPU execution. Proposals never grant authority.
mod attention;
mod budget;
mod control;
mod error;
mod ops;
mod profile;
mod roles;
mod session;
pub use roles::{CpuProposal, CpuReceipt, CpuRoleOutput, CpuRoleRequest, StopReason};
mod weights;
use crate::{
    LoadedWeights,
    tokenizer::{EncodedTokens, Tokenizer},
};
pub use budget::CpuBudget;
use budget::Meter;
pub use control::CpuControl;
pub use error::CpuError;
use eylema_contracts::model_contract::ModelManifest;
pub use profile::qwen_layout;
#[cfg(test)]
mod tests;
pub struct CpuModel<'a> {
    weights: weights::BoundWeights<'a>,
    manifest: &'a ModelManifest,
    tokenizer: &'a Tokenizer,
    base_bytes: u64,
    admission_work: u64,
}
impl std::fmt::Debug for CpuModel<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuModel")
            .field("admission_work", &self.admission_work)
            .field("roles", &self.manifest.fields().profiles.len())
            .field("tokenizer_sha256", &self.tokenizer.tokenizer_sha256())
            .finish_non_exhaustive()
    }
}
impl<'a> CpuModel<'a> {
    pub fn admit(
        manifest: &'a ModelManifest,
        weights: &'a LoadedWeights,
        tokenizer: &'a Tokenizer,
        budget: CpuBudget,
        control: &CpuControl,
    ) -> Result<Self, CpuError> {
        budget.validate()?;
        control.check()?;
        if tokenizer.manifest_sha256() != profile::PARENT_SHA {
            return Err(CpuError::IdentityMismatch);
        }
        let fields = manifest.fields();
        for (name, actual) in [
            ("tokenizer.json", tokenizer.tokenizer_sha256()),
            ("tokenizer_config.json", tokenizer.config_sha256()),
        ] {
            if fields
                .components
                .iter()
                .find(|c| c.name == name)
                .is_none_or(|c| c.sha256 != actual)
            {
                return Err(CpuError::IdentityMismatch);
            }
        }
        let base_bytes = weights
            .resident_content_bytes()
            .checked_add(134_217_728 + 4_194_304 + 131_072)
            .ok_or(CpuError::ResidentLimit)?;
        let mut meter = Meter::new(budget, control.clone(), base_bytes)?;
        meter.preflight(131_072)?;
        let layout = qwen_layout(manifest)?;
        if weights.manifest_sha256() != profile::CPU_SHA
            || weights.layout_sha256() != layout.layout_sha256()
        {
            return Err(CpuError::IdentityMismatch);
        }
        meter.charge(131_072)?;
        weights::scan(weights, &layout, &mut meter)?;
        let bound = weights::BoundWeights::loaded(profile::QWEN, weights)?;
        control.check()?;
        Ok(Self {
            weights: bound,
            manifest,
            tokenizer,
            base_bytes,
            admission_work: control.observed_work(),
        })
    }
    pub fn begin<'m>(
        &'m self,
        budget: CpuBudget,
        control: CpuControl,
    ) -> Result<CpuSession<'m, 'a>, CpuError> {
        Ok(CpuSession {
            model: self,
            engine: session::Engine::new(
                profile::QWEN,
                self.weights,
                budget,
                control,
                self.base_bytes,
            )?,
        })
    }
    pub fn admission_work(&self) -> u64 {
        self.admission_work
    }
}
pub struct CpuSession<'m, 'a> {
    model: &'m CpuModel<'a>,
    engine: session::Engine<'a>,
}
impl std::fmt::Debug for CpuSession<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuSession")
            .field("positions", &self.engine.pos)
            .field("model_roles", &self.model.manifest.fields().profiles.len())
            .field("valid", &self.engine.valid)
            .finish_non_exhaustive()
    }
}
impl CpuSession<'_, '_> {
    pub fn prefill(&mut self, tokens: &EncodedTokens) -> Result<CpuLogits, CpuError> {
        if tokens.manifest_sha256() != profile::PARENT_SHA {
            self.engine.valid = false;
            return Err(CpuError::IdentityMismatch);
        }
        self.engine.prefill_ids(tokens.ids())?;
        self.engine.output()
    }
    /// Append numerical input under context/work/deadline bounds. This diagnostic
    /// API does not sample or return generated text; `generate` enforces the
    /// separate `max_new_tokens` proposal limit.
    pub fn decode(&mut self, token: u32) -> Result<CpuLogits, CpuError> {
        self.engine.decode_id(token)?;
        self.engine.output()
    }
    /// Numerical diagnostic for the most recent fully committed token, flattened
    /// layer-major. It is neither language reasoning nor an accepted role proposal.
    pub fn last_layer_hidden(&self) -> Result<&[f32], CpuError> {
        if !self.engine.valid || self.engine.pos == 0 {
            return Err(CpuError::InvalidState);
        }
        Ok(&self.engine.ws.layers)
    }
    pub fn observed_work(&self) -> u64 {
        self.engine.meter.control.observed_work()
    }
    pub fn peak_logical_bytes(&self) -> u64 {
        self.engine.meter.peak
    }
}
pub struct CpuLogits {
    values: Vec<f32>,
}
impl CpuLogits {
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}
impl std::fmt::Debug for CpuLogits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuLogits")
            .field("count", &self.values.len())
            .finish()
    }
}
