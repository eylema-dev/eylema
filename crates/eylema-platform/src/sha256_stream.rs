//! Bounded SHA256 lifecycle over the same native provider as the wire hash.
use crate::PlatformError;

const MAX_BYTES: u64 = 2_147_483_648;
const MAX_CHUNK: usize = 1_048_576;

pub struct Sha256Stream {
    context: native::Context,
    limit: u64,
    seen: u64,
    poisoned: bool,
}
impl std::fmt::Debug for Sha256Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sha256Stream")
            .field("bytes", &self.seen)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}
impl Sha256Stream {
    pub fn new(byte_limit: u64) -> Result<Self, PlatformError> {
        if byte_limit == 0 {
            return Err(PlatformError::InvalidInput);
        }
        if byte_limit > MAX_BYTES {
            return Err(PlatformError::TooLarge);
        }
        Ok(Self {
            context: native::init()?,
            limit: byte_limit,
            seen: 0,
            poisoned: false,
        })
    }
    pub fn update(&mut self, chunk: &[u8]) -> Result<(), PlatformError> {
        if self.poisoned {
            return Err(PlatformError::ServiceFailure);
        }
        let next = self.seen.checked_add(chunk.len() as u64);
        if chunk.len() > MAX_CHUNK || next.is_none_or(|v| v > self.limit) {
            self.poisoned = true;
            return Err(PlatformError::TooLarge);
        }
        if chunk.is_empty() {
            return Ok(());
        }
        let result = native::update(&mut self.context, chunk);
        self.apply_update(result, next.unwrap())
    }
    fn apply_update(
        &mut self,
        result: Result<(), PlatformError>,
        next: u64,
    ) -> Result<(), PlatformError> {
        match result {
            Ok(()) => {
                self.seen = next;
                Ok(())
            }
            Err(e) => {
                self.poisoned = true;
                Err(e)
            }
        }
    }
    pub fn finish(mut self) -> Result<[u8; 32], PlatformError> {
        if self.poisoned {
            return Err(PlatformError::ServiceFailure);
        }
        native::finish(&mut self.context)
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::PlatformError;
    use std::ffi::{c_int, c_void};
    // CommonDigest.h macosx27.0: CC_LONG=u32; no pointers in this state.
    #[repr(C)]
    pub(super) struct Context {
        count: [u32; 2],
        hash: [u32; 8],
        wbuf: [u32; 16],
    }
    #[link(name = "System")]
    unsafe extern "C" {
        fn CC_SHA256_Init(c: *mut Context) -> c_int;
        fn CC_SHA256_Update(c: *mut Context, data: *const c_void, len: u32) -> c_int;
        fn CC_SHA256_Final(md: *mut u8, c: *mut Context) -> c_int;
    }
    fn status(rc: c_int) -> Result<(), PlatformError> {
        if rc == 1 {
            Ok(())
        } else {
            Err(PlatformError::ServiceFailure)
        }
    }
    pub(super) fn init() -> Result<Context, PlatformError> {
        let mut c = Context {
            count: [0; 2],
            hash: [0; 8],
            wbuf: [0; 16],
        };
        // SAFETY: installed SDK ABI independently checked by C assertions:
        // writable104byte/align4 context, synchronous call, no retained pointer.
        status(unsafe { CC_SHA256_Init(&mut c) })?;
        Ok(c)
    }
    pub(super) fn update(c: &mut Context, bytes: &[u8]) -> Result<(), PlatformError> {
        let len = u32::try_from(bytes.len()).map_err(|_| PlatformError::TooLarge)?;
        // SAFETY: live exclusively borrowed initialized context; valid input
        // slice for checked CC_LONG length; synchronous provider retains none.
        status(unsafe { CC_SHA256_Update(c, bytes.as_ptr().cast(), len) })
    }
    pub(super) fn finish(c: &mut Context) -> Result<[u8; 32], PlatformError> {
        let mut out = [0; 32];
        // SAFETY: exclusive live context and writable32byte output; call is
        // synchronous. Safe public finish consumes state and prevents reuse.
        status(unsafe { CC_SHA256_Final(out.as_mut_ptr(), c) })?;
        Ok(out)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn sdk_layout_and_failed_status() {
            assert_eq!(std::mem::size_of::<Context>(), 104);
            assert_eq!(std::mem::align_of::<Context>(), 4);
            assert_eq!(std::mem::offset_of!(Context, count), 0);
            assert_eq!(std::mem::offset_of!(Context, hash), 8);
            assert_eq!(std::mem::offset_of!(Context, wbuf), 40);
            assert_eq!(status(0), Err(PlatformError::ServiceFailure));
            assert_eq!(status(-1), Err(PlatformError::ServiceFailure));
        }
    }
}
#[cfg(not(target_os = "macos"))]
mod native {
    use super::PlatformError;
    pub(super) struct Context;
    pub(super) fn init() -> Result<Context, PlatformError> {
        Err(PlatformError::Unavailable)
    }
    pub(super) fn update(_: &mut Context, _: &[u8]) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable)
    }
    pub(super) fn finish(_: &mut Context) -> Result<[u8; 32], PlatformError> {
        Err(PlatformError::Unavailable)
    }
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn service_error_and_overflow_poison_state() {
        let mut h = Sha256Stream::new(8).unwrap();
        assert_eq!(
            h.apply_update(Err(PlatformError::ServiceFailure), 1),
            Err(PlatformError::ServiceFailure)
        );
        assert_eq!(h.update(b"x"), Err(PlatformError::ServiceFailure));
        assert_eq!(h.finish(), Err(PlatformError::ServiceFailure));
        let mut h = Sha256Stream::new(8).unwrap();
        h.seen = u64::MAX;
        assert_eq!(h.update(b"x"), Err(PlatformError::TooLarge));
        assert_eq!(h.finish(), Err(PlatformError::ServiceFailure));
    }
}
