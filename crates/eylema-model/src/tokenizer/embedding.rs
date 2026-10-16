use super::{
    AdmissionBudget, EncodedTokens, TextBudget, TokenizeError as E, Tokenizer,
    admission::{self, ExpectedProfile},
    bpe,
    budget::{self, Counters},
};
use crate::cpu::CpuControl;
use eylema_contracts::model_contract::ModelManifest;
use std::io::Read;

const MAX_RENDERED_BYTES: usize = 16_384;
const EOS_TOKEN_ID: u32 = 151_643;

/// A tokenizer admitted only for the pinned G68 embedding manifest.
pub struct EmbeddingTokenizer(Tokenizer);

impl std::fmt::Debug for EmbeddingTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EmbeddingTokenizer").field(&self.0).finish()
    }
}

impl EmbeddingTokenizer {
    pub fn load<T: Read, C: Read>(
        tokenizer: &mut T,
        config: &mut C,
        manifest: &ModelManifest,
        budget: AdmissionBudget,
    ) -> Result<Self, E> {
        let data = admission::admit(
            tokenizer,
            config,
            manifest,
            budget,
            ExpectedProfile::embedding(),
        )?;
        Ok(Self(Tokenizer { data }))
    }

    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.0.manifest_sha256()
    }

    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.0.tokenizer_sha256()
    }

    pub fn config_sha256(&self) -> [u8; 32] {
        self.0.config_sha256()
    }

    pub fn resident_bytes(&self) -> usize {
        self.0.data.resident
    }

    pub fn encode_query(
        &self,
        text: &str,
        budget: TextBudget,
        control: &CpuControl,
    ) -> Result<EncodedTokens, E> {
        self.encode(text, true, budget, control)
    }

    pub fn encode_document(
        &self,
        text: &str,
        budget: TextBudget,
        control: &CpuControl,
    ) -> Result<EncodedTokens, E> {
        self.encode(text, false, budget, control)
    }

    fn encode(
        &self,
        text: &str,
        query: bool,
        budget: TextBudget,
        control: &CpuControl,
    ) -> Result<EncodedTokens, E> {
        control.check().map_err(budget::control_error)?;
        if text.is_empty()
            || text.contains("<|")
            || text.contains("<tool_call>")
            || text.contains("</tool_call>")
            || text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(E::InvalidInput);
        }
        let prefix = if query {
            crate::embedding::profile::QUERY_PREFIX
        } else {
            ""
        };
        let rendered_len = prefix.len().checked_add(text.len()).ok_or(E::SizeLimit)?;
        if rendered_len == 0 || rendered_len > MAX_RENDERED_BYTES {
            return Err(E::SizeLimit);
        }
        if budget.max_tokens == 0 || budget.max_tokens > 4096 {
            return Err(E::InvalidInput);
        }
        let mut rendered = String::new();
        rendered
            .try_reserve_exact(rendered_len)
            .map_err(|_| E::AllocationFailed)?;
        control
            .charge(rendered_len as u64)
            .map_err(budget::control_error)?;
        rendered.push_str(prefix);
        rendered.push_str(text);

        let mut counters = Counters::text(budget, self.0.data.resident)?;
        counters.control = Some(control.clone());
        let mut ids = Vec::new();
        let limit = budget.max_tokens.min(self.0.data.context_limit);
        let content_limit = limit.checked_sub(1).ok_or(E::TokenLimit)?;
        bpe::encode_text(
            &self.0.data,
            &rendered,
            &mut counters,
            &mut ids,
            content_limit,
        )?;
        counters.push(&mut ids, EOS_TOKEN_ID)?;
        control.check().map_err(budget::control_error)?;
        Ok(EncodedTokens {
            ids,
            manifest_sha256: self.0.data.manifest_sha256,
        })
    }
}
