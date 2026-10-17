//! Factual verification independent of model scoring and checkpoint counters.
use super::types::*;
use eylema_contracts::codec::{self, Value};
#[derive(Clone)]
pub struct SelectedFacts {
    pub language: String,
    pub site: String,
    pub owner: String,
    pub checklist: String,
}
pub struct PreparedReport {
    bytes: Vec<u8>,
    facts: usize,
    need: [u8; 32],
}
impl PreparedReport {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn fact_count(&self) -> usize {
        self.facts
    }
    pub fn need(&self) -> [u8; 32] {
        self.need
    }
    pub fn bind_citations(mut self, pins: &super::storage::VerifiedPins) -> Result<Self> {
        let mut rows = match codec::decode(&self.bytes).map_err(|_| ReportError::InvalidInput)? {
            Value::List(x) => x,
            _ => return Err(ReportError::InvalidInput),
        };
        rows.push(pins.value());
        self.bytes = codec::encode(&Value::List(rows)).map_err(|_| ReportError::Limit)?;
        Ok(self)
    }

    pub fn digest(&self) -> Result<[u8; 32]> {
        eylema_platform::sha256(&self.bytes).map_err(|_| ReportError::Unsupported)
    }
}
pub fn prepare_report(
    need: &ReportNeed,
    sources: &[SourceDocument],
    selected: &[SelectedFacts],
    planner: [u8; 32],
    decision: [u8; 32],
    reviewed: bool,
) -> Result<PreparedReport> {
    if !reviewed {
        return Err(ReportError::NeedsSourceReview);
    }
    if planner == [0; 32] || decision == [0; 32] || selected.len() != need.sections.len() {
        return Err(ReportError::InvalidWitness);
    }
    let mut rows = vec![];
    for (language, site) in &need.sections {
        let matching: Vec<_> = sources
            .iter()
            .filter(|s| &s.language == language && &s.site == site)
            .collect();
        let mut owners = std::collections::BTreeSet::new();
        let mut checklists = std::collections::BTreeSet::new();
        for s in matching {
            if let Some(x) = &s.owner {
                owners.insert(x.as_str());
            }
            if let Some(x) = &s.checklist {
                checklists.insert(x.as_str());
            }
        }
        if owners.len() > 1 || checklists.len() > 1 {
            return Err(ReportError::Conflict);
        }
        let owner = owners.first().ok_or(ReportError::NoEvidence)?;
        let checklist = checklists.first().ok_or(ReportError::NoEvidence)?;
        let chosen: Vec<_> = selected
            .iter()
            .filter(|s| &s.language == language && &s.site == site)
            .collect();
        if chosen.len() != 1 || &chosen[0].owner != owner || &chosen[0].checklist != checklist {
            return Err(ReportError::InvalidWitness);
        }
        rows.push(Value::List(vec![
            Value::Text(language.clone()),
            Value::Text(site.clone()),
            Value::Text((*owner).into()),
            Value::Text((*checklist).into()),
        ]));
    }
    let need = need.digest()?;
    let bytes = codec::encode(&Value::List(vec![
        Value::Text("report:v1".into()),
        Value::Bytes(need.to_vec()),
        Value::List(rows),
        Value::List(vec![
            Value::Text("Unknown".into()),
            Value::Text("no_evidence".into()),
        ]),
        Value::Bytes(planner.to_vec()),
        Value::Bytes(decision.to_vec()),
    ]))
    .map_err(|_| ReportError::Limit)?;
    Ok(PreparedReport {
        bytes,
        facts: 2 * selected.len(),
        need,
    })
}
