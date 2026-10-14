//! Identified text frontend, never model execution or task authority.
mod admission;
mod bpe;
mod budget;
mod error;
mod json;
mod prompt;
mod split;
mod unicode;
pub use budget::{AdmissionBudget, TextBudget};
pub use error::TokenizeError;
#[cfg(test)]
mod tests;
pub struct Tokenizer {
    data: admission::AdmittedData,
}
impl std::fmt::Debug for Tokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokenizer")
            .field("vocabulary_entries", &self.data.vocab.len())
            .field("merge_entries", &self.data.merges.len())
            .finish()
    }
}
impl Tokenizer {
    pub fn load<T: std::io::Read, C: std::io::Read>(
        tokenizer: &mut T,
        config: &mut C,
        manifest: &eylema_contracts::model_contract::ModelManifest,
        budget: AdmissionBudget,
    ) -> Result<Self, TokenizeError> {
        Ok(Self {
            data: admission::admit(
                tokenizer,
                config,
                manifest,
                budget,
                admission::ExpectedProfile::qwen(),
            )?,
        })
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        admission::MANIFEST_SHA
    }
    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.data.tokenizer_sha256
    }
    pub fn config_sha256(&self) -> [u8; 32] {
        self.data.config_sha256
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptRole {
    System,
    User,
    Assistant,
}
#[derive(Clone, Copy)]
pub struct PromptMessage<'a> {
    pub role: PromptRole,
    pub content: &'a str,
}
pub struct EncodedTokens {
    ids: Vec<u32>,
}
impl std::fmt::Debug for EncodedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncodedTokens")
            .field("count", &self.ids.len())
            .finish()
    }
}
impl EncodedTokens {
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        admission::MANIFEST_SHA
    }
}
impl Tokenizer {
    pub fn encode_text(
        &self,
        text: &str,
        budget: TextBudget,
    ) -> Result<EncodedTokens, TokenizeError> {
        if text.len() > 8192 {
            return Err(TokenizeError::SizeLimit);
        }
        let mut c = budget::Counters::text(budget, self.data.resident)?;
        let mut ids = Vec::new();
        bpe::encode_text(&self.data, text, &mut c, &mut ids, budget.max_tokens)?;
        Ok(EncodedTokens { ids })
    }
    pub fn encode_prompt(
        &self,
        messages: &[PromptMessage<'_>],
        budget: TextBudget,
    ) -> Result<EncodedTokens, TokenizeError> {
        let mut c = budget::Counters::text(budget, self.data.resident)?;
        let ids = prompt::encode(
            &self.data,
            messages,
            &mut c,
            budget.max_tokens.min(self.data.context_limit),
        )?;
        Ok(EncodedTokens { ids })
    }
    pub fn decode(&self, ids: &[u32], budget: TextBudget) -> Result<String, TokenizeError> {
        let mut c = budget::Counters::text(budget, self.data.resident)?;
        bpe::decode(&self.data, ids, &mut c, budget.max_tokens)
    }
}
impl std::fmt::Debug for PromptMessage<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptMessage")
            .field("role", &self.role)
            .field("content_bytes", &self.content.len())
            .finish()
    }
}
