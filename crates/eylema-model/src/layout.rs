use crate::{
    LoadError,
    error::{copied, reserved},
};
use eylema_contracts::model_contract::ModelManifest;
use eylema_platform::sha256;
pub(crate) const MAX_ARTIFACT: u64 = 2_147_483_648;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredDtype {
    BF16,
    F32,
}
impl StoredDtype {
    pub(crate) fn width(self) -> u64 {
        if self == Self::BF16 { 2 } else { 4 }
    }
    pub(crate) fn tag(self) -> u8 {
        if self == Self::BF16 { 1 } else { 2 }
    }
}
pub struct TensorSpec {
    name: String,
    dtype: StoredDtype,
    shape: Vec<u64>,
    bytes: u64,
}
impl std::fmt::Debug for TensorSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TensorSpec")
            .field("dtype", &self.dtype)
            .field("rank", &self.shape.len())
            .finish_non_exhaustive()
    }
}
impl TensorSpec {
    pub fn new(name: &str, dtype: StoredDtype, shape: &[u64]) -> Result<Self, LoadError> {
        if name.is_empty()
            || name.len() > 128
            || !name.as_bytes()[0].is_ascii_alphanumeric()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || shape.is_empty()
            || shape.len() > 4
            || shape.contains(&0)
        {
            return Err(LoadError::InvalidLayout);
        }
        let mut n = dtype.width();
        for &d in shape {
            n = n.checked_mul(d).ok_or(LoadError::SizeLimit)?;
        }
        if n > MAX_ARTIFACT {
            return Err(LoadError::SizeLimit);
        }
        let mut dims = reserved(shape.len())?;
        dims.extend_from_slice(shape);
        Ok(Self {
            name: copied(name)?,
            dtype,
            shape: dims,
            bytes: n,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn dtype(&self) -> StoredDtype {
        self.dtype
    }
    pub fn shape(&self) -> &[u64] {
        &self.shape
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }
    pub(crate) fn content_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.capacity() + self.shape.capacity() * 8
    }
}
pub struct WeightLayout {
    pub(crate) tensors: Vec<TensorSpec>,
    pub(crate) manifest_hash: [u8; 32],
    pub(crate) layout_hash: [u8; 32],
    pub(crate) artifact_hash: [u8; 32],
    pub(crate) artifact_size: u64,
}
impl std::fmt::Debug for WeightLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeightLayout")
            .field("tensors", &self.tensors.len())
            .finish_non_exhaustive()
    }
}
impl WeightLayout {
    pub fn new(manifest: &ModelManifest, tensors: &[TensorSpec]) -> Result<Self, LoadError> {
        let d = manifest.fields();
        let dtype = match d.numeric_format.as_str() {
            "bfloat16" => StoredDtype::BF16,
            "float32" => StoredDtype::F32,
            _ => return Err(LoadError::InvalidProfile),
        };
        if d.artifact_format != "safetensors.v1"
            || d.quantization != "none"
            || tensors.is_empty()
            || tensors.len() > 1024
        {
            return Err(LoadError::InvalidProfile);
        }
        let c = d
            .components
            .iter()
            .find(|c| c.name == d.configuration.weights_component)
            .ok_or(LoadError::InvalidProfile)?;
        if c.size > MAX_ARTIFACT {
            return Err(LoadError::SizeLimit);
        }
        let wire = manifest.to_bytes().map_err(|_| LoadError::InvalidProfile)?;
        let manifest_hash = sha256(&wire)?;
        let mut own = reserved(tensors.len())?;
        for t in tensors {
            if t.dtype != dtype {
                return Err(LoadError::InvalidLayout);
            }
            own.push(TensorSpec::new(t.name(), t.dtype(), t.shape())?)
        }
        own.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        if own.windows(2).any(|w| w[0].name == w[1].name) {
            return Err(LoadError::InvalidLayout);
        }
        let mut bytes = reserved(
            17 + 32
                + 4
                + own
                    .iter()
                    .map(|t| 4 + t.name.len() + 1 + 4 + t.shape.len() * 8)
                    .sum::<usize>(),
        )?;
        bytes.extend_from_slice(b"weight-layout.v1\0");
        bytes.extend_from_slice(&manifest_hash);
        bytes.extend_from_slice(&(own.len() as u32).to_be_bytes());
        for t in &own {
            bytes.extend_from_slice(&(t.name.len() as u32).to_be_bytes());
            bytes.extend_from_slice(t.name.as_bytes());
            bytes.push(t.dtype.tag());
            bytes.extend_from_slice(&(t.shape.len() as u32).to_be_bytes());
            for d in &t.shape {
                bytes.extend_from_slice(&d.to_be_bytes())
            }
        }
        Ok(Self {
            tensors: own,
            manifest_hash,
            layout_hash: sha256(&bytes)?,
            artifact_hash: c.sha256,
            artifact_size: c.size,
        })
    }
    pub fn tensors(&self) -> &[TensorSpec] {
        &self.tensors
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_hash
    }
    pub fn layout_sha256(&self) -> [u8; 32] {
        self.layout_hash
    }
    pub(crate) fn content_bytes(&self) -> usize {
        self.tensors.iter().map(TensorSpec::content_bytes).sum()
    }
}
