//! Synchronous single-instance memory boundary. The host owns linear-memory
//! access and must copy output before the next mutating call. No crypto imports.
use super::*;
use eylema_contracts::codec::{self, WireRecord};
use std::sync::Mutex;

struct Buffers {
    input: Vec<u8>,
    output: Vec<u8>,
}
static BUFFERS: Mutex<Buffers> = Mutex::new(Buffers {
    input: Vec::new(),
    output: Vec::new(),
});

/// Invalid allocations clear both buffers. Zero is an error sentinel.
#[unsafe(no_mangle)]
pub extern "C" fn eyl_allocate_input(length: u32) -> u32 {
    let Ok(mut state) = BUFFERS.lock() else {
        return 0;
    };
    state.input.clear();
    state.output.clear();
    if length == 0 || length as usize > MAX_WIRE_BYTES {
        return 0;
    }
    state.input.resize(length as usize, 0);
    state.input.as_mut_ptr() as u32
}
fn process(input: &[u8], mode: u32) -> Result<Vec<u8>, ()> {
    match mode {
        0 => codec::encode(&codec::decode(input).map_err(|_| ())?).map_err(|_| ()),
        1 => codec::encode_record(&codec::decode_record(input).map_err(|_| ())?).map_err(|_| ()),
        2 => match codec::decode_record(input).map_err(|_| ())? {
            WireRecord::Proposal { task, proposal }
            | WireRecord::Execution { task, proposal, .. } => {
                codec::canonical_approval_payload(&task, &proposal).map_err(|_| ())
            }
            _ => Err(()),
        },
        3 => {
            if input.len() != 17 {
                return Err(());
            }
            let kind = IdKind::from_byte(input[0]).map_err(|_| ())?;
            let entropy: [u8; 16] = input[1..].try_into().map_err(|_| ())?;
            Ok(id_from_entropy(kind, &entropy).into_bytes())
        }
        _ => Err(()),
    }
}
/// Status 0 success; 1 invalid buffer length/lock; 2 invalid codec/kind/mode.
#[unsafe(no_mangle)]
pub extern "C" fn eyl_process(length: u32, mode: u32) -> u32 {
    let Ok(mut state) = BUFFERS.lock() else {
        return 1;
    };
    state.output.clear();
    if length == 0 || length as usize != state.input.len() {
        return 1;
    }
    match process(&state.input, mode) {
        Ok(output) => {
            state.output = output;
            0
        }
        Err(()) => 2,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn eyl_output_ptr() -> u32 {
    BUFFERS.lock().map_or(0, |state| {
        if state.output.is_empty() {
            0
        } else {
            state.output.as_ptr() as u32
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn eyl_output_len() -> u32 {
    BUFFERS.lock().map_or(0, |state| state.output.len() as u32)
}
