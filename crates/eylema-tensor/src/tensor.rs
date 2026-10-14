pub(crate) const MAX_ELEMENTS: usize = 4096;
pub(crate) const MAX_WORK: usize = 262144;

pub(crate) fn zeroed(length: usize) -> Result<Vec<f32>, TensorError> {
    if length > MAX_ELEMENTS {
        return Err(TensorError::ExtentLimit);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| TensorError::AllocationFailed)?;
    result.resize(length, 0.);
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorError {
    InvalidShape,
    ExtentOverflow,
    ExtentLimit,
    WorkLimit,
    InvalidParameter,
    IndexOutOfBounds,
    NonFinite,
    FullyMasked,
    UnsupportedLayout,
    UnsupportedOperator,
    SpecMismatch,
    DuplicateOperator,
    AllocationFailed,
}

#[derive(Debug, PartialEq)]
pub struct Tensor {
    shape: Vec<usize>,
    values: Vec<f32>,
}

pub(crate) fn extent(shape: &[usize]) -> Result<usize, TensorError> {
    if shape.is_empty() || shape.len() > 4 || shape.contains(&0) {
        return Err(TensorError::InvalidShape);
    }
    let total = shape.iter().try_fold(1usize, |n, &d| {
        n.checked_mul(d).ok_or(TensorError::ExtentOverflow)
    })?;
    if total > MAX_ELEMENTS {
        return Err(TensorError::ExtentLimit);
    }
    Ok(total)
}

pub(crate) fn finite(value: f32) -> Result<f32, TensorError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(TensorError::NonFinite)
    }
}

pub(crate) fn copied<T: Copy>(values: &[T]) -> Result<Vec<T>, TensorError> {
    if values.len() > MAX_ELEMENTS {
        return Err(TensorError::ExtentLimit);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| TensorError::AllocationFailed)?;
    result.extend_from_slice(values);
    Ok(result)
}

impl Tensor {
    pub fn new(shape: &[usize], values: &[f32]) -> Result<Self, TensorError> {
        if extent(shape)? != values.len() {
            return Err(TensorError::InvalidShape);
        }
        for &value in values {
            finite(value)?;
        }
        Ok(Self {
            shape: copied(shape)?,
            values: copied(values)?,
        })
    }
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    F32RowMajor,
    F16RowMajor,
    BF16RowMajor,
    Quantized,
}

pub fn validate_layout(layout: Layout) -> Result<(), TensorError> {
    match layout {
        Layout::F32RowMajor => Ok(()),
        _ => Err(TensorError::UnsupportedLayout),
    }
}
