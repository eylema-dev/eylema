//! Actual native sockets and full-synchronized owned journal. Trusted enrollment
//! fixtures are not real person authentication, browser or physical power-loss evidence.
#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
use eylema_contracts::{
    codec::{self, Value, WireRecord},
    *,
};
use eylema_host::{
    DurableHost,
    native::{NativeHash, NativeServices},
};
use eylema_journal::{Journal, NativeStorage};
use eylema_runtime::HostServices;
use eylema_web::{
    Config, Credentials, Server, WebError,
    protocol::{self, Request},
};
use std::{
    fs::File,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
fn sync(f: &File) -> std::io::Result<()> {
    eylema_platform::full_sync_file(f).map_err(std::io::Error::other)
}
struct Fixture {
    address: SocketAddr,
    origin: String,
    path: PathBuf,
    normal: Credentials,
    reader: Credentials,
    other: Credentials,
    human: Credentials,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Fixture {
    fn new(config: Config) -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::env::temp_dir().join(format!(
            "eylema-http-{}",
            eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap()
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let path = dir.join("journal.eylj");
        let storage = NativeStorage::create_new(&path, sync).unwrap();
        let journal =
            Journal::create_new(storage, NativeHash, eylema_journal::Limits::default()).unwrap();
        let (host, owner) =
            DurableHost::create_new(journal, NativeServices, eylema_runtime::Limits::default())
                .unwrap();
        let mut server = Server::bind(host, owner, config).unwrap();
        let mut grant = support::grant();
        grant.permissions.push(Permission::RequestCancellation);
        let normal = server.register_session(grant.clone()).unwrap();
        grant.principal = PrincipalRef::new("principal:reader").unwrap();
        grant.permissions = vec![Permission::Read, Permission::ObserveEvents];
        let reader = server.register_session(grant.clone()).unwrap();
        grant.resource_scope =
            ResourceScope::new(vec![ResourceId::new("state:other").unwrap()]).unwrap();
        let other = server.register_session(grant).unwrap();
        let mut grant = support::grant();
        grant.principal = PrincipalRef::new("principal:verified-fixture").unwrap();
        grant.permissions = vec![
            Permission::Read,
            Permission::ObserveEvents,
            Permission::RequestCancellation,
        ];
        let human = server.register_verified_human(grant).unwrap();
        let address = server.address().unwrap();
        let origin = server.origin().to_owned();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let worker = Some(thread::spawn(move || server.run(&flag).unwrap()));
        Self {
            address,
            origin,
            path,
            normal,
            reader,
            other,
            human,
            stop,
            worker,
        }
    }
    fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
    }
    fn header(&self, credentials: &Credentials, length: usize) -> String {
        format!(
            "POST /v1/request HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nAuthorization: Bearer {}\r\nX-Eylema-CSRF: {}\r\nContent-Type: application/eylema\r\nContent-Length: {length}\r\n\r\n",
            self.address,
            self.origin,
            credentials.bearer(),
            credentials.csrf()
        )
    }
    fn raw(&self, bytes: &[u8]) -> (u16, Result<Value, protocol::TransportError>) {
        let mut stream = self.connect();
        stream.write_all(bytes).unwrap();
        response(&mut stream)
    }
    #[allow(dead_code)]
    fn raw_http(&self, bytes: &[u8]) -> String {
        let mut stream = self.connect();
        stream.write_all(bytes).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        String::from_utf8(response).unwrap()
    }
    fn request(
        &self,
        credentials: &Credentials,
        request: &Request,
    ) -> (u16, Result<Value, protocol::TransportError>) {
        let body = protocol::encode_request(request).unwrap();
        let mut bytes = self.header(credentials, body.len()).into_bytes();
        bytes.extend(body);
        self.raw(&bytes)
    }
    fn ok(&self, credentials: &Credentials, request: &Request) -> Value {
        let (status, value) = self.request(credentials, request);
        assert_eq!(status, 200);
        value.unwrap()
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(&self.path).unwrap()
    }
}

#[test]
fn browser_console_cookie_csrf_task_and_stop_flow() {
    let f = Fixture::new(Config::default());
    let get = format!(
        "GET /console HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        f.address
    );
    let page = f.raw_http(get.as_bytes());
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("Set-Cookie: eylema_session="));
    assert!(page.contains("HttpOnly; SameSite=Strict"));
    assert!(page.contains("name=\"eylema-csrf\""));
    assert!(page.contains("Create task"));
    assert!(page.contains("aria-live=\"assertive\""));
    for (path, marker) in [
        ("/console.css", "focus-visible"),
        ("/console.js", "localStorage"),
    ] {
        let asset = f.raw_http(
            format!(
                "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                f.address
            )
            .as_bytes(),
        );
        assert!(asset.starts_with("HTTP/1.1 200"), "{asset}");
        assert!(asset.contains(marker));
        assert!(asset.contains("Content-Security-Policy: default-src 'self'"));
    }
    let cookie = page
        .split("Set-Cookie: eylema_session=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let csrf = page
        .split("name=\"eylema-csrf\" content=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let post = |path: &str, body: &str| {
        format!(
            "POST /console/v1/{path} HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nSec-Fetch-Site: same-origin\r\nCookie: eylema_session={cookie}\r\nX-Eylema-CSRF: {csrf}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            f.address,
            f.origin,
            body.len()
        )
    };
    let created = f.raw_http(post("create", "{\"delta\":7}").as_bytes());
    assert!(created.starts_with("HTTP/1.1 200"), "{created}");
    let created_body = created.split("\r\n\r\n").nth(1).unwrap();
    let task = created_body
        .split("\"task\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let running =
        f.raw_http(post("run", &format!("{{\"task\":\"{task}\",\"delta\":7}}")).as_bytes());
    assert!(running.starts_with("HTTP/1.1 200"), "{running}");
    let run_body = running.split("\r\n\r\n").nth(1).unwrap();
    let execution = run_body
        .split("\"execution\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let inspect = f.raw_http(
        post(
            "inspect",
            &format!("{{\"task\":\"{task}\",\"execution\":\"{execution}\"}}"),
        )
        .as_bytes(),
    );
    assert!(inspect.starts_with("HTTP/1.1 200"), "{inspect}");
    assert!(inspect.contains(task));
    assert!(inspect.contains(execution));
    let stop = f.raw_http(post("stop", &format!("{{\"execution\":\"{execution}\"}}")).as_bytes());
    assert!(stop.starts_with("HTTP/1.1 403"), "{stop}");
    assert!(stop.contains("FORBIDDEN"));
    let no_csrf = format!(
        "POST /console/v1/inspect HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nCookie: eylema_session={cookie}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
        f.address, f.origin
    );
    let rejected = f.raw_http(no_csrf.as_bytes());
    assert!(rejected.starts_with("HTTP/1.1 403"));
    assert!(rejected.contains("FORBIDDEN"));
    let wrong_origin = post("inspect", &format!("{{\"task\":\"{task}\"}}"))
        .replace(&f.origin, "http://attacker.invalid");
    let rejected_origin = f.raw_http(wrong_origin.as_bytes());
    assert!(rejected_origin.starts_with("HTTP/1.1 403"));
    let oversized = format!(
        "POST /console/v1/inspect HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nSec-Fetch-Site: same-origin\r\nCookie: eylema_session={cookie}\r\nX-Eylema-CSRF: {csrf}\r\nContent-Type: application/json\r\nContent-Length: 999999\r\nConnection: close\r\n\r\n",
        f.address, f.origin
    );
    let rejected_size = f.raw_http(oversized.as_bytes());
    assert!(rejected_size.starts_with("HTTP/1.1 413"));
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                assert!(result.is_ok());
            }
        }
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}
fn response(stream: &mut TcpStream) -> (u16, Result<Value, protocol::TransportError>) {
    let mut bytes = Vec::new();
    let result = stream.read_to_end(&mut bytes);
    if let Err(e) = result {
        assert_eq!(e.kind(), std::io::ErrorKind::ConnectionReset);
    }
    let cut = bytes
        .windows(4)
        .position(|x| x == b"\r\n\r\n")
        .expect("bounded HTTP response")
        + 4;
    let header = std::str::from_utf8(&bytes[..cut]).unwrap();
    assert!(header.contains("Cache-Control: no-store\r\n"));
    assert!(header.contains("X-Content-Type-Options: nosniff\r\n"));
    assert!(!header.to_ascii_lowercase().contains("access-control-allow"));
    let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
    let length: usize = header
        .lines()
        .find_map(|x| x.strip_prefix("Content-Length: "))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(bytes.len() - cut, length);
    (status, protocol::decode_response(&bytes[cut..]))
}
fn task(name: &str) -> TaskSpec {
    let now = NativeServices.now().unwrap().unix_ms();
    let mut draft = support::task(name, 30, 100).fields().clone();
    draft.budget = Budget::new(30, 100, 1024, 1, Timestamp::new(now + 60000).unwrap()).unwrap();
    TaskSpec::new(draft).unwrap()
}
fn task_read(t: &TaskSpec) -> Request {
    Request::Task(t.fields().task_id.clone())
}
fn execution(value: Value) -> ExecutionId {
    let Value::Text(id) = value else {
        panic!("execution ID")
    };
    ExecutionId::new(id).unwrap()
}
fn read_execution(f: &Fixture, id: &ExecutionId) -> WireRecord {
    let Value::Bytes(bytes) = f.ok(&f.normal, &Request::Execution(id.clone())) else {
        panic!("typed execution")
    };
    codec::decode_record(&bytes).unwrap()
}
fn terminal(f: &Fixture, id: &ExecutionId) -> WireRecord {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let record = read_execution(f, id);
        if let WireRecord::Execution { execution, .. } = &record
            && matches!(
                execution.fields().state,
                ExecutionState::Succeeded | ExecutionState::Cancelled | ExecutionState::Failed
            )
        {
            return record;
        }
        assert!(Instant::now() < deadline, "worker completion deadline");
        thread::sleep(Duration::from_millis(5));
    }
}
fn positive() {
    let f = Fixture::new(Config::default());
    let t = task("task:http-positive");
    let p = support::proposal(&t, &[2, 3]);
    assert_eq!(f.ok(&f.normal, &Request::Create(t.clone())), Value::Null);
    let request = Request::Admit {
        task: t.clone(),
        proposal: p.clone(),
        approvals: vec![],
    };
    let id = execution(f.ok(&f.normal, &request));
    // The client socket is closed after every response; native work still completes.
    let WireRecord::Execution {
        task: original,
        proposal,
        execution: done,
    } = terminal(&f, &id)
    else {
        panic!("execution")
    };
    assert_eq!(original, t);
    assert_eq!(proposal, p);
    assert_eq!(done.fields().state, ExecutionState::Succeeded);
    assert_eq!(execution(f.ok(&f.normal, &request)), id);
    let Value::List(snapshot) = f.ok(&f.reader, &task_read(&t)) else {
        panic!("task snapshot")
    };
    assert_eq!(snapshot[1], Value::Unsigned(5));
    assert_eq!(snapshot[2], Value::Unsigned(2));
    let Value::List(events) = f.ok(
        &f.reader,
        &Request::Events {
            task: t.fields().task_id.clone(),
            after: None,
            limit: 256,
        },
    ) else {
        panic!("events")
    };
    assert!(!events.is_empty());
    for wire in events {
        let Value::Bytes(b) = wire else {
            panic!("event bytes")
        };
        assert!(matches!(
            codec::decode_record(&b).unwrap(),
            WireRecord::Event(_)
        ));
    }
    assert!(!done.fields().result_evidence.is_empty());
    let Value::List(observed) = f.ok(
        &f.normal,
        &Request::Observation(done.fields().result_evidence[0].clone()),
    ) else {
        panic!("observation")
    };
    assert_eq!(observed.len(), 7);
    let bytes = f.bytes();
    thread::sleep(Duration::from_millis(40));
    assert_eq!(f.bytes(), bytes, "idle polling has no journal effect");
    // No credential material is retained in the journal, nor formatted for logging.
    assert!(
        !bytes
            .windows(f.normal.bearer().len())
            .any(|b| b == f.normal.bearer().as_bytes())
    );
    assert!(
        !bytes
            .windows(f.normal.csrf().len())
            .any(|b| b == f.normal.csrf().as_bytes())
    );
    assert_eq!(format!("{:?}", f.normal), "Credentials(<redacted>)");
}
fn authority() {
    let f = Fixture::new(Config::default());
    let t = task("task:http-authority");
    f.ok(&f.normal, &Request::Create(t.clone()));
    let body = protocol::encode_request(&task_read(&t)).unwrap();
    let header = f.header(&f.normal, body.len());
    let before = f.bytes();
    let variants = vec![
        (
            header.replace(&format!("X-Eylema-CSRF: {}\r\n", f.normal.csrf()), ""),
            403,
        ),
        (
            header.replace(&format!("Host: {}", f.address), "Host: attacker.invalid"),
            403,
        ),
        (
            header.replace(&format!("Origin: {}", f.origin), "Origin: null"),
            403,
        ),
        (header.replace(&f.origin, "http://127.0.0.1:1"), 403),
        (
            header.replace(&format!("Origin: {}\r\n", f.origin), ""),
            403,
        ),
        (
            header.replace(
                &format!("Authorization: Bearer {}\r\n", f.normal.bearer()),
                "",
            ),
            401,
        ),
        (header.replace(f.normal.bearer(), "invalid-token"), 401),
        (header.replace(f.normal.csrf(), f.reader.csrf()), 403),
        (
            header.replace("\r\n\r\n", "\r\nSec-Fetch-Site: cross-site\r\n\r\n"),
            403,
        ),
    ];
    for (header, status) in variants {
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(&body);
        assert_eq!(f.raw(&bytes).0, status);
        assert_eq!(f.bytes(), before);
    }
    for (credentials, request) in [
        (&f.reader, Request::Create(task("task:read-only"))),
        (
            &f.reader,
            Request::Admit {
                task: t.clone(),
                proposal: support::proposal(&t, &[1]),
                approvals: vec![],
            },
        ),
        (&f.other, task_read(&t)),
        (&f.other, Request::Create(task("task:foreign-create"))),
        (
            &f.normal,
            Request::Cancel(ExecutionId::new("execution:missing").unwrap()),
        ),
    ] {
        assert_eq!(f.request(credentials, &request).0, 403);
        assert_eq!(f.bytes(), before);
    }
    assert!(matches!(f.ok(&f.normal, &task_read(&t)), Value::List(_)));
}
fn malformed() {
    let f = Fixture::new(Config::default());
    let t = task("task:http-malformed");
    f.ok(&f.normal, &Request::Create(t.clone()));
    let body = protocol::encode_request(&task_read(&t)).unwrap();
    let header = f.header(&f.normal, body.len());
    let before = f.bytes();
    let variants = vec![
        (
            header.replace("\r\n\r\n", "\r\ncontent-length: 1\r\n\r\n"),
            400,
        ),
        (header.replace("\r\n\r\n", "\r\nBad: héllo\r\n\r\n"), 400),
        (header.replace("/v1/request ", "/v1/enroll "), 405),
        (
            header.replace("\r\n\r\n", "\r\nhOsT: attacker.invalid\r\n\r\n"),
            400,
        ),
        (
            header.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
            400,
        ),
        (
            header.replace("\r\n\r\n", "\r\nExpect: 100-continue\r\n\r\n"),
            400,
        ),
        (
            header.replace("\r\n\r\n", "\r\nContent-Encoding: gzip\r\n\r\n"),
            400,
        ),
        (header.replace("Origin:", " Origin:"), 400),
        (
            header.replace("application/eylema", "application/json"),
            400,
        ),
        (
            header.replace(
                &format!("Content-Length: {}", body.len()),
                "Content-Length: -1",
            ),
            400,
        ),
        (
            header.replace(
                &format!("Content-Length: {}", body.len()),
                "Content-Length: 999999999999999999999999999",
            ),
            413,
        ),
        (
            header.replace(&format!("Content-Length: {}\r\n", body.len()), ""),
            400,
        ),
        (header.replace("HTTP/1.1", "HTTP/1.0"), 400),
        (header.replace("POST ", "OPTIONS "), 405),
        (
            header.replace("/v1/request ", "/v1/request?human=true "),
            405,
        ),
        (
            header.replace("/v1/request ", "http://attacker.invalid/v1/request "),
            405,
        ),
        (header.replace("\r\n\r\n", "\r\nBad: x\0y\r\n\r\n"), 400),
    ];
    for (header, status) in variants {
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(&body);
        assert_eq!(f.raw(&bytes).0, status);
        assert_eq!(f.bytes(), before);
        f.ok(&f.normal, &task_read(&t));
    }
    for invalid in [
        b"not EYL".to_vec(),
        codec::encode(&Value::List(vec![
            Value::Unsigned(1),
            Value::Unsigned(99),
            Value::Bool(true),
        ]))
        .unwrap(),
        codec::encode(&Value::List(vec![
            Value::Unsigned(1),
            Value::Unsigned(2),
            Value::Text(t.fields().task_id.as_str().into()),
            Value::Bool(true),
        ]))
        .unwrap(),
        {
            let mut b = body.clone();
            b.push(0);
            b
        },
    ] {
        let mut bytes = f.header(&f.normal, invalid.len()).into_bytes();
        bytes.extend(invalid);
        assert_eq!(f.raw(&bytes).0, 400);
        assert_eq!(f.bytes(), before);
    }
}
fn sizes() {
    let f = Fixture::new(Config::default());
    let before = f.bytes();
    assert_eq!(f.raw(f.header(&f.normal, 65537).as_bytes()).0, 413);
    assert_eq!(f.bytes(), before);
    let mut bytes = b"POST /v1/request HTTP/1.1\r\nLong: ".to_vec();
    bytes.extend(vec![b'a'; 8192]);
    assert_eq!(f.raw(&bytes).0, 413);
    assert_eq!(f.bytes(), before);
    let t = task("task:after-size");
    f.ok(&f.normal, &Request::Create(t.clone()));
    f.ok(&f.normal, &task_read(&t));
    let f = Fixture::new(Config {
        response_bytes: 256,
        ..Config::default()
    });
    let t = task("task:response-bound");
    f.ok(&f.normal, &Request::Create(t.clone()));
    assert_eq!(f.request(&f.normal, &task_read(&t)).0, 413);
}
fn timeouts() {
    let f = Fixture::new(Config {
        request_timeout: Duration::from_millis(90),
        worker_interval: Duration::from_millis(1),
        ..Config::default()
    });
    let before = f.bytes();
    let mut stream = f.connect();
    let started = Instant::now();
    stream
        .write_all(b"POST /v1/request HTTP/1.1\r\nHost:")
        .unwrap();
    assert_eq!(response(&mut stream).0, 408);
    assert!(
        started.elapsed() >= Duration::from_millis(70),
        "deadline did not wait for incomplete ingress"
    );
    assert_eq!(f.bytes(), before);
    let t = task("task:timeout");
    let body = protocol::encode_request(&Request::Create(t.clone())).unwrap();
    let mut stream = f.connect();
    stream
        .write_all(f.header(&f.normal, body.len()).as_bytes())
        .unwrap();
    stream.write_all(&body[..1]).unwrap();
    assert_eq!(response(&mut stream).0, 408);
    assert_eq!(f.bytes(), before);
    f.ok(&f.normal, &Request::Create(t));
}
fn expiry_and_disconnect() {
    let f = Fixture::new(Config {
        session_lifetime: Duration::from_millis(45),
        worker_interval: Duration::from_millis(1),
        ..Config::default()
    });
    let before = f.bytes();
    thread::sleep(Duration::from_millis(60));
    assert_eq!(
        f.request(&f.normal, &Request::Create(task("task:expired")))
            .0,
        401
    );
    assert_eq!(f.bytes(), before);
    let f = Fixture::new(Config {
        session_lifetime: Duration::from_millis(120),
        request_timeout: Duration::from_millis(300),
        worker_interval: Duration::from_millis(1),
        ..Config::default()
    });
    let before = f.bytes();
    let body = protocol::encode_request(&Request::Create(task("task:expires-ingress"))).unwrap();
    let mut stream = f.connect();
    stream
        .write_all(f.header(&f.normal, body.len()).as_bytes())
        .unwrap();
    thread::sleep(Duration::from_millis(140));
    stream.write_all(&body).unwrap();
    assert_eq!(response(&mut stream).0, 401);
    assert_eq!(f.bytes(), before);
    let f = Fixture::new(Config::default());
    {
        let mut stream = f.connect();
        stream.write_all(b"POST /v1/request HTTP/1.1\r\n").unwrap();
    }
    let t = task("task:disconnect");
    f.ok(&f.normal, &Request::Create(t.clone()));
    f.ok(&f.normal, &task_read(&t));
}
fn cancellation() {
    let f = Fixture::new(Config {
        worker_interval: Duration::from_millis(100),
        ..Config::default()
    });
    let t = task("task:cancel");
    f.ok(&f.normal, &Request::Create(t.clone()));
    let id = execution(f.ok(
        &f.normal,
        &Request::Admit {
            task: t.clone(),
            proposal: support::proposal(&t, &[1; 10]),
            approvals: vec![],
        },
    ));
    assert_eq!(f.request(&f.normal, &Request::Cancel(id.clone())).0, 403);
    let Value::List(receipt) = f.ok(&f.human, &Request::Cancel(id.clone())) else {
        panic!("cancellation receipt")
    };
    assert_eq!(receipt[0], Value::Text("requested".into()));
    assert_eq!(receipt[3], Value::Text("principal:verified-fixture".into()));
    let WireRecord::Execution { execution, .. } = terminal(&f, &id) else {
        panic!("terminal execution")
    };
    assert_eq!(execution.fields().state, ExecutionState::Cancelled);
    assert!(execution.fields().effective_stop_evidence.is_some());
}
fn configuration() {
    for config in [
        Config {
            address: "0.0.0.0:0".parse().unwrap(),
            ..Config::default()
        },
        Config {
            address: "[::1]:0".parse().unwrap(),
            ..Config::default()
        },
        Config {
            request_timeout: Duration::ZERO,
            ..Config::default()
        },
        Config {
            sessions: 17,
            ..Config::default()
        },
    ] {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::env::temp_dir()
            .join(eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap());
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let storage = NativeStorage::create_new(&dir.join("journal"), sync).unwrap();
        let journal =
            Journal::create_new(storage, NativeHash, eylema_journal::Limits::default()).unwrap();
        let (host, owner) =
            DurableHost::create_new(journal, NativeServices, eylema_runtime::Limits::default())
                .unwrap();
        assert!(matches!(
            Server::bind(host, owner, config),
            Err(WebError::Configuration)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
fn enrollment_bound() {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir()
        .join(eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap());
    std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let path = dir.join("journal");
    let storage = NativeStorage::create_new(&path, sync).unwrap();
    let journal =
        Journal::create_new(storage, NativeHash, eylema_journal::Limits::default()).unwrap();
    let (host, owner) =
        DurableHost::create_new(journal, NativeServices, eylema_runtime::Limits::default())
            .unwrap();
    let mut server = Server::bind(
        host,
        owner,
        Config {
            sessions: 1,
            ..Config::default()
        },
    )
    .unwrap();
    server.register_session(support::grant()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(
        server.register_session(support::grant()),
        Err(WebError::Configuration)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    drop(server);
    std::fs::remove_dir_all(dir).unwrap();
}

pub fn run_native_proof() {
    positive();
    authority();
    malformed();
    sizes();
    timeouts();
    expiry_and_disconnect();
    cancellation();
    configuration();
    enrollment_bound();
}
#[test]
fn running_native_http_acceptance() {
    run_native_proof();
}
