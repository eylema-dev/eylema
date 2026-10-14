use crate::{
    LoadError, TensorSpec, WeightLayout,
    error::reserved,
    header::{AUX_BYTES, HEADER_BYTES, TensorRange, validate_header},
    layout::MAX_ARTIFACT,
};
use eylema_contracts::model_contract::ModelManifest;
use eylema_platform::{sha256, sha256_stream::Sha256Stream};
use std::io::{ErrorKind, Read};
#[derive(Clone, Copy, Debug)]
pub struct LoadBudget {
    pub max_artifact_bytes: u64,
    pub max_resident_content_bytes: u64,
    pub max_read_calls: u32,
    pub max_interruptions: u32,
}
pub struct LoadedWeights {
    data: Vec<u8>,
    ranges: Vec<TensorRange>,
    payload_start: usize,
    artifact_hash: [u8; 32],
    manifest_hash: [u8; 32],
    layout_hash: [u8; 32],
}
impl std::fmt::Debug for LoadedWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedWeights")
            .field("bytes", &self.data.len())
            .field("tensors", &self.ranges.len())
            .finish_non_exhaustive()
    }
}
impl LoadedWeights {
    fn range(&self, name: &str) -> Option<&TensorRange> {
        if name.is_empty() || name.len() > 128 {
            return None;
        }
        let i = self
            .ranges
            .binary_search_by(|r| r.spec.name().cmp(name))
            .ok()?;
        self.ranges.get(i)
    }
    pub fn tensor_bytes(&self, name: &str) -> Option<&[u8]> {
        let r = self.range(name)?;
        self.data
            .get(self.payload_start + r.start as usize..self.payload_start + r.end as usize)
    }
    pub fn tensor_spec(&self, name: &str) -> Option<&TensorSpec> {
        self.range(name).map(|r| &r.spec)
    }
    pub fn artifact_len(&self) -> u64 {
        self.data.len() as u64
    }
    pub fn artifact_sha256(&self) -> [u8; 32] {
        self.artifact_hash
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_hash
    }
    pub fn layout_sha256(&self) -> [u8; 32] {
        self.layout_hash
    }
}
struct Reader<'a, R> {
    r: &'a mut R,
    calls: u32,
    interruptions: u32,
    budget: LoadBudget,
}
impl<R: Read> Reader<'_, R> {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, LoadError> {
        loop {
            if self.calls >= self.budget.max_read_calls {
                return Err(LoadError::WorkLimit);
            }
            self.calls += 1;
            match self.r.read(b) {
                Ok(n) => {
                    if n > b.len() {
                        return Err(LoadError::Io);
                    }
                    return Ok(n);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    if self.interruptions >= self.budget.max_interruptions {
                        return Err(LoadError::WorkLimit);
                    }
                    self.interruptions += 1
                }
                Err(_) => return Err(LoadError::Io),
            }
        }
    }
    fn exact(&mut self, mut b: &mut [u8]) -> Result<(), LoadError> {
        while !b.is_empty() {
            let request = b.len().min(65536);
            let n = self.read(&mut b[..request])?;
            if n == 0 {
                return Err(LoadError::Io);
            }
            b = &mut b[n..];
        }
        Ok(())
    }
}
pub fn load<R: Read>(
    reader: &mut R,
    manifest: &ModelManifest,
    layout: &WeightLayout,
    budget: LoadBudget,
) -> Result<LoadedWeights, LoadError> {
    if budget.max_artifact_bytes == 0
        || budget.max_read_calls == 0
        || budget.max_read_calls > 65568
        || budget.max_interruptions > 16
        || budget.max_resident_content_bytes > MAX_ARTIFACT + AUX_BYTES as u64
    {
        return Err(LoadError::InvalidProfile);
    }
    if budget.max_artifact_bytes > MAX_ARTIFACT {
        return Err(LoadError::SizeLimit);
    }
    let d = manifest.fields();
    let c = d
        .components
        .iter()
        .find(|c| c.name == d.configuration.weights_component)
        .ok_or(LoadError::InvalidProfile)?;
    if c.size < 8 || c.size > MAX_ARTIFACT || c.size > budget.max_artifact_bytes {
        return Err(LoadError::SizeLimit);
    }
    if c.size
        .checked_add(AUX_BYTES as u64)
        .ok_or(LoadError::ResidentLimit)?
        > budget.max_resident_content_bytes
    {
        return Err(LoadError::ResidentLimit);
    }
    let wire = manifest.to_bytes().map_err(|_| LoadError::InvalidProfile)?;
    if sha256(&wire)? != layout.manifest_hash
        || c.size != layout.artifact_size
        || c.sha256 != layout.artifact_hash
    {
        return Err(LoadError::InvalidProfile);
    }
    let total = usize::try_from(c.size).map_err(|_| LoadError::SizeLimit)?;
    let mut r = Reader {
        r: reader,
        calls: 0,
        interruptions: 0,
        budget,
    };
    let mut prefix = [0; 8];
    r.exact(&mut prefix)?;
    let h = usize::try_from(u64::from_le_bytes(prefix)).map_err(|_| LoadError::SizeLimit)?;
    if h > HEADER_BYTES || h.checked_add(8).is_none_or(|n| n > total) {
        return Err(LoadError::SizeLimit);
    }
    let mut header = reserved(h)?;
    header.resize(h, 0);
    r.exact(&mut header)?;
    let payload_start = 8 + h;
    let ranges = validate_header(&header, (total - payload_start) as u64, layout)?;
    let mut data = reserved(total)?;
    data.extend_from_slice(&prefix);
    data.extend_from_slice(&header);
    let mut hash = Sha256Stream::new(c.size)?;
    hash.update(&prefix)?;
    for b in header.chunks(65536) {
        hash.update(b)?;
    }
    drop(header);
    let mut scratch = [0u8; 65536];
    while data.len() < total {
        let n = (total - data.len()).min(scratch.len());
        r.exact(&mut scratch[..n])?;
        data.extend_from_slice(&scratch[..n]);
        hash.update(&scratch[..n])?;
    }
    if r.read(&mut scratch[..1])? != 0 {
        return Err(LoadError::SizeLimit);
    }
    let actual = hash.finish()?;
    if actual != c.sha256 {
        return Err(LoadError::HashMismatch);
    }
    Ok(LoadedWeights {
        data,
        ranges,
        payload_start,
        artifact_hash: actual,
        manifest_hash: layout.manifest_hash,
        layout_hash: layout.layout_hash,
    })
}
