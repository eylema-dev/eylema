use crate::*;
use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;

trait DeadlineClock {
    fn now(&self) -> Instant;
}

struct SystemClock;
impl DeadlineClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

trait TimedRead: Read {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
}
impl TimedRead for TcpStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }
}

struct RequestDeadline(Instant);
impl RequestDeadline {
    fn start(clock: &impl DeadlineClock, timeout: Duration) -> Result<Self, WebError> {
        clock
            .now()
            .checked_add(timeout)
            .map(Self)
            .ok_or(WebError::Configuration)
    }

    fn remaining(&self, clock: &impl DeadlineClock) -> Result<Duration, WebError> {
        self.0
            .checked_duration_since(clock.now())
            .filter(|d| !d.is_zero())
            .ok_or(WebError::Timeout)
    }
}

pub(crate) struct Headers(BTreeMap<String, String>);
impl Headers {
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
}
fn remaining(deadline: Instant) -> Result<Duration, WebError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(WebError::Timeout)
}
fn read(
    reader: &mut BufReader<impl TimedRead>,
    bytes: &mut [u8],
    deadline: &RequestDeadline,
    clock: &impl DeadlineClock,
) -> Result<usize, WebError> {
    for _ in 0..16 {
        reader
            .get_ref()
            .set_read_timeout(Some(deadline.remaining(clock)?))
            .map_err(|_| WebError::Io)?;
        match reader.read(bytes) {
            Ok(0) => return Err(WebError::Malformed),
            Ok(n) => {
                deadline.remaining(clock)?;
                return Ok(n);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(WebError::Timeout);
            }
            Err(_) => return Err(WebError::Io),
        }
    }
    Err(WebError::Io)
}
fn headers(
    reader: &mut BufReader<impl TimedRead>,
    limit: usize,
    deadline: &RequestDeadline,
    clock: &impl DeadlineClock,
) -> Result<Headers, WebError> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        if bytes.len() == limit {
            return Err(WebError::TooLarge);
        }
        let mut byte = [0];
        read(reader, &mut byte, deadline, clock)?;
        bytes.push(byte[0]);
    }
    let source = std::str::from_utf8(&bytes).map_err(|_| WebError::Malformed)?;
    let mut lines = source[..source.len() - 4].split("\r\n");
    let request = lines.next().ok_or(WebError::Malformed)?;
    let parts: Vec<_> = request.split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(WebError::Malformed);
    }
    if parts[0] != "POST" || parts[1] != "/v1/request" {
        return Err(WebError::Unsupported);
    }
    let mut values = BTreeMap::new();
    for line in lines {
        if values.len() == 32 {
            return Err(WebError::TooLarge);
        }
        let (name, value) = line.split_once(':').ok_or(WebError::Malformed)?;
        if name.is_empty()
            || name.len() > 96
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(WebError::Malformed);
        }
        let value = value.trim_matches([' ', '\t']);
        if value.len() > 2048 || !value.bytes().all(|b| b.is_ascii_graphic() || b == b' ') {
            return Err(WebError::Malformed);
        }
        if values
            .insert(name.to_ascii_lowercase(), value.to_owned())
            .is_some()
        {
            return Err(WebError::Malformed);
        }
    }
    if values.contains_key("transfer-encoding")
        || values.contains_key("expect")
        || values.contains_key("content-encoding")
    {
        return Err(WebError::Malformed);
    }
    Ok(Headers(values))
}
fn process<T: Storage, H: Sha256, S: HostServices>(
    server: &mut Server<T, H, S>,
    reader: &mut BufReader<impl TimedRead>,
    clock: &impl DeadlineClock,
) -> Result<Value, WebError> {
    let deadline = RequestDeadline::start(clock, server.config.request_timeout)?;
    let headers = headers(reader, server.config.header_bytes, &deadline, clock)?;
    let content_length = headers.get("content-length").ok_or(WebError::Malformed)?;
    if content_length.is_empty() || !content_length.bytes().all(|b| b.is_ascii_digit()) {
        return Err(WebError::Malformed);
    }
    let length: usize = content_length.parse().map_err(|_| WebError::TooLarge)?;
    if length > server.config.body_bytes {
        return Err(WebError::TooLarge);
    }
    if headers.get("content-type") != Some("application/eylema") {
        return Err(WebError::Malformed);
    }
    let actor = server.authenticate(&headers)?;
    let mut body = vec![0; length];
    let mut offset = 0;
    while offset < body.len() {
        offset += read(reader, &mut body[offset..], &deadline, clock)?;
    }
    if !reader.buffer().is_empty() {
        return Err(WebError::Malformed);
    }
    deadline.remaining(clock)?;
    let request = protocol::decode_request(&body).map_err(|_| WebError::Malformed)?;
    // Expiry is checked again after bounded body ingress. Decoding/request
    // data cannot mint a session or change an actor's accepted authority.
    if Instant::now() >= server.actors[actor].expires {
        return Err(WebError::Unauthorized);
    }
    server.dispatch(actor, request)
}
fn write(stream: &mut TcpStream, mut bytes: &[u8], deadline: Instant) -> Result<(), WebError> {
    let mut interrupted = 0;
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining(deadline)?))
            .map_err(|_| WebError::Io)?;
        match stream.write(bytes) {
            Ok(0) => return Err(WebError::Io),
            Ok(n) => {
                bytes = &bytes[n..];
                interrupted = 0;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted && interrupted < 16 => {
                interrupted += 1
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(WebError::Timeout);
            }
            Err(_) => return Err(WebError::Io),
        }
    }
    Ok(())
}
pub(crate) fn serve<T: Storage, H: Sha256, S: HostServices>(
    server: &mut Server<T, H, S>,
    stream: TcpStream,
) {
    let mut reader = BufReader::with_capacity(512, stream);
    let result = process(server, &mut reader, &SystemClock);
    let (mut status, mut body) = match result {
        Ok(value) => match protocol::encode_response(value) {
            Ok(body) => (200, body),
            Err(_) => (
                413,
                protocol::encode_error(WebError::TooLarge.code()).expect("static error"),
            ),
        },
        Err(error) => (
            error.status(),
            protocol::encode_error(error.code()).expect("bounded error code"),
        ),
    };
    if body.len() > server.config.response_bytes {
        status = 413;
        body = protocol::encode_error(WebError::TooLarge.code()).expect("static error");
    }
    let response = format!(
        "HTTP/1.1 {status} Response\r\nContent-Type: application/eylema\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n\r\n",
        body.len()
    );
    let mut response = response.into_bytes();
    response.extend_from_slice(&body);
    let stream = reader.get_mut();
    if let Some(deadline) = Instant::now().checked_add(server.config.response_timeout) {
        // Emit one bounded frame and a write-side FIN before dropping a socket
        // with unread rejected ingress. A client disconnect still loses its
        // response, never the native listener or prior durable result.
        if write(stream, &response, deadline).is_ok() {
            let _ = stream.shutdown(std::net::Shutdown::Write);
            // Bounded close grace drains at most 8 KiB in at most 5 ms total.
            // This does not extend the request ingress deadline or dispatch.
            if let Some(close_deadline) = Instant::now().checked_add(Duration::from_millis(5)) {
                let mut discarded = 0;
                let mut scratch = [0; 1024];
                while discarded < 8192 {
                    let Ok(duration) = remaining(close_deadline) else {
                        break;
                    };
                    if stream.set_read_timeout(Some(duration)).is_err() {
                        break;
                    }
                    match stream.read(&mut scratch) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => discarded += n,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::io;
    use std::rc::Rc;

    struct TestClock(Rc<Cell<Instant>>);
    impl DeadlineClock for TestClock {
        fn now(&self) -> Instant {
            self.0.get()
        }
    }

    struct TimedBytes {
        bytes: &'static [u8],
        offset: usize,
        now: Rc<Cell<Instant>>,
        header_len: usize,
        header_progress: Duration,
        body_progress: Duration,
        timeout: Cell<Duration>,
        observed_timeouts: Rc<RefCell<Vec<Duration>>>,
    }
    impl TimedRead for TimedBytes {
        fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
            let timeout = timeout.ok_or_else(|| io::Error::other("missing deadline"))?;
            self.timeout.set(timeout);
            self.observed_timeouts.borrow_mut().push(timeout);
            Ok(())
        }
    }
    impl Read for TimedBytes {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.offset == self.bytes.len() {
                return Ok(0);
            }
            let now = self.now.get();
            let available = self.timeout.get();
            let progress = if self.offset < self.header_len {
                self.header_progress
            } else {
                self.body_progress
            };
            if progress > available {
                self.now.set(now + available);
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.now.set(now + progress);
            let count = output.len().min(self.bytes.len() - self.offset);
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    #[test]
    fn absolute_deadline_is_shared_across_header_and_body_progress() {
        let header = b"POST /v1/request HTTP/1.1\r\n\r\n";
        let start = Instant::now();
        let now = Rc::new(Cell::new(start));
        let clock = TestClock(now.clone());
        let stream = TimedBytes {
            bytes: b"POST /v1/request HTTP/1.1\r\n\r\nbody",
            offset: 0,
            now,
            header_len: header.len(),
            header_progress: Duration::from_millis(2),
            body_progress: Duration::from_millis(20),
            timeout: Cell::new(Duration::ZERO),
            observed_timeouts: Rc::new(RefCell::new(Vec::new())),
        };
        let observed = stream.observed_timeouts.clone();
        let deadline = RequestDeadline::start(&clock, Duration::from_millis(90)).unwrap();
        let mut reader = BufReader::with_capacity(1, stream);

        // The valid header consumes 2ms per byte. Body ingress gets only the
        // remainder of the same 90ms budget. No wall-clock pacing is involved.
        assert!(headers(&mut reader, 128, &deadline, &clock).is_ok());
        let mut body = [0];
        assert_eq!(read(&mut reader, &mut body, &deadline, &clock), Ok(1));
        assert_eq!(body[0], b'b');
        assert!(matches!(
            read(&mut reader, &mut body, &deadline, &clock),
            Err(WebError::Timeout)
        ));
        assert_eq!(reader.get_ref().offset, header.len() + 1);
        let timeouts = observed.borrow();
        assert_eq!(timeouts.len(), header.len() + 2);
        assert_eq!(timeouts[0], Duration::from_millis(90));
        let after_header = 90 - (header.len() as u64 * 2);
        assert_eq!(timeouts[header.len()], Duration::from_millis(after_header));
        assert_eq!(
            timeouts[header.len() + 1],
            Duration::from_millis(after_header - 20)
        );
    }
}
