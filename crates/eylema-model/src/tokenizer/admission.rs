use super::{
    TokenizeError as E,
    budget::{AdmissionBudget, Counters, compare, sort},
    json::{self, Parser},
};
use eylema_contracts::model_contract::ModelManifest;
use eylema_platform::sha256_stream::Sha256Stream;
use std::cmp::Ordering;
use std::io::{ErrorKind, Read};
const READ_CHUNK_BYTES: usize = 65536;
pub(super) const MANIFEST_SHA: [u8; 32] =
    hex(b"c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2");
const TOKEN_SHA: [u8; 32] =
    hex(b"aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4");
const CONFIG_SHA: [u8; 32] =
    hex(b"d5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101");
const TEMPLATE_SHA: [u8; 32] =
    hex(b"a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8");
pub(super) const fn hex(s: &[u8; 64]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut i = 0;
    while i < 32 {
        let a = s[i * 2];
        let b = s[i * 2 + 1];
        out[i] = ((if a <= b'9' { a - b'0' } else { a - b'a' + 10 }) << 4)
            | (if b <= b'9' { b - b'0' } else { b - b'a' + 10 });
        i += 1;
    }
    out
}
pub(super) struct ExpectedProfile {
    pub tokenizer_size: usize,
    pub config_size: usize,
    pub tokenizer_sha256: [u8; 32],
    pub config_sha256: [u8; 32],
    pub decision: bool,
    pub control: Option<crate::cpu::CpuControl>,
}
pub(super) const DECISION_SHA: [u8; 32] =
    hex(b"c47cc3f8008e4735ab43acfde124b884cab99f9206913cbecfc79ab06e2d0cdc");
impl ExpectedProfile {
    pub fn decision(control: &crate::cpu::CpuControl) -> Self {
        Self {
            tokenizer_size: 11422654,
            config_size: 9706,
            tokenizer_sha256: TOKEN_SHA,
            config_sha256: hex(b"253153d0738ceb4c668d2eff957714dd2bea0b56de772a9fdccd96cbf517e6a0"),
            decision: true,
            control: Some(control.clone()),
        }
    }
    pub fn qwen() -> Self {
        Self {
            tokenizer_size: 11422654,
            config_size: 9732,
            tokenizer_sha256: TOKEN_SHA,
            config_sha256: CONFIG_SHA,
            decision: false,
            control: None,
        }
    }
}
pub(super) struct Token {
    pub text: String,
    pub id: u32,
}
pub(super) struct Merge {
    pub left: u32,
    pub right: u32,
    pub out: u32,
    pub rank: u32,
}
pub(super) struct Id {
    pub id: u32,
    pub index: usize,
    pub added: bool,
}
pub(super) struct AdmittedData {
    pub vocab: Vec<Token>,
    pub merges: Vec<Merge>,
    pub added: Vec<Token>,
    pub by_id: Vec<Id>,
    pub byte_ids: [u32; 256],
    pub tokenizer_sha256: [u8; 32],
    pub config_sha256: [u8; 32],
    pub resident: usize,
    pub context_limit: usize,
    pub manifest_sha256: [u8; 32],
}
impl std::fmt::Debug for AdmittedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmittedData")
            .field("vocab_count", &self.vocab.len())
            .field("merge_count", &self.merges.len())
            .field("resident_bytes", &self.resident)
            .finish()
    }
}
pub(super) fn digest(bytes: &[u8]) -> Result<[u8; 32], E> {
    let mut h = Sha256Stream::new(bytes.len().max(1) as u64)?;
    h.update(bytes)?;
    Ok(h.finish()?)
}
fn read<R: Read>(r: &mut R, size: usize, hash: [u8; 32], c: &mut Counters) -> Result<Vec<u8>, E> {
    let mut bytes = Vec::new();
    c.vec(&mut bytes, size)?;
    let mut h = Sha256Stream::new(size.max(1) as u64)?;
    let mut buf = [0; READ_CHUNK_BYTES];
    loop {
        if c.reads >= c.max_reads {
            return Err(E::WorkLimit);
        }
        c.reads += 1;
        let want = (size - bytes.len()).min(buf.len()).max(1);
        let n = match r.read(&mut buf[..want]) {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                if c.interruptions >= c.max_interruptions {
                    return Err(E::WorkLimit);
                }
                c.interruptions += 1;
                continue;
            }
            Err(_) => return Err(E::Io),
        };
        if n > want {
            return Err(E::Io);
        }
        if n == 0 {
            if bytes.len() != size {
                return Err(E::Io);
            }
            break;
        }
        if bytes.len() + n > size {
            return Err(E::SizeLimit);
        }
        c.charge(n)?;
        h.update(&buf[..n])?;
        bytes.extend_from_slice(&buf[..n]);
    }
    if h.finish()? != hash {
        return Err(E::HashMismatch);
    }
    Ok(bytes)
}
fn text(data: &[u8], c: &mut Counters, limit: usize) -> Result<String, E> {
    let mut p = Parser::new(data, c, limit)?;
    let s = p.text()?;
    if p.at != data.len() {
        return Err(E::InvalidJson);
    }
    Ok(s)
}
fn semantic(data: &[u8], expected: &[u8], c: &mut Counters) -> Result<(), E> {
    if data.first() == Some(&b'{') {
        let a = Parser::new(data, c, 8192)?.object()?;
        let b = Parser::new(expected, c, 8192)?.object()?;
        if a.len() != b.len() {
            return Err(E::InvalidConfig);
        }
        for ((ak, ar), (bk, br)) in a.iter().zip(b.iter()) {
            if ak != bk {
                return Err(E::InvalidConfig);
            }
            semantic(&data[ar.clone()], &expected[br.clone()], c)?;
        }
        json::free_fields(c, a);
        json::free_fields(c, b);
    } else if data.first() == Some(&b'[') {
        let a = Parser::new(data, c, 8192)?.array()?;
        let b = Parser::new(expected, c, 8192)?.array()?;
        if a.len() != b.len() {
            return Err(E::InvalidConfig);
        }
        for (ar, br) in a.iter().zip(b.iter()) {
            semantic(&data[ar.clone()], &expected[br.clone()], c)?;
        }
        c.drop_vec(a);
        c.drop_vec(b);
    } else if data.first() == Some(&b'"') {
        let a = text(data, c, 8192)?;
        let b = text(expected, c, 8192)?;
        if a != b {
            return Err(E::InvalidConfig);
        }
        let n = a.capacity() + b.capacity();
        drop(a);
        drop(b);
        c.release(n);
    } else if data != expected {
        return Err(E::InvalidConfig);
    }
    Ok(())
}
pub(super) fn alphabet() -> [char; 256] {
    let mut out = ['\0'; 256];
    let mut next = 256;
    for (b, ch) in out.iter_mut().enumerate() {
        let cp = if (33..=126).contains(&b) || (161..=172).contains(&b) || (174..=255).contains(&b)
        {
            b as u32
        } else {
            let cp = next;
            next += 1;
            cp
        };
        *ch = char::from_u32(cp).expect("fixed byte alphabet scalar");
    }
    out
}
fn fingerprint(key: &str, c: &mut Counters) -> Result<u64, E> {
    let mut h = 14695981039346656037u64;
    for b in key.bytes() {
        c.charge(1)?;
        h = (h ^ u64::from(b)).wrapping_mul(1099511628211);
    }
    Ok(h)
}
fn token_id(v: &[Token], index: &[(u64, usize)], key: &str, c: &mut Counters) -> Result<u32, E> {
    let h = fingerprint(key, c)?;
    c.charge(18)?;
    let (mut lo, mut hi) = (0, index.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if index[mid].0 < h {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    while lo < index.len() && index[lo].0 == h {
        c.charge(1)?;
        let candidate = &v[index[lo].1];
        if compare(c, &candidate.text, key)? == Ordering::Equal {
            return Ok(candidate.id);
        }
        lo += 1;
    }
    Err(E::InvalidVocabulary)
}
pub(super) fn admit<T: Read, C: Read>(
    t: &mut T,
    cfg: &mut C,
    manifest: &ModelManifest,
    budget: AdmissionBudget,
    e: ExpectedProfile,
) -> Result<AdmittedData, E> {
    let mut c = Counters::admission(budget)?;
    c.control = e.control;
    if let Some(control) = &c.control {
        control.check().map_err(super::budget::control_error)?;
    }
    if e.tokenizer_size == 0
        || e.tokenizer_size > 16777216
        || e.config_size == 0
        || e.config_size > 65536
    {
        return Err(E::SizeLimit);
    }
    let rawsize = e
        .tokenizer_size
        .checked_add(e.config_size)
        .ok_or(E::ResidentLimit)?;
    if rawsize + 65536 > c.max_resident {
        return Err(E::ResidentLimit);
    }
    // Necessary lower bounds only: parsing and short/interrupted reads still
    // consume the unchanged incremental budgets after this preflight.
    let tokenizer_work = u64::try_from(e.tokenizer_size).map_err(|_| E::WorkLimit)?;
    let config_work = u64::try_from(e.config_size).map_err(|_| E::WorkLimit)?;
    let minimum_reads = e
        .tokenizer_size
        .div_ceil(READ_CHUNK_BYTES)
        .checked_add(e.config_size.div_ceil(READ_CHUNK_BYTES))
        .and_then(|n| n.checked_add(2)) // One EOF probe for each artifact.
        .ok_or(E::WorkLimit)?;
    if tokenizer_work > c.max_work
        || config_work > c.max_work
        || u64::try_from(minimum_reads).map_err(|_| E::WorkLimit)? > u64::from(c.max_reads)
    {
        return Err(E::WorkLimit);
    }
    c.hold(65536 + 32768)?;
    let m = manifest.to_bytes().map_err(|_| E::InvalidProfile)?;
    let expected_manifest = if e.decision {
        DECISION_SHA
    } else {
        MANIFEST_SHA
    };
    if digest(&m)? != expected_manifest {
        return Err(E::UnsupportedProfile);
    }
    drop(m);
    c.release(32768);
    let raw = read(t, e.tokenizer_size, e.tokenizer_sha256, &mut c)?;
    let fields = Parser::new(&raw, &mut c, 4096)?.object()?;
    if fields.len() != 9 {
        return Err(E::InvalidConfig);
    }
    for(k,val)in [("version",b"\"1.0\"".as_slice()),("truncation",b"null"),("padding",b"null"),("normalizer",br#"{"type":"NFC"}"#),("post_processor",br#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#),("decoder",br#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#)]{semantic(json::get(&raw,&fields,k)?,val,&mut c)?;}
    semantic(
        json::get(&raw, &fields, "pre_tokenizer")?,
        PRETOKENIZER,
        &mut c,
    )?;
    let model = json::get(&raw, &fields, "model")?;
    let mf = Parser::new(model, &mut c, 4096)?.object()?;
    if mf.len() != 10 {
        return Err(E::InvalidConfig);
    }
    for (k, val) in [
        ("type", b"\"BPE\"".as_slice()),
        ("dropout", b"null"),
        ("unk_token", b"null"),
        ("continuing_subword_prefix", b"\"\""),
        ("end_of_word_suffix", b"\"\""),
        ("fuse_unk", b"false"),
        ("byte_fallback", b"false"),
        ("ignore_merges", b"false"),
    ] {
        semantic(json::get(model, &mf, k)?, val, &mut c)?;
    }
    let vd = json::get(model, &mf, "vocab")?;
    let mut p = Parser::new(vd, &mut c, 4096)?;
    p.key_limit = 4096;
    let vf = p.object()?;
    if vf.is_empty() || vf.len() > 160000 {
        return Err(E::InvalidVocabulary);
    }
    let mut vocab = Vec::new();
    c.vec(&mut vocab, vf.len())?;
    let vc = vf.capacity() * std::mem::size_of::<(String, json::Span)>();
    for (name, r) in vf {
        let id = json::uint(&vd[r])?;
        if id >= if e.decision { 151669 } else { 151936 } {
            return Err(E::InvalidVocabulary);
        }
        vocab.push(Token { text: name, id });
    }
    c.release(vc);
    // Validate ID uniqueness before merge references can obscure vocabulary errors.
    let mut ids = Vec::new();
    c.vec(&mut ids, vocab.len())?;
    for v in &vocab {
        ids.push(v.id);
    }
    sort(&mut ids, &mut c, |a, b, c| {
        c.charge(1)?;
        Ok(a.cmp(b))
    })?;
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(E::InvalidVocabulary);
    }
    c.drop_vec(ids);
    // Sorted non-cryptographic lookup hints always confirm full text equality.
    let mut index = Vec::new();
    c.vec(&mut index, vocab.len())?;
    for (i, v) in vocab.iter().enumerate() {
        index.push((fingerprint(&v.text, &mut c)?, i));
    }
    sort(&mut index, &mut c, |a, b, c| {
        c.charge(1)?;
        Ok(a.0.cmp(&b.0))
    })?;
    // The streaming object is already sorted by decoded key; no vocabulary renumbering.
    let mut byte_ids = [0; 256];
    for (b, ch) in alphabet().iter().enumerate() {
        let mut buf = [0; 4];
        byte_ids[b] = token_id(&vocab, &index, ch.encode_utf8(&mut buf), &mut c)?;
    }
    let md = json::get(model, &mf, "merges")?;
    let spans = Parser::new(md, &mut c, 4096)?.array()?;
    if spans.len() > 160000 {
        return Err(E::InvalidMerge);
    }
    let mut merges = Vec::new();
    c.vec(&mut merges, spans.len())?;
    for (rank, span) in spans.iter().enumerate() {
        let pair = &md[span.clone()];
        let pieces = Parser::new(pair, &mut c, 4096)?.array()?;
        if pieces.len() != 2 {
            return Err(E::InvalidMerge);
        }
        let left = text(&pair[pieces[0].clone()], &mut c, 4096)?;
        let right = text(&pair[pieces[1].clone()], &mut c, 4096)?;
        let l = token_id(&vocab, &index, &left, &mut c).map_err(|e| {
            if e == E::InvalidVocabulary {
                E::InvalidMerge
            } else {
                e
            }
        })?;
        let r = token_id(&vocab, &index, &right, &mut c).map_err(|e| {
            if e == E::InvalidVocabulary {
                E::InvalidMerge
            } else {
                e
            }
        })?;
        let n = left.len().checked_add(right.len()).ok_or(E::SizeLimit)?;
        if n > 4096 {
            return Err(E::InvalidMerge);
        }
        let mut joined = Vec::new();
        c.bytes(&mut joined, left.as_bytes())?;
        c.bytes(&mut joined, right.as_bytes())?;
        let key = std::str::from_utf8(&joined).map_err(|_| E::InvalidMerge)?;
        let out = token_id(&vocab, &index, key, &mut c).map_err(|e| {
            if e == E::InvalidVocabulary {
                E::InvalidMerge
            } else {
                e
            }
        })?;
        merges.push(Merge {
            left: l,
            right: r,
            out,
            rank: rank as u32,
        });
        let freed = left.capacity() + right.capacity();
        drop(left);
        drop(right);
        c.release(freed);
        c.drop_vec(joined);
        c.drop_vec(pieces);
    }
    c.drop_vec(spans);
    sort(&mut merges, &mut c, |a, b, c| {
        c.charge(1)?;
        Ok((a.left, a.right).cmp(&(b.left, b.right)))
    })?;
    for w in merges.windows(2) {
        if (w[0].left, w[0].right) == (w[1].left, w[1].right) {
            return Err(E::InvalidMerge);
        }
    }
    let ad = json::get(&raw, &fields, "added_tokens")?;
    let spans = Parser::new(ad, &mut c, 4096)?.array()?;
    if spans.len() != 26 {
        return Err(E::InvalidVocabulary);
    }
    let mut added = Vec::new();
    c.vec(&mut added, 26)?;
    for (index, span) in spans.iter().enumerate() {
        let obj = &ad[span.clone()];
        let f = Parser::new(obj, &mut c, 4096)?.object()?;
        if f.len() != 7 {
            return Err(E::InvalidConfig);
        }
        let id = json::uint(json::get(obj, &f, "id")?)?;
        let name = text(json::get(obj, &f, "content")?, &mut c, 4096)?;
        if id != 151643 + index as u32 || name != ADDED[index] {
            return Err(E::InvalidVocabulary);
        }
        for key in ["single_word", "lstrip", "rstrip", "normalized"] {
            if json::get(obj, &f, key)? != b"false" {
                return Err(E::InvalidConfig);
            }
        }
        let special = json::get(obj, &f, "special")?;
        if ![b"true".as_slice(), b"false"].contains(&special) {
            return Err(E::InvalidConfig);
        }
        added.push(Token { text: name, id });
        json::free_fields(&mut c, f);
    }
    c.drop_vec(spans);
    let mut by_id = Vec::new();
    c.vec(&mut by_id, vocab.len() + added.len())?;
    for (i, v) in vocab.iter().enumerate() {
        by_id.push(Id {
            id: v.id,
            index: i,
            added: false,
        });
    }
    for (i, v) in added.iter().enumerate() {
        by_id.push(Id {
            id: v.id,
            index: i,
            added: true,
        });
    }
    sort(&mut by_id, &mut c, |a, b, c| {
        c.charge(1)?;
        Ok(a.id.cmp(&b.id))
    })?;
    if by_id.windows(2).any(|w| w[0].id == w[1].id) {
        return Err(E::InvalidVocabulary);
    }
    c.drop_vec(index);
    json::free_fields(&mut c, mf);
    json::free_fields(&mut c, fields);
    c.drop_vec(raw);
    c.work = 0;
    let rawcfg = read(cfg, e.config_size, e.config_sha256, &mut c)?;
    let f = Parser::new(&rawcfg, &mut c, 8192)?.object()?;
    if (!e.decision && f.len() != 14) || f.len() > 32 {
        return Err(E::InvalidConfig);
    }
    let template = text(json::get(&rawcfg, &f, "chat_template")?, &mut c, 8192)?;
    if !e.decision && digest(template.as_bytes())? != TEMPLATE_SHA {
        return Err(E::InvalidConfig);
    }
    let size = template.capacity();
    drop(template);
    c.release(size);
    json::free_fields(&mut c, f);
    c.drop_vec(rawcfg);
    c.release(65536);
    let context = manifest
        .fields()
        .configuration
        .context_limit
        .checked_sub(if e.decision {
            0
        } else {
            manifest.fields().configuration.max_new_tokens
        })
        .ok_or(E::InvalidProfile)? as usize;
    Ok(AdmittedData {
        vocab,
        merges,
        added,
        by_id,
        byte_ids,
        tokenizer_sha256: e.tokenizer_sha256,
        config_sha256: e.config_sha256,
        resident: c.resident,
        context_limit: context.min(4096),
        manifest_sha256: expected_manifest,
    })
}

const PRETOKENIZER:&[u8]=br#"{"type":"Sequence","pretokenizers":[{"type":"Split","pattern":{"Regex":"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\\r\\n\\p{L}\\p{N}]?\\p{L}+|\\p{N}| ?[^\\s\\p{L}\\p{N}]+[\\r\\n]*|\\s*[\\r\\n]+|\\s+(?!\\S)|\\s+"},"behavior":"Isolated","invert":false},{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}]}"#;
pub(super) const ADDED: [&str; 26] = [
    "<|endoftext|>",
    "<|im_start|>",
    "<|im_end|>",
    "<|object_ref_start|>",
    "<|object_ref_end|>",
    "<|box_start|>",
    "<|box_end|>",
    "<|quad_start|>",
    "<|quad_end|>",
    "<|vision_start|>",
    "<|vision_end|>",
    "<|vision_pad|>",
    "<|image_pad|>",
    "<|video_pad|>",
    "<tool_call>",
    "</tool_call>",
    "<|fim_prefix|>",
    "<|fim_middle|>",
    "<|fim_suffix|>",
    "<|fim_pad|>",
    "<|repo_name|>",
    "<|file_sep|>",
    "<tool_response>",
    "</tool_response>",
    "<think>",
    "</think>",
];
#[cfg(test)]
pub(super) fn test_expected() -> ExpectedProfile {
    ExpectedProfile {
        tokenizer_size: 6227,
        config_size: 9732,
        tokenizer_sha256: hex(b"248499cd3f9f391e68fb812053ba81c62a69e1cd8d244671df71ee7a37ecfb37"),
        config_sha256: CONFIG_SHA,
        decision: false,
        control: None,
    }
}
#[cfg(test)]
pub(super) fn test_load(t: &[u8], c: &[u8], b: AdmissionBudget) -> Result<AdmittedData, E> {
    admit(
        &mut std::io::Cursor::new(t),
        &mut std::io::Cursor::new(c),
        &super::tests::manifest(),
        b,
        test_expected(),
    )
}
#[cfg(test)]
pub(super) fn test_load_reidentified(
    t: &[u8],
    c: &[u8],
    b: AdmissionBudget,
) -> Result<AdmittedData, E> {
    let mut e = test_expected();
    e.tokenizer_size = t.len();
    e.tokenizer_sha256 = digest(t)?;
    admit(
        &mut std::io::Cursor::new(t),
        &mut std::io::Cursor::new(c),
        &super::tests::manifest(),
        b,
        e,
    )
}
