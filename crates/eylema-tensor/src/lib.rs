#![forbid(unsafe_code)]

mod identity;
mod tensor;

pub use identity::{OperatorKey, PrimitiveCoverage, reference_coverage};
pub use tensor::{Layout, Tensor, TensorError, validate_layout};
