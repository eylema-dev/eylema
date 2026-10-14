//! Minimal local device-owner proof. No request field can construct a proof.
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HumanError {
    InvalidInput,
    Unavailable,
    Denied,
    Timeout,
    Expired,
    ServiceFailure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HumanPurpose {
    LocalCli,
    AcceptanceProof,
}
impl HumanPurpose {
    pub fn resource(self) -> &'static str {
        match self {
            Self::LocalCli => "state:cli",
            Self::AcceptanceProof => "state:g73-proof",
        }
    }
    fn reason(self) -> &'static std::ffi::CStr {
        match self {
            Self::LocalCli => c"create a 5-minute Eylema stop session for local state:cli tasks",
            Self::AcceptanceProof => {
                c"approve and stop the bounded Eylema state:g73-proof counter demonstration"
            }
        }
    }
}
pub struct DeviceOwnerProof {
    binding: [u8; 32],
    verified_at: Instant,
}
impl std::fmt::Debug for DeviceOwnerProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceOwnerProof(<opaque>)")
    }
}
impl DeviceOwnerProof {
    /// Consumes the proof. This returns its challenge, never a constructor.
    pub fn consume_binding(self, max_age: Duration) -> Result<[u8; 32], HumanError> {
        if max_age.is_zero() || max_age > Duration::from_secs(15) {
            return Err(HumanError::InvalidInput);
        }
        if Instant::now()
            .checked_duration_since(self.verified_at)
            .is_none_or(|age| age >= max_age)
        {
            return Err(HumanError::Expired);
        }
        Ok(self.binding)
    }
}
#[derive(Clone, Copy)]
enum Reply {
    Pending,
    Complete(bool, Instant),
    Closed,
}
struct Completion {
    state: Mutex<Reply>,
    wake: Condvar,
    deadline: Instant,
}
impl Completion {
    fn new(deadline: Instant) -> Self {
        Self {
            state: Mutex::new(Reply::Pending),
            wake: Condvar::new(),
            deadline,
        }
    }
    fn complete_at(&self, success: bool, at: Instant) {
        if let Ok(mut reply) = self.state.lock() {
            if matches!(*reply, Reply::Pending) {
                *reply = if at >= self.deadline {
                    Reply::Closed
                } else {
                    Reply::Complete(success, at)
                };
            }
            self.wake.notify_all();
        }
    }
    fn wait(&self) -> Result<Instant, HumanError> {
        let mut reply = self.state.lock().map_err(|_| HumanError::ServiceFailure)?;
        loop {
            let now = Instant::now();
            if now >= self.deadline {
                *reply = Reply::Closed;
                return Err(HumanError::Timeout);
            }
            match *reply {
                Reply::Complete(true, at) => return Ok(at),
                Reply::Complete(false, _) => return Err(HumanError::Denied),
                Reply::Closed => return Err(HumanError::Timeout),
                Reply::Pending => {}
            }
            let result = self
                .wake
                .wait_timeout(reply, self.deadline.duration_since(now))
                .map_err(|_| HumanError::ServiceFailure)?;
            reply = result.0;
        }
    }
}
/// Noninteractive capability observation only. Never issues a proof.
pub fn device_owner_available() -> Result<bool, HumanError> {
    native::available()
}
pub fn authenticate_device_owner(
    binding: [u8; 32],
    purpose: HumanPurpose,
    timeout: Duration,
) -> Result<DeviceOwnerProof, HumanError> {
    if binding == [0; 32] || timeout.is_zero() || timeout > Duration::from_secs(60) {
        return Err(HumanError::InvalidInput);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(HumanError::InvalidInput)?;
    let verified_at = native::authenticate(purpose, deadline)?;
    Ok(DeviceOwnerProof {
        binding,
        verified_at,
    })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native {
    use super::*;
    use std::ffi::{CStr, c_char, c_void};
    type Object = *mut c_void;
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Object;
        fn objc_msgSend();
    }
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "LocalAuthentication", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "System")]
    unsafe extern "C" {
        static _NSConcreteStackBlock: [usize; 32];
        fn _Block_copy(block: *const c_void) -> *mut c_void;
        fn _Block_release(block: *const c_void);
    }
    struct OwnedObject(Object);
    impl Drop for OwnedObject {
        fn drop(&mut self) {
            if !self.0.is_null() {
                void0(self.0, c"release");
            }
        }
    }
    fn selector(s: &CStr) -> Object {
        // SAFETY: static NUL-terminated selector; runtime retains the interned selector.
        unsafe { sel_registerName(s.as_ptr()) }
    }
    fn class(s: &CStr) -> Result<Object, HumanError> {
        // SAFETY: static NUL-terminated class name; linked system frameworks own class lifetime.
        let v = unsafe { objc_getClass(s.as_ptr()) };
        if v.is_null() {
            Err(HumanError::Unavailable)
        } else {
            Ok(v)
        }
    }
    fn object0(receiver: Object, name: &CStr) -> Object {
        // SAFETY: only NSObject alloc/init selectors; ARM64 object and SEL are pointer-sized.
        unsafe {
            let f: unsafe extern "C" fn(Object, Object) -> Object =
                std::mem::transmute(objc_msgSend as *const ());
            f(receiver, selector(name))
        }
    }
    fn void0(receiver: Object, name: &CStr) {
        // SAFETY: only NSObject release and LAContext invalidate, both void with no arguments.
        unsafe {
            let f: unsafe extern "C" fn(Object, Object) =
                std::mem::transmute(objc_msgSend as *const ());
            f(receiver, selector(name));
        }
    }
    fn context() -> Result<OwnedObject, HumanError> {
        let value = object0(object0(class(c"LAContext")?, c"alloc"), c"init");
        if value.is_null() {
            Err(HumanError::ServiceFailure)
        } else {
            Ok(OwnedObject(value))
        }
    }
    fn can_evaluate(context: Object) -> bool {
        // SAFETY: SDK LAContext canEvaluatePolicy:(NSInteger) error:(NSError**), policy2.
        // ARM64 macOS BOOL is bool; a null error out pointer requests no error object.
        unsafe {
            let f: unsafe extern "C" fn(Object, Object, isize, *mut Object) -> bool =
                std::mem::transmute(objc_msgSend as *const ());
            f(
                context,
                selector(c"canEvaluatePolicy:error:"),
                2,
                std::ptr::null_mut(),
            )
        }
    }
    pub(super) fn available() -> Result<bool, HumanError> {
        let context = context()?;
        Ok(can_evaluate(context.0))
    }
    #[repr(C)]
    struct Descriptor {
        reserved: usize,
        size: usize,
        copy: unsafe extern "C" fn(*mut Block, *const Block),
        dispose: unsafe extern "C" fn(*const Block),
        signature: *const c_char,
    }
    // SAFETY: descriptor is immutable static ABI metadata; signature points to static bytes.
    unsafe impl Sync for Descriptor {}
    #[repr(C)]
    struct Block {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: unsafe extern "C" fn(*const Block, bool, Object),
        descriptor: *const Descriptor,
        state: *const Completion,
    }
    static DESCRIPTOR: Descriptor = Descriptor {
        reserved: 0,
        size: std::mem::size_of::<Block>(),
        copy: copy_state,
        dispose: dispose_state,
        signature: c"v24@?0B8@16".as_ptr(),
    };
    unsafe extern "C" fn copy_state(_dst: *mut Block, src: *const Block) {
        // SAFETY: Blocks runtime copied the entire literal and invokes this helper on a live
        // source with a valid Arc-owned state. Each heap copy gets one extra strong reference.
        unsafe {
            Arc::increment_strong_count((*src).state);
        }
    }
    unsafe extern "C" fn dispose_state(src: *const Block) {
        // SAFETY: one helper invocation per copied block lifetime; pairs with copy_state.
        unsafe {
            Arc::decrement_strong_count((*src).state);
        }
    }
    unsafe extern "C" fn invoke(block: *const Block, success: bool, _error: Object) {
        // SAFETY: OS retains a copied block until the callback ends; its Arc state remains
        // alive even after timeout/context invalidation. NSError is never read or retained.
        unsafe {
            (&*(*block).state).complete_at(success, Instant::now());
        }
    }
    struct OwnedBlock(*mut c_void);
    impl Drop for OwnedBlock {
        fn drop(&mut self) {
            // SAFETY: pointer is the nonnull heap block returned by _Block_copy; release once.
            unsafe {
                _Block_release(self.0);
            }
        }
    }
    fn callback(state: Arc<Completion>) -> Result<OwnedBlock, HumanError> {
        let ptr = Arc::into_raw(state);
        // SAFETY: documented Apple Blocks ABI stack isa and immutable descriptor, explicit
        // copy/dispose flags. Copy is synchronous and balances our stack-owned Arc below.
        let block = Block {
            isa: std::ptr::addr_of!(_NSConcreteStackBlock).cast(),
            flags: (1 << 25) | (1 << 30),
            reserved: 0,
            invoke,
            descriptor: &DESCRIPTOR,
            state: ptr,
        };
        let heap = unsafe { _Block_copy((&block as *const Block).cast()) };
        // SAFETY: stack literal owns one strong reference independently of any heap copies.
        unsafe {
            Arc::decrement_strong_count(ptr);
        }
        if heap.is_null() {
            Err(HumanError::ServiceFailure)
        } else {
            Ok(OwnedBlock(heap))
        }
    }
    struct Invalidate<'a>(&'a OwnedObject);
    impl Drop for Invalidate<'_> {
        fn drop(&mut self) {
            void0(self.0.0, c"invalidate");
        }
    }
    pub(super) fn authenticate(
        purpose: HumanPurpose,
        deadline: Instant,
    ) -> Result<Instant, HumanError> {
        let pool = OwnedObject(object0(
            object0(class(c"NSAutoreleasePool")?, c"alloc"),
            c"init",
        ));
        let context = context()?;
        let _invalidate = Invalidate(&context);
        if !can_evaluate(context.0) {
            return Err(HumanError::Unavailable);
        }
        // SAFETY: documented setter takes NSTimeInterval(double). Fresh context, zero reuse.
        unsafe {
            let f: unsafe extern "C" fn(Object, Object, f64) =
                std::mem::transmute(objc_msgSend as *const ());
            f(
                context.0,
                selector(c"setTouchIDAuthenticationAllowableReuseDuration:"),
                0.0,
            );
        }
        let allocated = object0(class(c"NSString")?, c"alloc");
        // SAFETY: NSString initWithUTF8String takes valid static NUL-terminated UTF8; owns result.
        let string = unsafe {
            let f: unsafe extern "C" fn(Object, Object, *const c_char) -> Object =
                std::mem::transmute(objc_msgSend as *const ());
            f(
                allocated,
                selector(c"initWithUTF8String:"),
                purpose.reason().as_ptr(),
            )
        };
        if string.is_null() {
            return Err(HumanError::ServiceFailure);
        }
        let reason = OwnedObject(string);
        let state = Arc::new(Completion::new(deadline));
        let block = callback(state.clone())?;
        if Instant::now() >= deadline {
            return Err(HumanError::Timeout);
        }
        // SAFETY: SDK evaluatePolicy:localizedReason:reply:, NSInteger2, owned nonempty NSString
        // and valid heap block (BOOL,NSError*). OS copies reply; context lives through wait and
        // invalidation. Callback synchronizes without unwinding and ignores all error content.
        unsafe {
            let f: unsafe extern "C" fn(Object, Object, isize, Object, *mut c_void) =
                std::mem::transmute(objc_msgSend as *const ());
            f(
                context.0,
                selector(c"evaluatePolicy:localizedReason:reply:"),
                2,
                reason.0,
                block.0,
            );
        }
        let result = state.wait();
        // Objects drop in reverse order; invalidation guard runs before context/pool release.
        let _ = &pool;
        result
    }
    #[cfg(test)]
    pub(super) fn test_block_lifetime() {
        let state = Arc::new(Completion::new(Instant::now() + Duration::from_secs(1)));
        let weak = Arc::downgrade(&state);
        let first = callback(state.clone()).unwrap();
        // SAFETY: real OS copy of a live heap block; each release matches one ownership.
        let copy = unsafe { _Block_copy(first.0) };
        assert!(!copy.is_null());
        let second = OwnedBlock(copy);
        drop(state);
        drop(first);
        assert!(weak.upgrade().is_some());
        drop(second);
        assert!(weak.upgrade().is_none());
    }
}
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
mod native {
    use super::*;
    pub(super) fn available() -> Result<bool, HumanError> {
        Err(HumanError::Unavailable)
    }
    pub(super) fn authenticate(_: HumanPurpose, _: Instant) -> Result<Instant, HumanError> {
        Err(HumanError::Unavailable)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_challenge_and_timeout_never_open_authentication_ui() {
        assert!(matches!(
            authenticate_device_owner([0; 32], HumanPurpose::LocalCli, Duration::from_secs(1)),
            Err(HumanError::InvalidInput)
        ));
        assert!(matches!(
            authenticate_device_owner([1; 32], HumanPurpose::LocalCli, Duration::ZERO),
            Err(HumanError::InvalidInput)
        ));
        assert!(matches!(
            authenticate_device_owner([1; 32], HumanPurpose::LocalCli, Duration::from_secs(61)),
            Err(HumanError::InvalidInput)
        ));
    }
    #[test]
    fn first_reply_is_final_and_late_reply_cannot_promote_timeout() {
        let now = Instant::now();
        let c = Completion::new(now + Duration::from_secs(1));
        c.complete_at(false, now);
        c.complete_at(true, now);
        assert_eq!(c.wait(), Err(HumanError::Denied));
        let late = Completion::new(now);
        late.complete_at(true, now);
        assert_eq!(late.wait(), Err(HumanError::Timeout));
    }
    #[test]
    fn proof_binding_and_freshness_are_consumed_not_caller_attestation() {
        let now = Instant::now();
        let good = DeviceOwnerProof {
            binding: [7; 32],
            verified_at: now,
        };
        assert_eq!(good.consume_binding(Duration::from_secs(15)), Ok([7; 32]));
        let stale = DeviceOwnerProof {
            binding: [7; 32],
            verified_at: now - Duration::from_secs(20),
        };
        assert_eq!(
            stale.consume_binding(Duration::from_secs(15)),
            Err(HumanError::Expired)
        );
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn actual_os_block_copy_holds_reply_state_until_last_dispose() {
        native::test_block_lifetime();
    }
}
