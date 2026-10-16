//! Identified text frontend, never model execution or task authority.
mod admission;
mod bpe;
mod budget;
mod embedding;
mod error;
mod json;
mod prompt;
mod split;
mod unicode;
pub use budget::{AdmissionBudget, TextBudget};
pub use embedding::EmbeddingTokenizer;
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
        self.data.manifest_sha256
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
    manifest_sha256: [u8; 32],
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
        self.manifest_sha256
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
        Ok(EncodedTokens {
            ids,
            manifest_sha256: self.data.manifest_sha256,
        })
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
        Ok(EncodedTokens {
            ids,
            manifest_sha256: self.data.manifest_sha256,
        })
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

impl Tokenizer {
    pub fn encode_prompt_controlled(
        &self,
        messages: &[PromptMessage<'_>],
        budget: TextBudget,
        control: &crate::cpu::CpuControl,
    ) -> Result<EncodedTokens, TokenizeError> {
        control.check().map_err(budget::control_error)?;
        let mut c = budget::Counters::text(budget, self.data.resident)?;
        c.control = Some(control.clone());
        let ids = prompt::encode(
            &self.data,
            messages,
            &mut c,
            budget.max_tokens.min(self.data.context_limit),
        )?;
        control.check().map_err(budget::control_error)?;
        Ok(EncodedTokens {
            ids,
            manifest_sha256: self.data.manifest_sha256,
        })
    }
    pub fn decode_controlled(
        &self,
        ids: &[u32],
        budget: TextBudget,
        control: &crate::cpu::CpuControl,
    ) -> Result<String, TokenizeError> {
        control.check().map_err(budget::control_error)?;
        let mut c = budget::Counters::text(budget, self.data.resident)?;
        c.control = Some(control.clone());
        let text = bpe::decode(&self.data, ids, &mut c, budget.max_tokens)?;
        control.check().map_err(budget::control_error)?;
        Ok(text)
    }
    #[cfg(test)]
    pub(crate) fn test_synthetic() -> Self {
        let budget = AdmissionBudget {
            max_resident_bytes: 134217728,
            max_read_calls: 1024,
            max_interruptions: 16,
            max_work_per_artifact: 134217728,
        };
        Self {
            data: admission::test_load(
                include_bytes!("../../tests/fixtures/g29/synthetic-tokenizer.json"),
                include_bytes!("../../tests/fixtures/g29/pinned-config.json"),
                budget,
            )
            .unwrap(),
        }
    }
}

/// Separately admitted reranker frontend; no inference or authority.
#[derive(Debug)]
pub struct DecisionTokenizer {
    pub(crate) inner: Tokenizer,
    pub(crate) control: crate::cpu::CpuControl,
}
impl DecisionTokenizer {
    pub(crate) fn ensure_execution(
        &self,
        control: &crate::cpu::CpuControl,
    ) -> Result<(), crate::cpu::CpuError> {
        if self.control.same_execution(control) {
            Ok(())
        } else {
            Err(crate::cpu::CpuError::IdentityMismatch)
        }
    }
}
impl DecisionTokenizer {
    pub fn load<T: std::io::Read, C: std::io::Read>(
        tokenizer: &mut T,
        config: &mut C,
        manifest: &eylema_contracts::model_contract::ModelManifest,
        budget: AdmissionBudget,
        control: &crate::cpu::CpuControl,
    ) -> Result<Self, TokenizeError> {
        let inner = Tokenizer {
            data: admission::admit(
                tokenizer,
                config,
                manifest,
                budget,
                admission::ExpectedProfile::decision(control),
            )?,
        };
        for (id, want) in [(9693, "yes"), (2152, "no")] {
            let text = inner.decode_controlled(
                &[id],
                TextBudget {
                    max_resident_bytes: 134217728,
                    max_tokens: 1,
                    max_work: 2000000,
                },
                control,
            )?;
            if text != want {
                return Err(TokenizeError::InvalidVocabulary);
            }
        }
        Ok(Self {
            inner,
            control: control.clone(),
        })
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.inner.manifest_sha256()
    }
    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.inner.tokenizer_sha256()
    }
    pub fn config_sha256(&self) -> [u8; 32] {
        self.inner.config_sha256()
    }
}

impl DecisionTokenizer {
    pub fn encode_pair(
        &self,
        instruction: &str,
        query: &str,
        candidate: &str,
        b: TextBudget,
        control: &crate::cpu::CpuControl,
    ) -> Result<EncodedTokens, TokenizeError> {
        if [instruction, query, candidate]
            .iter()
            .any(|x| x.len() > 16384)
            || 205 + instruction.len() + query.len() + candidate.len() > 32768
        {
            return Err(TokenizeError::SizeLimit);
        }
        let mut c = budget::Counters::text(b, self.inner.data.resident)?;
        c.control = Some(control.clone());
        control.check().map_err(budget::control_error)?;
        for s in [instruction, query, candidate] {
            bpe::guard(&self.inner.data, s, &mut c)?;
        }
        let mut body = Vec::new();
        for s in [
            "<Instruction>: ",
            instruction,
            "\n<Query>: ",
            query,
            "\n<Candidate>: ",
            candidate,
        ] {
            c.charge(s.len())?;
            c.bytes(&mut body, s.as_bytes())?;
        }
        let user = std::str::from_utf8(&body).map_err(|_| TokenizeError::InvalidInput)?;
        let messages = [
            PromptMessage {
                role: PromptRole::System,
                content: "Judge if candidate matches query and instruction. Return yes or no.",
            },
            PromptMessage {
                role: PromptRole::User,
                content: user,
            },
        ];
        let ids = prompt::encode_profile(
            &self.inner.data,
            &messages,
            &mut c,
            b.max_tokens.min(4096),
            true,
        )?;
        control.check().map_err(budget::control_error)?;
        Ok(EncodedTokens {
            ids,
            manifest_sha256: self.manifest_sha256(),
        })
    }
}
