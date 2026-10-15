use super::score::{DecisionReceipt, DecisionResult, OptionScore, QuestionScores};
use crate::cpu::CpuError as E;
pub struct DecisionOption<'a> {
    pub id: &'a str,
    pub candidate: &'a str,
}
pub struct DecisionQuestion<'a> {
    pub id: &'a str,
    pub instruction: &'a str,
    pub query: &'a str,
    pub options: &'a [DecisionOption<'a>],
}
pub struct DecisionBatch<'a> {
    pub(crate) questions: &'a [DecisionQuestion<'a>],
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_:.-".contains(&b))
}
impl<'a> DecisionBatch<'a> {
    pub fn new(questions: &'a [DecisionQuestion<'a>]) -> Result<Self, E> {
        if questions.is_empty() || questions.len() > 4 {
            return Err(E::InvalidInput);
        }
        let mut raw = 0usize;
        let mut rendered = 0usize;
        for (i, q) in questions.iter().enumerate() {
            if !id(q.id)
                || questions[..i].iter().any(|x| x.id == q.id)
                || q.options.is_empty()
                || q.options.len() > 4
            {
                return Err(E::InvalidInput);
            }
            if q.instruction.len() > 16384 || q.query.len() > 16384 {
                return Err(E::InvalidInput);
            }
            raw = raw
                .checked_add(q.instruction.len() + q.query.len())
                .ok_or(E::InvalidInput)?;
            for (j, o) in q.options.iter().enumerate() {
                if !id(o.id)
                    || q.options[..j].iter().any(|x| x.id == o.id)
                    || o.candidate.len() > 16384
                {
                    return Err(E::InvalidInput);
                }
                raw = raw.checked_add(o.candidate.len()).ok_or(E::InvalidInput)?;
                let n = 205usize
                    .checked_add(q.instruction.len() + q.query.len() + o.candidate.len())
                    .ok_or(E::InvalidInput)?;
                if n > 32768 {
                    return Err(E::InvalidInput);
                }
                rendered = rendered.checked_add(n).ok_or(E::InvalidInput)?;
            }
        }
        if raw > 65536 || rendered > 524288 {
            return Err(E::InvalidInput);
        }
        Ok(Self { questions })
    }
}

pub(crate) fn output_reservation(batch: &DecisionBatch<'_>) -> Result<u64, E> {
    let mut bytes = std::mem::size_of::<DecisionResult>()
        .checked_add(std::mem::size_of::<DecisionReceipt>())
        .ok_or(E::ResidentLimit)? as u64;
    for question in batch.questions {
        bytes = bytes
            .checked_add(std::mem::size_of::<QuestionScores>() as u64)
            .and_then(|n| n.checked_add(question.id.len() as u64))
            .and_then(|n| {
                n.checked_add((question.options.len() * std::mem::size_of::<OptionScore>()) as u64)
            })
            .ok_or(E::ResidentLimit)?;
        for option in question.options {
            bytes = bytes
                .checked_add(option.id.len() as u64)
                .ok_or(E::ResidentLimit)?;
        }
        let largest_choice = question
            .options
            .iter()
            .map(|option| option.id.len())
            .max()
            .unwrap_or(0);
        bytes = bytes
            .checked_add(largest_choice as u64)
            .ok_or(E::ResidentLimit)?;
    }
    Ok(bytes)
}

pub(crate) fn actual_output_bytes(questions: &[QuestionScores], capacity: usize) -> Result<u64, E> {
    let mut bytes =
        (std::mem::size_of::<DecisionResult>() + std::mem::size_of::<DecisionReceipt>()) as u64;
    bytes = bytes
        .checked_add(
            (capacity as u64)
                .checked_mul(std::mem::size_of::<QuestionScores>() as u64)
                .ok_or(E::ResidentLimit)?,
        )
        .ok_or(E::ResidentLimit)?;
    for question in questions {
        bytes = bytes
            .checked_add(question.id.capacity() as u64)
            .and_then(|n| {
                n.checked_add(
                    (question.scores.capacity() as u64)
                        .checked_mul(std::mem::size_of::<OptionScore>() as u64)?,
                )
            })
            .and_then(|n| n.checked_add(question.choice.as_ref()?.capacity() as u64))
            .ok_or(E::ResidentLimit)?;
        for score in &question.scores {
            bytes = bytes
                .checked_add(score.id.capacity() as u64)
                .ok_or(E::ResidentLimit)?;
        }
    }
    Ok(bytes)
}

pub(crate) fn copy(s: &str) -> Result<String, E> {
    let mut out = String::new();
    out.try_reserve_exact(s.len())
        .map_err(|_| E::AllocationFailed)?;
    out.push_str(s);
    Ok(out)
}
