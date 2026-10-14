use crate::tensor::{MAX_WORK, copied, extent, finite, zeroed};
use crate::{Tensor, TensorError};

fn rank2(tensor: &Tensor) -> Result<(usize, usize), TensorError> {
    if tensor.shape().len() != 2 {
        return Err(TensorError::InvalidShape);
    }
    Ok((tensor.shape()[0], tensor.shape()[1]))
}

fn work(rows: usize, inner: usize, out: usize) -> Result<usize, TensorError> {
    rows.checked_mul(inner)
        .and_then(|n| n.checked_mul(out))
        .ok_or(TensorError::ExtentOverflow)
}

pub fn embedding(table: &Tensor, tokens: &[u32]) -> Result<Tensor, TensorError> {
    let (vocabulary, width) = rank2(table)?;
    if tokens.is_empty() {
        return Err(TensorError::InvalidShape);
    }
    if vocabulary > 64 || width > 16 || tokens.len() > 16 {
        return Err(TensorError::ExtentLimit);
    }
    for &id in tokens {
        if usize::try_from(id).map_err(|_| TensorError::IndexOutOfBounds)? >= vocabulary {
            return Err(TensorError::IndexOutOfBounds);
        }
    }
    let mut result = zeroed(extent(&[tokens.len(), width])?)?;
    for (row, &id) in tokens.iter().enumerate() {
        let source = id as usize * width;
        result[row * width..(row + 1) * width]
            .copy_from_slice(&table.values()[source..source + width]);
    }
    Tensor::new(&[tokens.len(), width], &result)
}

pub fn linear(input: &Tensor, weight_out_in: &Tensor) -> Result<Tensor, TensorError> {
    let (rows, inner) = rank2(input)?;
    let (out, weight_inner) = rank2(weight_out_in)?;
    if inner != weight_inner {
        return Err(TensorError::InvalidShape);
    }
    let count = extent(&[rows, out])?;
    if work(rows, inner, out)? > MAX_WORK {
        return Err(TensorError::WorkLimit);
    }
    let mut result = zeroed(count)?;
    for row in 0..rows {
        for column in 0..out {
            let mut sum = 0_f32;
            for k in 0..inner {
                let product = finite(
                    input.values()[row * inner + k] * weight_out_in.values()[column * inner + k],
                )?;
                sum = finite(sum + product)?;
            }
            result[row * out + column] = sum;
        }
    }
    Tensor::new(&[rows, out], &result)
}

pub fn tied_logits(hidden: &Tensor, embedding_table: &Tensor) -> Result<Tensor, TensorError> {
    let (vocabulary, width) = rank2(embedding_table)?;
    if vocabulary > 64 || width > 16 {
        return Err(TensorError::ExtentLimit);
    }
    linear(hidden, embedding_table)
}

pub fn rms_norm(input: &Tensor, scale: &Tensor, epsilon: f32) -> Result<Tensor, TensorError> {
    if !epsilon.is_finite() || epsilon <= 0. {
        return Err(TensorError::InvalidParameter);
    }
    let width = *input.shape().last().ok_or(TensorError::InvalidShape)?;
    if scale.shape() != [width] {
        return Err(TensorError::InvalidShape);
    }
    let mut result = zeroed(input.values().len())?;
    for (source, target) in input
        .values()
        .chunks_exact(width)
        .zip(result.chunks_exact_mut(width))
    {
        let mut sum = 0_f32;
        for &value in source {
            sum = finite(sum + finite(value * value)?)?;
        }
        let mean = finite(sum / width as f32)?;
        let root = finite(finite(mean + epsilon)?.sqrt())?;
        let inverse = finite(1_f32 / root)?;
        for i in 0..width {
            target[i] = finite(finite(source[i] * inverse)? * scale.values()[i])?;
        }
    }
    Tensor::new(input.shape(), &result)
}

pub fn silu(input: &Tensor) -> Result<Tensor, TensorError> {
    let mut result = zeroed(input.values().len())?;
    for (target, &value) in result.iter_mut().zip(input.values()) {
        *target = if value >= 0. {
            finite(value / finite(1. + finite((-value).exp())?)?)?
        } else {
            let exp = finite(value.exp())?;
            finite(finite(value * exp)? / finite(1. + exp)?)?
        };
    }
    Tensor::new(input.shape(), &result)
}

pub fn swiglu(
    input: &Tensor,
    gate: &Tensor,
    up: &Tensor,
    down: &Tensor,
) -> Result<Tensor, TensorError> {
    let (rows, inner) = rank2(input)?;
    let (hidden, gate_inner) = rank2(gate)?;
    let (up_hidden, up_inner) = rank2(up)?;
    let (out, down_inner) = rank2(down)?;
    if inner != gate_inner || inner != up_inner || hidden != up_hidden || hidden != down_inner {
        return Err(TensorError::InvalidShape);
    }
    extent(&[rows, hidden])?;
    extent(&[rows, out])?;
    let total_work = work(rows, inner, hidden)?
        .checked_mul(2)
        .and_then(|n| n.checked_add(work(rows, hidden, out).ok()?))
        .ok_or(TensorError::ExtentOverflow)?;
    if total_work > MAX_WORK {
        return Err(TensorError::WorkLimit);
    }
    let activated = silu(&linear(input, gate)?)?;
    let projected = linear(input, up)?;
    let mut product = zeroed(activated.values().len())?;
    for (i, value) in product.iter_mut().enumerate() {
        *value = finite(activated.values()[i] * projected.values()[i])?;
    }
    linear(&Tensor::new(&[rows, hidden], &product)?, down)
}

pub fn residual_add(input: &Tensor, residual: &Tensor) -> Result<Tensor, TensorError> {
    if input.shape() != residual.shape() {
        return Err(TensorError::InvalidShape);
    }
    let mut result = zeroed(input.values().len())?;
    for (i, value) in result.iter_mut().enumerate() {
        *value = finite(input.values()[i] + residual.values()[i])?;
    }
    Tensor::new(input.shape(), &result)
}

pub fn reshape(input: &Tensor, shape: &[usize]) -> Result<Tensor, TensorError> {
    if extent(shape)? != input.values().len() {
        return Err(TensorError::InvalidShape);
    }
    Tensor::new(shape, input.values())
}

pub fn transpose(input: &Tensor, permutation: &[usize]) -> Result<Tensor, TensorError> {
    let rank = input.shape().len();
    if permutation.len() != rank {
        return Err(TensorError::InvalidParameter);
    }
    let mut shape = copied(input.shape())?;
    for (axis, &source_axis) in permutation.iter().enumerate() {
        if source_axis >= rank || permutation[..axis].contains(&source_axis) {
            return Err(TensorError::InvalidParameter);
        }
        shape[axis] = input.shape()[source_axis];
    }
    let mut result = zeroed(input.values().len())?;
    for (flat, value) in result.iter_mut().enumerate() {
        let mut remainder = flat;
        let mut source_coordinates = [0usize; 4];
        for axis in (0..rank).rev() {
            source_coordinates[permutation[axis]] = remainder % shape[axis];
            remainder /= shape[axis];
        }
        let mut source_flat = 0;
        for (&coordinate, &dim) in source_coordinates[..rank].iter().zip(input.shape()) {
            source_flat = source_flat * dim + coordinate;
        }
        *value = input.values()[source_flat];
    }
    Tensor::new(&shape, &result)
}
