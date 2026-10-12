//! Narrow, synchronous native Metal proof using only the Apple Objective-C ABI.
use super::{MAX_METAL_ELEMENTS, PlatformError};
use std::ffi::{CString, c_char, c_void};
use std::ptr::NonNull;

type Id = *mut c_void;
type Sel = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct MtlSize {
    width: usize,
    height: usize,
    depth: usize,
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_release(object: Id);
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

struct AutoreleasePool(*mut c_void);

impl AutoreleasePool {
    fn new() -> Self {
        // SAFETY: Objective-C runtime creates a thread-local autorelease pool;
        // the matching pop is called exactly once by Drop on this thread.
        Self(unsafe { objc_autoreleasePoolPush() })
    }
}

impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        // SAFETY: this token came from objc_autoreleasePoolPush and has not
        // previously been popped.
        unsafe { objc_autoreleasePoolPop(self.0) }
    }
}

struct OwnedObject {
    pointer: NonNull<c_void>,
    release: unsafe extern "C" fn(Id),
}

impl OwnedObject {
    fn from_owned(pointer: Id) -> Option<Self> {
        Some(Self {
            pointer: NonNull::new(pointer)?,
            release: objc_release,
        })
    }

    fn as_id(&self) -> Id {
        self.pointer.as_ptr()
    }
}

impl Drop for OwnedObject {
    fn drop(&mut self) {
        // SAFETY: all production handles are +1 objects returned by Metal's
        // new/create APIs. The test handle uses its matching injected releaser.
        unsafe { (self.release)(self.pointer.as_ptr()) }
    }
}

fn selector(name: &'static std::ffi::CStr) -> Sel {
    // SAFETY: selector names are static NUL-terminated C strings.
    unsafe { sel_registerName(name.as_ptr()) }
}

unsafe fn send_id0(receiver: Id, name: &'static std::ffi::CStr) -> Id {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
        // SAFETY: objc_msgSend uses the platform C ABI; this call site binds
        // the exact two-pointer selector signature of an object getter.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: caller supplies a live object and this selector returns an id.
    unsafe { send(receiver, selector(name)) }
}

unsafe fn send_id1(receiver: Id, name: &'static std::ffi::CStr, value: Id) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        // SAFETY: this call site binds the Objective-C one-object-argument ABI.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver and value are live Objective-C objects for this call.
    unsafe { send(receiver, selector(name), value) }
}

unsafe fn send_string(receiver: Id, value: *const c_char) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
        // SAFETY: NSString's stringWithUTF8String: takes one C string pointer.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: value is a live NUL-terminated CString for the duration of call.
    unsafe { send(receiver, selector(c"stringWithUTF8String:"), value) }
}

unsafe fn send_library(receiver: Id, source: Id, error: *mut Id) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, Id, Id, *mut Id) -> Id =
        // SAFETY: matches newLibraryWithSource:options:error: in the macOS SDK.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: source is a live NSString; nil options and writable error slot
    // match the declared Metal method. The method is synchronous.
    unsafe {
        send(
            receiver,
            selector(c"newLibraryWithSource:options:error:"),
            source,
            std::ptr::null_mut(),
            error,
        )
    }
}

unsafe fn send_pipeline(receiver: Id, function: Id, error: *mut Id) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, Id, *mut Id) -> Id =
        // SAFETY: matches newComputePipelineStateWithFunction:error:.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: function is a live MTLFunction and error is a writable NSError* slot.
    unsafe {
        send(
            receiver,
            selector(c"newComputePipelineStateWithFunction:error:"),
            function,
            error,
        )
    }
}

unsafe fn send_buffer_with_bytes(receiver: Id, bytes: *const c_void, length: usize) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, *const c_void, usize, usize) -> Id =
        // SAFETY: matches newBufferWithBytes:length:options: (NSUInteger is usize).
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: bytes points to length readable bytes; shared storage options are 0.
    unsafe {
        send(
            receiver,
            selector(c"newBufferWithBytes:length:options:"),
            bytes,
            length,
            0,
        )
    }
}

unsafe fn send_buffer_with_length(receiver: Id, length: usize) -> Id {
    let send: unsafe extern "C" fn(Id, Sel, usize, usize) -> Id =
        // SAFETY: matches newBufferWithLength:options: (NSUInteger is usize).
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: the requested bounded allocation has shared storage options 0.
    unsafe {
        send(
            receiver,
            selector(c"newBufferWithLength:options:"),
            length,
            0,
        )
    }
}

unsafe fn send_usize0(receiver: Id, name: &'static std::ffi::CStr) -> usize {
    let send: unsafe extern "C" fn(Id, Sel) -> usize =
        // SAFETY: NSUInteger-valued Metal property getter.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver is a live Metal object and selector returns NSUInteger.
    unsafe { send(receiver, selector(name)) }
}

unsafe fn send_pointer0(receiver: Id, name: &'static std::ffi::CStr) -> *mut c_void {
    let send: unsafe extern "C" fn(Id, Sel) -> *mut c_void =
        // SAFETY: MTLBuffer contents returns a data pointer.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver is a live MTLBuffer.
    unsafe { send(receiver, selector(name)) }
}

unsafe fn send_void1(receiver: Id, name: &'static std::ffi::CStr, value: Id) {
    let send: unsafe extern "C" fn(Id, Sel, Id) =
        // SAFETY: void selector with one Objective-C object argument.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver and value are live objects.
    unsafe { send(receiver, selector(name), value) }
}

unsafe fn send_set_buffer(receiver: Id, buffer: Id, index: usize) {
    let send: unsafe extern "C" fn(Id, Sel, Id, usize, usize) =
        // SAFETY: matches setBuffer:offset:atIndex:.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: buffer is live; zero offset and bounded index match encoded args.
    unsafe {
        send(
            receiver,
            selector(c"setBuffer:offset:atIndex:"),
            buffer,
            0,
            index,
        )
    }
}

unsafe fn send_dispatch(receiver: Id, grid: MtlSize, group: MtlSize) {
    let send: unsafe extern "C" fn(Id, Sel, MtlSize, MtlSize) =
        // SAFETY: matches dispatchThreads:threadsPerThreadgroup: with MTLSize
        // defined by three NSUInteger fields in the macOS Metal SDK.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver is a live compute encoder and the grids are nonzero.
    unsafe {
        send(
            receiver,
            selector(c"dispatchThreads:threadsPerThreadgroup:"),
            grid,
            group,
        )
    }
}

unsafe fn send_void0(receiver: Id, name: &'static std::ffi::CStr) {
    let send: unsafe extern "C" fn(Id, Sel) =
        // SAFETY: void selector with no explicit arguments.
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: receiver remains live through this synchronous method call.
    unsafe { send(receiver, selector(name)) }
}

fn command_buffer_result(status: usize) -> Result<(), PlatformError> {
    if status == 4 {
        Ok(())
    } else {
        Err(PlatformError::ServiceFailure)
    }
}

pub(super) fn add_i32(lhs: &[i32], rhs: &[i32]) -> Result<Vec<i32>, PlatformError> {
    let _pool = AutoreleasePool::new();
    let device = OwnedObject::from_owned(
        // SAFETY: Metal returns a retained system-default device or nil.
        unsafe { MTLCreateSystemDefaultDevice() },
    )
    .ok_or(PlatformError::Unavailable)?;

    let queue = OwnedObject::from_owned(
        // SAFETY: newCommandQueue follows ObjC's +1 ownership rule.
        unsafe { send_id0(device.as_id(), c"newCommandQueue") },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let source = CString::new(include_str!("../../../shaders/metal/add_i32.metal"))
        .map_err(|_| PlatformError::ServiceFailure)?;
    let source_string =
        // SAFETY: NSString copies valid UTF-8 from a live NUL-terminated string.
        unsafe { send_string(objc_get_class(c"NSString"), source.as_ptr()) };
    if source_string.is_null() {
        return Err(PlatformError::ServiceFailure);
    }
    let mut error = std::ptr::null_mut();
    let library = OwnedObject::from_owned(
        // SAFETY: synchronous compile from the embedded trusted shader source.
        unsafe { send_library(device.as_id(), source_string, &mut error) },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let function = OwnedObject::from_owned(
        // SAFETY: function lookup returns a retained function or nil.
        unsafe {
            send_id1(
                library.as_id(),
                c"newFunctionWithName:",
                objc_string("add_i32")?,
            )
        },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let pipeline = OwnedObject::from_owned(
        // SAFETY: pipeline factory follows the +1 new-method ownership rule.
        unsafe { send_pipeline(device.as_id(), function.as_id(), &mut error) },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let width =
        // SAFETY: live pipeline and NSUInteger-valued property.
        unsafe { send_usize0(pipeline.as_id(), c"threadExecutionWidth") };
    if width == 0 {
        return Err(PlatformError::ServiceFailure);
    }

    let byte_length = lhs
        .len()
        .checked_mul(std::mem::size_of::<i32>())
        .filter(|length| *length <= MAX_METAL_ELEMENTS * std::mem::size_of::<i32>())
        .ok_or(PlatformError::TooLarge)?;
    let lhs_buffer = OwnedObject::from_owned(
        // SAFETY: slice pointer is valid for the checked byte length; Metal
        // copies input bytes before returning and the slice remains alive.
        unsafe { send_buffer_with_bytes(device.as_id(), lhs.as_ptr().cast(), byte_length) },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let rhs_buffer = OwnedObject::from_owned(
        // SAFETY: same bound and lifetime as the left input buffer.
        unsafe { send_buffer_with_bytes(device.as_id(), rhs.as_ptr().cast(), byte_length) },
    )
    .ok_or(PlatformError::ServiceFailure)?;
    let result_buffer = OwnedObject::from_owned(
        // SAFETY: bounded nonzero output allocation with shared storage.
        unsafe { send_buffer_with_length(device.as_id(), byte_length) },
    )
    .ok_or(PlatformError::ServiceFailure)?;

    // commandBuffer and computeCommandEncoder are autoreleased; _pool keeps
    // them alive until after synchronous completion and readback below.
    let command_buffer =
        // SAFETY: live queue and object getter per MTLCommandQueue contract.
        unsafe { send_id0(queue.as_id(), c"commandBuffer") };
    if command_buffer.is_null() {
        return Err(PlatformError::ServiceFailure);
    }
    let encoder =
        // SAFETY: command buffer is live in the current autorelease pool.
        unsafe { send_id0(command_buffer, c"computeCommandEncoder") };
    if encoder.is_null() {
        return Err(PlatformError::ServiceFailure);
    }
    // SAFETY: all bound objects remain live until command completion.
    unsafe {
        send_void1(encoder, c"setComputePipelineState:", pipeline.as_id());
        send_set_buffer(encoder, lhs_buffer.as_id(), 0);
        send_set_buffer(encoder, rhs_buffer.as_id(), 1);
        send_set_buffer(encoder, result_buffer.as_id(), 2);
        send_dispatch(
            encoder,
            MtlSize {
                width: lhs.len(),
                height: 1,
                depth: 1,
            },
            MtlSize {
                width: width.min(lhs.len()),
                height: 1,
                depth: 1,
            },
        );
        send_void0(encoder, c"endEncoding");
        send_void0(command_buffer, c"commit");
        send_void0(command_buffer, c"waitUntilCompleted");
    }
    let status =
        // SAFETY: live command buffer; status is NSUInteger per SDK enum.
        unsafe { send_usize0(command_buffer, c"status") };
    command_buffer_result(status)?;
    let output =
        // SAFETY: MTLBuffer owns this pointer through completed command buffer.
        unsafe { send_pointer0(result_buffer.as_id(), c"contents") };
    if output.is_null() {
        return Err(PlatformError::ServiceFailure);
    }
    let mut result = vec![0_i32; lhs.len()];
    // SAFETY: successful shared-storage buffer is byte_length bytes, has
    // completed GPU writes, and result is an equally sized initialized slice.
    unsafe {
        std::ptr::copy_nonoverlapping(output.cast::<i32>(), result.as_mut_ptr(), result.len())
    };
    Ok(result)
}

fn objc_get_class(name: &'static std::ffi::CStr) -> Id {
    // SAFETY: NSString is a static Objective-C runtime class name.
    unsafe { objc_getClass(name.as_ptr()) }
}

fn objc_string(value: &'static str) -> Result<Id, PlatformError> {
    let string = CString::new(value).map_err(|_| PlatformError::InvalidInput)?;
    // SAFETY: NSString copies the NUL-terminated UTF-8 before the CString drops.
    let result = unsafe { send_string(objc_get_class(c"NSString"), string.as_ptr()) };
    if result.is_null() {
        Err(PlatformError::ServiceFailure)
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static RELEASES: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn fake_release(pointer: Id) {
        // SAFETY: test constructs exactly one Box<u8> and transfers it to handle.
        unsafe { drop(Box::from_raw(pointer.cast::<u8>())) };
        RELEASES.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn non_completed_worker_status_is_a_stable_failure() {
        assert_eq!(command_buffer_result(4), Ok(()));
        for status in [0, 1, 2, 3, 5, usize::MAX] {
            assert_eq!(
                command_buffer_result(status),
                Err(PlatformError::ServiceFailure)
            );
        }
    }

    #[test]
    fn retained_resource_is_released_once_when_scope_exits() {
        RELEASES.store(0, Ordering::SeqCst);
        {
            let pointer = NonNull::new(Box::into_raw(Box::new(7_u8)).cast::<c_void>()).unwrap();
            let _resource = OwnedObject {
                pointer,
                release: fake_release,
            };
        }
        assert_eq!(RELEASES.load(Ordering::SeqCst), 1);
    }

    fn simulated_worker_failure() -> Result<(), PlatformError> {
        let left = NonNull::new(Box::into_raw(Box::new(1_u8)).cast::<c_void>()).unwrap();
        let right = NonNull::new(Box::into_raw(Box::new(2_u8)).cast::<c_void>()).unwrap();
        let _left = OwnedObject {
            pointer: left,
            release: fake_release,
        };
        let _right = OwnedObject {
            pointer: right,
            release: fake_release,
        };
        Err(PlatformError::ServiceFailure)
    }

    #[test]
    fn every_retained_resource_is_released_after_worker_failure() {
        RELEASES.store(0, Ordering::SeqCst);
        assert_eq!(
            simulated_worker_failure(),
            Err(PlatformError::ServiceFailure)
        );
        assert_eq!(RELEASES.load(Ordering::SeqCst), 2);
    }
}
