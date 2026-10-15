use super::{
    PromptMessage, PromptRole, TokenizeError as E, admission::AdmittedData, bpe, budget::Counters,
    unicode,
};
pub(super) fn encode(
    data: &AdmittedData,
    messages: &[PromptMessage<'_>],
    c: &mut Counters,
    limit: usize,
) -> Result<Vec<u32>, E> {
    if messages.len() > 8 {
        return Err(E::SizeLimit);
    }
    if messages.is_empty() {
        return Err(E::InvalidInput);
    }
    let first = usize::from(messages[0].role == PromptRole::System);
    if first == messages.len() || messages.last().is_none_or(|m| m.role != PromptRole::User) {
        return Err(E::InvalidInput);
    }
    let mut size = 0usize;
    for (i, m) in messages.iter().enumerate() {
        if m.content.len() > 8192 {
            return Err(E::SizeLimit);
        }
        size = size.checked_add(m.content.len()).ok_or(E::SizeLimit)?;
        if size > 32768 {
            return Err(E::SizeLimit);
        }
        if i >= first {
            let role = if (i - first).is_multiple_of(2) {
                PromptRole::User
            } else {
                PromptRole::Assistant
            };
            if m.role != role {
                return Err(E::InvalidInput);
            }
        }
    }
    let mut out = Vec::new();
    for m in messages {
        bpe::emit(&mut out, 151644, c, limit)?;
        let role = match m.role {
            PromptRole::System => "system\n",
            PromptRole::User => "user\n",
            PromptRole::Assistant => "assistant\n",
        };
        // LF terminates the fixed ASCII header, so NFC cannot compose across
        // this boundary. Normalize bounded content, then BPE the whole span.
        bpe::guard(data, m.content, c)?;
        let normalized = unicode::normalize(m.content, c)?;
        let mut body = Vec::new();
        let copied = role
            .len()
            .checked_add(normalized.len())
            .ok_or(E::SizeLimit)?;
        c.charge(copied)?;
        c.bytes(&mut body, role.as_bytes())?;
        c.bytes(&mut body, normalized.as_bytes())?;
        bpe::encode_normalized(
            data,
            std::str::from_utf8(&body).map_err(|_| E::InvalidInput)?,
            c,
            &mut out,
            limit,
        )?;
        c.drop_vec(body);
        let capacity = normalized.capacity();
        drop(normalized);
        c.release(capacity);
        bpe::emit(&mut out, 151645, c, limit)?;
        bpe::encode_text(data, "\n", c, &mut out, limit)?;
    }
    bpe::emit(&mut out, 151644, c, limit)?;
    bpe::encode_text(data, "assistant\n", c, &mut out, limit)?;
    bpe::emit(&mut out, 151667, c, limit)?;
    bpe::encode_text(data, "\n\n", c, &mut out, limit)?;
    bpe::emit(&mut out, 151668, c, limit)?;
    bpe::encode_text(data, "\n\n", c, &mut out, limit)?;
    Ok(out)
}
