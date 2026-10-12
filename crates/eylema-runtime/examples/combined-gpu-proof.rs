//! Bounded native Metal measurement worker; browser lifetime is independent.
use eylema_platform::{MAX_METAL_ELEMENTS, metal_add_i32};
use eylema_runtime::resource::{ResourceClass, ResourceLimits, ResourceManager};
use std::io::{self, Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_SAMPLES: usize = 10_000;

#[derive(Debug, Eq, PartialEq)]
enum Control {
    Stop,
    Invalid,
}

fn read_stop(reader: &mut impl Read) -> io::Result<Option<Control>> {
    let mut command = [0_u8; 6];
    for used in 0..command.len() {
        if reader.read(&mut command[used..used + 1])? == 0 {
            return Ok(if used == 0 {
                None
            } else {
                Some(Control::Invalid)
            });
        }
        if command[used] == b'\n' {
            return Ok(Some(
                if &command[..=used] == b"stop\n" || &command[..=used] == b"stop\r\n" {
                    Control::Stop
                } else {
                    Control::Invalid
                },
            ));
        }
    }
    Ok(Some(Control::Invalid))
}

fn duration_argument(arguments: &[String]) -> Result<u64, &'static str> {
    if arguments.len() != 1 {
        return Err("one duration in seconds is required");
    }
    arguments[0]
        .parse::<u64>()
        .ok()
        .filter(|seconds| (1..=120).contains(seconds))
        .ok_or("duration must be an integer between 1 and 120 seconds")
}

fn unix_ms() -> Result<u128, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seconds = duration_argument(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let manager = ResourceManager::new(ResourceLimits::default())?;
    let (stop_tx, stop_rx) = mpsc::channel();
    std::thread::spawn(move || match read_stop(&mut io::stdin().lock()) {
        Ok(Some(control)) => {
            let _ = stop_tx.send(control);
        }
        Err(_) => {
            let _ = stop_tx.send(Control::Invalid);
        }
        Ok(None) => {}
    });
    let lhs = vec![1_i32; MAX_METAL_ELEMENTS];
    let rhs = vec![2_i32; MAX_METAL_ELEMENTS];
    let bytes = MAX_METAL_ELEMENTS * std::mem::size_of::<i32>() * 3;
    let start = Instant::now();
    let deadline = Duration::from_secs(seconds);
    println!(
        "{{\"event\":\"started\",\"unix_ms\":{},\"pid\":{},\"elements\":{},\"declared_metal_buffer_bytes\":{},\"max_samples\":{}}}",
        unix_ms()?,
        std::process::id(),
        MAX_METAL_ELEMENTS,
        bytes,
        MAX_SAMPLES
    );
    io::stdout().flush()?;
    let mut count = 0;
    let mut reason = "duration_limit";
    while start.elapsed() < deadline && count < MAX_SAMPLES {
        match stop_rx.try_recv() {
            Ok(Control::Stop) => {
                reason = "requested_stop";
                break;
            }
            Ok(Control::Invalid) => {
                return Err("invalid control input; at most six bytes are accepted".into());
            }
            Err(_) => {}
        }
        let id = manager.admit(ResourceClass::Metal, bytes)?;
        let lease = manager.dispatch_next()?.ok_or("Metal lease unavailable")?;
        assert_eq!(lease.id(), id);
        lease.check_cancelled()?;
        let unix_start = unix_ms()?;
        let call_start = Instant::now();
        let output = metal_add_i32(&lhs, &rhs)?;
        let elapsed_ns = call_start.elapsed().as_nanos();
        if output.len() != MAX_METAL_ELEMENTS || output.iter().any(|value| *value != 3) {
            return Err("Metal readback differs from the exact CPU reference".into());
        }
        println!(
            "{{\"event\":\"sample\",\"index\":{},\"start_unix_ms\":{},\"end_unix_ms\":{},\"duration_ns\":{},\"status\":\"passed\"}}",
            count,
            unix_start,
            unix_ms()?,
            elapsed_ns
        );
        io::stdout().flush()?;
        drop(lease);
        count += 1;
        std::thread::sleep(Duration::from_millis(5));
    }
    if count == MAX_SAMPLES {
        reason = "sample_limit";
    }
    assert_eq!(manager.snapshot()?, Default::default());
    println!(
        "{{\"event\":\"finished\",\"unix_ms\":{},\"samples\":{},\"reason\":\"{}\"}}",
        unix_ms()?,
        count,
        reason
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Control, duration_argument, read_stop};
    use std::io::Cursor;

    #[test]
    fn duration_is_finite_and_requires_one_integer_argument() {
        for value in ["0", "121", "-1", "1.5", "NaN"] {
            assert!(duration_argument(&[value.to_owned()]).is_err());
        }
        assert!(duration_argument(&[]).is_err());
        assert!(duration_argument(&["1".into(), "2".into()]).is_err());
        assert_eq!(duration_argument(&["1".into()]), Ok(1));
        assert_eq!(duration_argument(&["120".into()]), Ok(120));
    }

    #[test]
    fn control_reader_has_a_fixed_byte_bound_and_rejects_oversized_input() {
        let mut oversized = Cursor::new(vec![b'a'; 65_536]);
        assert_eq!(read_stop(&mut oversized).unwrap(), Some(Control::Invalid));
        assert_eq!(
            oversized.position(),
            6,
            "the unread input never enters an allocation"
        );
        for value in [b"stop\n".as_slice(), b"stop\r\n".as_slice()] {
            assert_eq!(
                read_stop(&mut Cursor::new(value)).unwrap(),
                Some(Control::Stop)
            );
        }
        assert_eq!(
            read_stop(&mut Cursor::new(b"stop")).unwrap(),
            Some(Control::Invalid)
        );
        assert_eq!(
            read_stop(&mut Cursor::new(b"no\n")).unwrap(),
            Some(Control::Invalid)
        );
        assert_eq!(read_stop(&mut Cursor::new(b"")).unwrap(), None);
    }
}
