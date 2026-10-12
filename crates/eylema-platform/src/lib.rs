//! Narrow platform services. Portable contracts do not implement cryptography.
#![deny(unsafe_op_in_unsafe_fn)]

use eylema_contracts::codec::MAX_WIRE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformError {
    TooLarge,
    Unavailable,
    ServiceFailure,
    InvalidInput,
}

/// Maximum number of elements accepted by the initial bounded Metal proof.
pub const MAX_METAL_ELEMENTS: usize = 4096;

fn validate_metal_inputs(lhs: &[i32], rhs: &[i32]) -> Result<(), PlatformError> {
    if lhs.is_empty() || lhs.len() != rhs.len() {
        return Err(PlatformError::InvalidInput);
    }
    if lhs.len() > MAX_METAL_ELEMENTS {
        return Err(PlatformError::TooLarge);
    }
    if lhs
        .iter()
        .zip(rhs)
        .any(|(left, right)| left.checked_add(*right).is_none())
    {
        return Err(PlatformError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
fn cpu_reference_add(lhs: &[i32], rhs: &[i32]) -> Result<Vec<i32>, PlatformError> {
    validate_metal_inputs(lhs, rhs)?;
    lhs.iter()
        .zip(rhs)
        .map(|(left, right)| left.checked_add(*right).ok_or(PlatformError::InvalidInput))
        .collect()
}

/// Adds equal-length signed integer vectors with the native Metal backend.
/// Empty, mismatched, oversized and overflowing inputs are rejected before
/// any device or buffer allocation. Unsupported targets fail explicitly.
pub fn metal_add_i32(lhs: &[i32], rhs: &[i32]) -> Result<Vec<i32>, PlatformError> {
    validate_metal_inputs(lhs, rhs)?;
    metal::add_i32(lhs, rhs)
}

#[cfg(target_os = "macos")]
mod metal;
#[cfg(not(target_os = "macos"))]
mod metal {
    use super::PlatformError;
    pub(super) fn add_i32(_: &[i32], _: &[i32]) -> Result<Vec<i32>, PlatformError> {
        Err(PlatformError::Unavailable)
    }
}
impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PlatformError {}

/// Lowercase exact-byte representation, not a cryptographic implementation.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum IdKind {
    Task = 0,
    Action = 1,
    Proposal = 2,
    Execution = 3,
    Event = 4,
    Request = 5,
    Correlation = 6,
}
impl IdKind {
    pub fn from_byte(value: u8) -> Result<Self, PlatformError> {
        match value {
            0 => Ok(Self::Task),
            1 => Ok(Self::Action),
            2 => Ok(Self::Proposal),
            3 => Ok(Self::Execution),
            4 => Ok(Self::Event),
            5 => Ok(Self::Request),
            6 => Ok(Self::Correlation),
            _ => Err(PlatformError::InvalidInput),
        }
    }
    fn prefix(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Action => "action",
            Self::Proposal => "proposal",
            Self::Execution => "execution",
            Self::Event => "event",
            Self::Request => "request",
            Self::Correlation => "correlation",
        }
    }
}
/// Formats supplied 128-bit entropy. This function does not generate entropy or
/// authenticate an identity, and does not guarantee global uniqueness.
pub fn id_from_entropy(kind: IdKind, entropy: &[u8; 16]) -> String {
    format!("{}:{}", kind.prefix(), hex(entropy))
}

/// Uses the native OS SHA-256 service; unsupported targets return Unavailable.
pub fn sha256(bytes: &[u8]) -> Result<[u8; 32], PlatformError> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(PlatformError::TooLarge);
    }
    native::sha256(bytes)
}
/// Uses OS entropy without a time/counter/zero fallback.
pub fn random_id(kind: IdKind) -> Result<String, PlatformError> {
    let entropy = native::entropy()?;
    Ok(id_from_entropy(kind, &entropy))
}

/// Requests the macOS drive-cache flush after ordinary file/parent sync.
/// The caller retains the live file handle; unsupported targets fail explicitly.
pub fn full_sync_file(file: &std::fs::File) -> Result<(), PlatformError> {
    native::full_sync_file(file)
}

#[cfg(target_os = "macos")]
mod native {
    use super::PlatformError;
    use std::ffi::{c_int, c_void};
    #[link(name = "System")]
    unsafe extern "C" {
        fn CC_SHA256(data: *const c_void, length: u32, output: *mut u8) -> *mut u8;
        fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    }
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecRandomCopyBytes(random: *const c_void, count: usize, output: *mut c_void) -> c_int;
    }
    pub fn full_sync_file(file: &std::fs::File) -> Result<(), PlatformError> {
        use std::os::fd::AsRawFd;
        // SAFETY: macOS27 SDK fcntl(int,int,...) and F_FULLFSYNC=51. This
        // command has no additional argument. The borrowed File keeps its valid
        // descriptor alive throughout the synchronous OS call; no pointer.
        if unsafe { fcntl(file.as_raw_fd(), 51) } == 0 {
            Ok(())
        } else {
            Err(PlatformError::ServiceFailure)
        }
    }
    pub fn sha256(bytes: &[u8]) -> Result<[u8; 32], PlatformError> {
        let length = u32::try_from(bytes.len()).map_err(|_| PlatformError::TooLarge)?;
        let mut output = [0; 32];
        // SAFETY: SDK CommonDigest ABI: uint32_t CC_LONG, valid input slice for
        // length bytes (including zero), writable 32-byte output. Synchronous
        // call retains neither pointer. Only the OS service implements SHA-256.
        let result = unsafe { CC_SHA256(bytes.as_ptr().cast(), length, output.as_mut_ptr()) };
        if result != output.as_mut_ptr() {
            return Err(PlatformError::ServiceFailure);
        }
        Ok(output)
    }
    fn entropy_result(status: c_int, output: [u8; 16]) -> Result<[u8; 16], PlatformError> {
        if status == 0 {
            Ok(output)
        } else {
            Err(PlatformError::ServiceFailure)
        }
    }
    pub fn entropy() -> Result<[u8; 16], PlatformError> {
        let mut output = [0; 16];
        // SAFETY: SDK SecRandom ABI: null means kSecRandomDefault; count matches
        // the writable array; synchronous call retains no pointer. Nonzero OS
        // status is propagated before the output can become an ID.
        let status = unsafe {
            SecRandomCopyBytes(std::ptr::null(), output.len(), output.as_mut_ptr().cast())
        };
        entropy_result(status, output)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn failed_entropy_never_returns_an_id_source() {
            assert_eq!(
                entropy_result(-1, [0; 16]),
                Err(PlatformError::ServiceFailure)
            );
            assert_eq!(
                entropy_result(7, [42; 16]),
                Err(PlatformError::ServiceFailure)
            );
        }
    }
}
#[cfg(not(target_os = "macos"))]
mod native {
    use super::PlatformError;
    pub fn full_sync_file(_: &std::fs::File) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable)
    }
    pub fn sha256(_: &[u8]) -> Result<[u8; 32], PlatformError> {
        Err(PlatformError::Unavailable)
    }
    pub fn entropy() -> Result<[u8; 16], PlatformError> {
        Err(PlatformError::Unavailable)
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_format_is_bounded_and_validated_by_contracts() {
        let id = id_from_entropy(IdKind::Task, &[0xab; 16]);
        assert_eq!(id, "task:abababababababababababababababab");
        assert!(eylema_contracts::TaskId::new(id).is_ok());
        assert_eq!(IdKind::from_byte(7), Err(PlatformError::InvalidInput));
    }
    #[test]
    fn digest_aggregate_bound_applies_before_platform_call() {
        assert_eq!(
            sha256(&vec![0; MAX_WIRE_BYTES + 1]),
            Err(PlatformError::TooLarge)
        );
    }
    #[test]
    fn metal_inputs_are_bounded_and_checked_before_dispatch() {
        assert_eq!(metal_add_i32(&[], &[]), Err(PlatformError::InvalidInput));
        assert_eq!(metal_add_i32(&[1], &[]), Err(PlatformError::InvalidInput));
        assert_eq!(
            metal_add_i32(&[i32::MAX], &[1]),
            Err(PlatformError::InvalidInput)
        );
        let values = vec![1; MAX_METAL_ELEMENTS + 1];
        assert_eq!(
            metal_add_i32(&values, &values),
            Err(PlatformError::TooLarge)
        );
    }
    #[test]
    fn cpu_reference_uses_the_same_checked_bounded_contract() {
        assert_eq!(cpu_reference_add(&[-4, 7], &[9, -2]), Ok(vec![5, 5]));
        assert_eq!(
            cpu_reference_add(&[i32::MAX], &[1]),
            Err(PlatformError::InvalidInput)
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal-capable device; run explicitly with --ignored"]
    fn actual_metal_worker_adds_and_reads_back_on_hardware() {
        let lhs = [-10, 0, 42, 2_000_000];
        let rhs = [3, 5, -12, 7];
        assert_eq!(
            metal_add_i32(&lhs, &rhs).unwrap(),
            cpu_reference_add(&lhs, &rhs).unwrap()
        );
        let boundary = vec![1_i32; MAX_METAL_ELEMENTS];
        assert_eq!(
            metal_add_i32(&boundary, &boundary).unwrap(),
            cpu_reference_add(&boundary, &boundary).unwrap()
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn actual_os_services_match_sha_vectors_and_supply_ids() {
        assert_eq!(
            hex(&sha256(b"").unwrap()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc").unwrap()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let a = random_id(IdKind::Task).unwrap();
        let b = random_id(IdKind::Task).unwrap();
        assert!(eylema_contracts::TaskId::new(a.clone()).is_ok());
        assert!(eylema_contracts::TaskId::new(b.clone()).is_ok());
        assert_ne!(a, b); // Observed samples, not an absolute uniqueness proof.
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_native_target_is_explicit() {
        assert_eq!(sha256(b"abc"), Err(PlatformError::Unavailable));
        assert_eq!(random_id(IdKind::Task), Err(PlatformError::Unavailable));
    }
}
