use super::score::{DecisionReceipt, DecisionResult, OptionScore, QuestionScores};
use crate::cpu::CpuControl;
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

pub(crate) fn workspace_reservation(context: usize) -> Result<(u64, u64), E> {
    crate::decision::profile::workspace_reservation(context)
}

pub(crate) fn preflight(
    batch: &DecisionBatch<'_>,
    base: u64,
    context: usize,
    max_resident: u64,
    max_work: u64,
    observed_work: u64,
) -> Result<u64, E> {
    let output = output_reservation(batch)?;
    let (workspace_bytes, workspace_work) = workspace_reservation(context)?;
    let resident = base
        .checked_add(output)
        .and_then(|n| n.checked_add(workspace_bytes))
        .ok_or(E::ResidentLimit)?;
    if resident > max_resident {
        return Err(E::ResidentLimit);
    }
    if observed_work
        .checked_add(output)
        .and_then(|n| n.checked_add(workspace_work))
        .is_none_or(|n| n > max_work)
    {
        return Err(E::WorkLimit);
    }
    Ok(output)
}

pub(crate) fn identity(batch: &DecisionBatch<'_>, control: &CpuControl) -> Result<[u8; 32], E> {
    let mut hash = eylema_platform::sha256_stream::Sha256Stream::new(1048576)
        .map_err(|_| E::PlatformUnavailable)?;
    hash_part(&mut hash, b"decision.query-options.v1", control)?;
    hash_count(&mut hash, batch.questions.len(), control)?;
    for q in batch.questions {
        for s in [q.id, q.instruction, q.query] {
            hash_part(&mut hash, s.as_bytes(), control)?;
        }
        hash_count(&mut hash, q.options.len(), control)?;
        for o in q.options {
            hash_part(&mut hash, o.id.as_bytes(), control)?;
            hash_part(&mut hash, o.candidate.as_bytes(), control)?;
        }
    }
    hash.finish().map_err(|_| E::PlatformUnavailable)
}

fn hash_count(
    hash: &mut eylema_platform::sha256_stream::Sha256Stream,
    count: usize,
    control: &CpuControl,
) -> Result<(), E> {
    let count = u32::try_from(count).map_err(|_| E::InvalidInput)?;
    control.charge(4)?;
    hash.update(&count.to_le_bytes())
        .map_err(|_| E::PlatformUnavailable)
}

fn hash_part(
    hash: &mut eylema_platform::sha256_stream::Sha256Stream,
    bytes: &[u8],
    control: &CpuControl,
) -> Result<(), E> {
    let len = u32::try_from(bytes.len()).map_err(|_| E::InvalidInput)?;
    control.charge(4)?;
    hash.update(&len.to_le_bytes())
        .map_err(|_| E::PlatformUnavailable)?;
    for block in bytes.chunks(4096) {
        control.charge(block.len() as u64)?;
        hash.update(block).map_err(|_| E::PlatformUnavailable)?;
    }
    Ok(())
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
