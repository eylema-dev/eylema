//! Native CLI and bounded loopback client for the shared EYL transport.
#![forbid(unsafe_code)]

use eylema_contracts::{
    codec::{self, Value, WireRecord},
    transport::{self, Request},
    *,
};
use eylema_host::{
    DurableHost,
    native::{NativeHash, NativeServices},
};
use eylema_journal::{Journal, NativeStorage};
use eylema_runtime::{Grant, Limits as RuntimeLimits};
use eylema_web::{Config, Server};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[path = "human_proof.rs"]
mod human_proof;

const JOURNAL_FILE: &str = "journal.eylj";
const SESSION_FILE: &str = "session.txt";
const STOP_FILE: &str = "stop.request";
const MAX_REQUEST: usize = 64 * 1024;
const MAX_RESPONSE: usize = 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CliError(&'static str);
type Result<T> = std::result::Result<T, CliError>;

pub fn entry(arguments: impl IntoIterator<Item = OsString>) -> u8 {
    match run(arguments) {
        Ok(()) => 0,
        Err(error) => {
            println!("{{\"status\":\"error\",\"code\":\"{}\"}}", error.0);
            1
        }
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<()> {
    let args: Vec<OsString> = arguments.into_iter().collect();
    if args.is_empty() {
        print_help();
        return Ok(());
    }
    if args.iter().any(|arg| arg.to_str().is_none()) {
        return Err(CliError("INVALID_ARGUMENT"));
    }
    let command = args[0].to_str().ok_or(CliError("INVALID_ARGUMENT"))?;
    match command {
        "--help" | "help" => {
            if args.len() != 1 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            print_help();
            Ok(())
        }
        "--version" | "version" => {
            if args.len() != 1 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            println!("eylema {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "start" => {
            let human = args.len() == 2 && args[1].to_str() == Some("--human");
            if args.len() != 1 && !human {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            start_service(human)
        }
        "human-proof" => {
            if args.len() != 2 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            human_proof::run(Path::new(
                args[1].to_str().ok_or(CliError("INVALID_ARGUMENT"))?,
            ))
        }
        "doctor" => {
            if args.len() != 1 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            doctor()
        }
        "create" => {
            if args.len() != 2 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            create_task(Path::new(
                args[1].to_str().ok_or(CliError("INVALID_ARGUMENT"))?,
            ))
        }
        "run" => {
            if args.get(1).and_then(|x| x.to_str()) == Some("demo") {
                if args.len() > 3 {
                    return Err(CliError("INVALID_ARGUMENT"));
                }
                let delta = args
                    .get(2)
                    .and_then(|x| x.to_str())
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| CliError("INVALID_ARGUMENT"))
                    })
                    .transpose()?
                    .unwrap_or(1);
                return run_demo(delta);
            }
            if args.len() != 2 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            run_request(Path::new(
                args[1].to_str().ok_or(CliError("INVALID_ARGUMENT"))?,
            ))
        }
        "inspect" => {
            if args.len() != 3 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            inspect(
                args[1].to_str().ok_or(CliError("INVALID_ARGUMENT"))?,
                args[2].to_str().ok_or(CliError("INVALID_ARGUMENT"))?,
            )
        }
        "stop" if args.get(1).and_then(|x| x.to_str()) == Some("service") => {
            if args.len() != 2 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            stop_service()
        }
        "stop" => {
            if args.len() != 2 {
                return Err(CliError("INVALID_ARGUMENT"));
            }
            stop_execution(args[1].to_str().ok_or(CliError("INVALID_ARGUMENT"))?)
        }
        _ => Err(CliError("INVALID_ARGUMENT")),
    }
}

fn print_help() {
    println!(
        "Eylema native task CLI\nUsage: eylema <start|create|run|inspect|stop|doctor|--help|--version>\n  start [--human]       Start loopback service; optional OS-verified 5-minute stop session\n  create <request-file>  Submit an encoded EYL Create request\n  run demo [delta]       Run one bounded local-state demonstration\n  run <request-file>     Submit an encoded EYL Admit request\n  inspect <task|execution> <id>\n  stop <execution-id>    Request cancellation using the current session\n  stop service           Stop the local foreground service\n  doctor                 Check the local session and service connection\n  human-proof <new-dir>   Explicit OS-verified bounded acceptance demonstration"
    );
}

fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("EYLEMA_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or(CliError("HOME_UNAVAILABLE"))?;
    Ok(PathBuf::from(home).join("Library/Application Support/Eylema"))
}

fn ensure_private_dir(path: &Path) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(CliError("PLATFORM_UNSUPPORTED"));
    }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(CliError("UNSAFE_DATA_DIRECTORY"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700).recursive(true);
                builder
                    .create(path)
                    .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
            }
            #[cfg(not(unix))]
            return Err(CliError("PLATFORM_UNSUPPORTED"));
        }
        Err(_) => return Err(CliError("STORAGE_UNAVAILABLE")),
    }
    validate_private_dir(path)
}

fn validate_private_dir(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CliError("UNSAFE_DATA_DIRECTORY"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(CliError("UNSAFE_DATA_DIRECTORY"));
        }
    }
    Ok(())
}

fn full_sync(file: &File) -> std::io::Result<()> {
    eylema_platform::full_sync_file(file).map_err(std::io::Error::other)
}

fn open_host(
    path: &Path,
) -> Result<(
    DurableHost<NativeStorage, NativeHash, NativeServices>,
    eylema_host::Owner,
)> {
    ensure_private_dir(path)?;
    let journal_path = path.join(JOURNAL_FILE);
    let exists = match fs::symlink_metadata(&journal_path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(CliError("UNSAFE_JOURNAL"));
        }
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err(CliError("STORAGE_UNAVAILABLE")),
    };
    let storage = if exists {
        NativeStorage::open_existing(&journal_path, full_sync)
    } else {
        NativeStorage::create_new(&journal_path, full_sync)
    }
    .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    if exists {
        let journal = Journal::open(storage, NativeHash, eylema_journal::Limits::default())
            .map_err(|_| CliError("JOURNAL_INVALID"))?;
        DurableHost::open(journal, NativeServices).map_err(|_| CliError("JOURNAL_INVALID"))
    } else {
        let journal = Journal::create_new(storage, NativeHash, eylema_journal::Limits::default())
            .map_err(|_| CliError("JOURNAL_INVALID"))?;
        DurableHost::create_new(journal, NativeServices, RuntimeLimits::default())
            .map_err(|_| CliError("JOURNAL_INVALID"))
    }
}

fn cli_grant() -> Result<Grant> {
    Ok(Grant {
        principal: PrincipalRef::new("principal:local-cli")
            .map_err(|_| CliError("INTERNAL_ERROR"))?,
        resource_scope: ResourceScope::new(vec![
            ResourceId::new("state:cli").map_err(|_| CliError("INTERNAL_ERROR"))?,
        ])
        .map_err(|_| CliError("INTERNAL_ERROR"))?,
        permissions: vec![
            Permission::CreateTask,
            Permission::Read,
            Permission::ExecuteAction,
            Permission::ObserveEvents,
        ],
        policy_revision: Revision::new(1).map_err(|_| CliError("INTERNAL_ERROR"))?,
    })
}

fn write_session(path: &Path, origin: &str, bearer: &str, csrf: &str) -> Result<()> {
    let tmp = path.with_extension("session.tmp");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|_| CliError("SESSION_UNAVAILABLE"))?;
    writeln!(file, "{origin}\n{bearer}\n{csrf}").map_err(|_| CliError("SESSION_UNAVAILABLE"))?;
    file.sync_all()
        .map_err(|_| CliError("SESSION_UNAVAILABLE"))?;
    fs::rename(&tmp, path).map_err(|_| CliError("SESSION_UNAVAILABLE"))
}

fn start_service(human: bool) -> Result<()> {
    let directory = data_dir()?;
    let journal_path = directory.join(JOURNAL_FILE);
    let session_path = directory.join(SESSION_FILE);
    let stop_path = directory.join(STOP_FILE);
    let preferred = saved_service_address(&directory)?;
    let (host, owner) = open_host(&directory)?;
    // A successful exclusive journal open proves the previous service no longer owns this store.
    let _ = fs::remove_file(&session_path);
    let _ = fs::remove_file(directory.join("session.session.tmp"));
    let _ = fs::remove_file(&stop_path);
    let config = Config {
        address: preferred
            .filter(|address| std::net::TcpListener::bind(address).is_ok())
            .unwrap_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, 0))),
        ..Config::default()
    };
    let mut server =
        Server::bind(host, owner, config).map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    save_service_origin(&directory, server.origin())?;
    let credentials = if human {
        let mut grant = cli_grant()?;
        grant.permissions.push(Permission::RequestCancellation);
        server
            .register_device_owner(
                grant,
                eylema_platform::human::HumanPurpose::LocalCli,
                Duration::from_secs(300),
                Duration::from_secs(60),
            )
            .map_err(|_| CliError("HUMAN_ENROLLMENT_UNAVAILABLE"))?
    } else {
        server
            .register_session(cli_grant()?)
            .map_err(|_| CliError("SESSION_UNAVAILABLE"))?
    };
    let origin = server.origin().to_owned();
    write_session(
        &session_path,
        &origin,
        credentials.bearer(),
        credentials.csrf(),
    )?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = thread::Builder::new()
        .name("eylema-local-service".into())
        .spawn(move || server.run(&worker_stop))
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    println!(
        "{{\"status\":\"started\",\"origin\":\"{origin}\",\"data\":\"{}\"}}",
        json_escape(&directory.display().to_string())
    );
    while !stop_path.exists() {
        if worker.is_finished() {
            let _ = fs::remove_file(&session_path);
            return worker
                .join()
                .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?
                .map_err(|_| CliError("SERVICE_UNAVAILABLE"));
        }
        thread::sleep(Duration::from_millis(50));
    }
    stop.store(true, Ordering::Release);
    worker
        .join()
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    let _ = fs::remove_file(&session_path);
    let _ = fs::remove_file(&stop_path);
    let _ = journal_path;
    println!("{{\"status\":\"stopped\"}}");
    Ok(())
}

fn saved_service_address(directory: &Path) -> Result<Option<SocketAddr>> {
    let path = directory.join("origin.txt");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CliError("STORAGE_UNAVAILABLE")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 128 {
        return Err(CliError("UNSAFE_SESSION"));
    }
    let origin = fs::read_to_string(path).map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    let origin = origin.trim_end_matches(['\r', '\n']);
    if origin.contains('\n') || origin.contains('\r') {
        return Err(CliError("UNSAFE_SESSION"));
    }
    parse_origin(origin).map(|(address, _)| Some(address))
}

fn save_service_origin(directory: &Path, origin: &str) -> Result<()> {
    let path = directory.join("origin.txt");
    let tmp = directory.join("origin.tmp");
    let _ = fs::remove_file(&tmp);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    writeln!(file, "{origin}").map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    file.sync_all()
        .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    fs::rename(tmp, path).map_err(|_| CliError("STORAGE_UNAVAILABLE"))
}

fn stop_service() -> Result<()> {
    let directory = data_dir()?;
    validate_private_dir(&directory)?;
    let _ = load_session()?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(directory.join(STOP_FILE))
        .map_err(|_| CliError("SERVICE_NOT_RUNNING"))?;
    println!("{{\"status\":\"stop_requested\"}}");
    Ok(())
}

struct Session {
    origin: String,
    bearer: String,
    csrf: String,
}
fn load_session() -> Result<Session> {
    let path = data_dir()?.join(SESSION_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| CliError("SERVICE_NOT_RUNNING"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 2048 {
        return Err(CliError("UNSAFE_SESSION"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(CliError("UNSAFE_SESSION"));
        }
    }
    let file = File::open(&path).map_err(|_| CliError("SESSION_UNAVAILABLE"))?;
    if !file
        .metadata()
        .map_err(|_| CliError("UNSAFE_SESSION"))?
        .is_file()
    {
        return Err(CliError("UNSAFE_SESSION"));
    }
    let mut bytes = Vec::new();
    file.take(2049)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError("SESSION_UNAVAILABLE"))?;
    if bytes.len() > 2048 {
        return Err(CliError("UNSAFE_SESSION"));
    }
    let value = std::str::from_utf8(&bytes).map_err(|_| CliError("UNSAFE_SESSION"))?;
    let mut lines = value.lines();
    let origin = lines.next().ok_or(CliError("UNSAFE_SESSION"))?.to_owned();
    let bearer = lines.next().ok_or(CliError("UNSAFE_SESSION"))?.to_owned();
    let csrf = lines.next().ok_or(CliError("UNSAFE_SESSION"))?.to_owned();
    if lines.next().is_some()
        || bearer.len() != 64
        || csrf.len() != 64
        || !bearer.bytes().all(|x| x.is_ascii_hexdigit())
        || !csrf.bytes().all(|x| x.is_ascii_hexdigit())
    {
        return Err(CliError("UNSAFE_SESSION"));
    }
    Ok(Session {
        origin,
        bearer,
        csrf,
    })
}

fn parse_origin(origin: &str) -> Result<(SocketAddr, String)> {
    let authority = origin
        .strip_prefix("http://")
        .ok_or(CliError("UNSAFE_SESSION"))?;
    let address: SocketAddr = authority.parse().map_err(|_| CliError("UNSAFE_SESSION"))?;
    if address.ip() != Ipv4Addr::LOCALHOST {
        return Err(CliError("UNSAFE_SESSION"));
    }
    Ok((address, authority.to_owned()))
}

fn request(request: Request) -> Result<Value> {
    let session = load_session()?;
    let (address, authority) = parse_origin(&session.origin)?;
    let body = transport::encode_request(&request).map_err(|_| CliError("MALFORMED_REQUEST"))?;
    if body.len() > MAX_REQUEST {
        return Err(CliError("REQUEST_TOO_LARGE"));
    }
    let mut stream = TcpStream::connect_timeout(&address, CLIENT_TIMEOUT)
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    stream
        .set_read_timeout(Some(CLIENT_TIMEOUT))
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    stream
        .set_write_timeout(Some(CLIENT_TIMEOUT))
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    write!(stream, "POST /v1/request HTTP/1.1\r\nHost: {authority}\r\nOrigin: {}\r\nAuthorization: Bearer {}\r\nX-Eylema-CSRF: {}\r\nContent-Type: application/eylema\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", session.origin, session.bearer, session.csrf, body.len())
        .and_then(|_| stream.write_all(&body)).map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    let mut response = Vec::new();
    stream
        .take((MAX_RESPONSE + 4096) as u64)
        .read_to_end(&mut response)
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    if response.len() > MAX_RESPONSE + 4096 {
        return Err(CliError("RESPONSE_TOO_LARGE"));
    }
    let split = response
        .windows(4)
        .position(|x| x == b"\r\n\r\n")
        .ok_or(CliError("MALFORMED_RESPONSE"))?;
    let headers =
        std::str::from_utf8(&response[..split]).map_err(|_| CliError("MALFORMED_RESPONSE"))?;
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|x| x.parse::<u16>().ok())
        .ok_or(CliError("MALFORMED_RESPONSE"))?;
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|x| x.parse::<usize>().ok())
        .ok_or(CliError("MALFORMED_RESPONSE"))?;
    if length > MAX_RESPONSE || response.len() != split + 4 + length {
        return Err(CliError("MALFORMED_RESPONSE"));
    }
    let value =
        transport::decode_response(&response[split + 4..split + 4 + length]).map_err(|error| {
            match error {
                transport::TransportError::Remote(code) => CliError(remote_code(&code)),
                transport::TransportError::Malformed => CliError("MALFORMED_RESPONSE"),
            }
        })?;
    if !(200..300).contains(&status) {
        return Err(CliError("MALFORMED_RESPONSE"));
    }
    Ok(value)
}

fn remote_code(code: &str) -> &'static str {
    match code {
        "INVALID_REQUEST" => "INVALID_REQUEST",
        "FORBIDDEN" => "FORBIDDEN",
        "PRECONDITION_FAILED" => "PRECONDITION_FAILED",
        "APPROVAL_REQUIRED" => "APPROVAL_REQUIRED",
        "APPROVAL_EXPIRED" => "APPROVAL_EXPIRED",
        "APPROVAL_MISMATCH" => "APPROVAL_MISMATCH",
        "REVISION_CONFLICT" => "REVISION_CONFLICT",
        "CAPABILITY_UNAVAILABLE" => "CAPABILITY_UNAVAILABLE",
        "BUDGET_EXCEEDED" => "BUDGET_EXCEEDED",
        "DEADLINE_EXCEEDED" => "DEADLINE_EXCEEDED",
        "IDEMPOTENCY_CONFLICT" => "IDEMPOTENCY_CONFLICT",
        "CURSOR_EXPIRED" => "CURSOR_EXPIRED",
        "OUTCOME_UNKNOWN" => "OUTCOME_UNKNOWN",
        "REQUEST_TIMEOUT" => "REQUEST_TIMEOUT",
        "REQUEST_TOO_LARGE" => "REQUEST_TOO_LARGE",
        "MALFORMED_REQUEST" => "MALFORMED_REQUEST",
        "UNAUTHORIZED" => "UNAUTHORIZED",
        "UNSUPPORTED_REQUEST" => "UNSUPPORTED_REQUEST",
        "STORAGE_UNAVAILABLE" => "STORAGE_UNAVAILABLE",
        "TRANSPORT_UNAVAILABLE" => "TRANSPORT_UNAVAILABLE",
        "ENTROPY_UNAVAILABLE" => "ENTROPY_UNAVAILABLE",
        "INVALID_CONFIGURATION" => "INVALID_CONFIGURATION",
        _ => "SERVICE_ERROR",
    }
}

fn create_task(path: &Path) -> Result<()> {
    let bytes = read_request_file(path)?;
    let Request::Create(task) =
        transport::decode_request(&bytes).map_err(|_| CliError("MALFORMED_REQUEST"))?
    else {
        return Err(CliError("MALFORMED_REQUEST"));
    };
    request(Request::Create(task.clone()))?;
    println!(
        "{{\"status\":\"ok\",\"task_id\":\"{}\"}}",
        json_escape(task.fields().task_id.as_str())
    );
    Ok(())
}

fn run_request(path: &Path) -> Result<()> {
    let bytes = read_request_file(path)?;
    let Request::Admit { task, .. } =
        transport::decode_request(&bytes).map_err(|_| CliError("MALFORMED_REQUEST"))?
    else {
        return Err(CliError("MALFORMED_REQUEST"));
    };
    let value =
        request(transport::decode_request(&bytes).map_err(|_| CliError("MALFORMED_REQUEST"))?)?;
    let Value::Text(execution_id) = value else {
        return Err(CliError("MALFORMED_RESPONSE"));
    };
    println!(
        "{{\"status\":\"ok\",\"task_id\":\"{}\",\"execution_id\":\"{}\"}}",
        json_escape(task.fields().task_id.as_str()),
        json_escape(&execution_id)
    );
    Ok(())
}

fn run_demo(delta: u64) -> Result<()> {
    if !(1..=1_000_000).contains(&delta) {
        return Err(CliError("INVALID_REQUEST"));
    }
    let make_id =
        |kind| eylema_platform::random_id(kind).map_err(|_| CliError("ENTROPY_UNAVAILABLE"));
    let task_id = TaskId::new(
        make_id(eylema_platform::IdKind::Task).map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?,
    )
    .map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?;
    let context = RequestContext {
        request_id: RequestId::new(make_id(eylema_platform::IdKind::Request)?)
            .map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?,
        correlation_id: CorrelationId::new(make_id(eylema_platform::IdKind::Correlation)?)
            .map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?,
        contract_version: ContractVersion::CURRENT,
    };
    let scope = ResourceScope::new(vec![
        ResourceId::new("state:cli").map_err(|_| CliError("INTERNAL_ERROR"))?,
    ])
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    let revision = Revision::new(1).map_err(|_| CliError("INTERNAL_ERROR"))?;
    let deadline = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| CliError("CLOCK_UNAVAILABLE"))?
            .as_millis(),
    )
    .map_err(|_| CliError("CLOCK_UNAVAILABLE"))?;
    let budget = Budget::new(
        1,
        12,
        1024,
        1,
        Timestamp::new(deadline + 60_000).map_err(|_| CliError("CLOCK_UNAVAILABLE"))?,
    )
    .map_err(|_| CliError("INVALID_REQUEST"))?;
    let target = ExecutionTarget::new(
        ExecutionContext::NativeMacos,
        Backend::NativeCpu,
        GpuApi::None,
    )
    .map_err(|_| CliError("INVALID_REQUEST"))?;
    let task = TaskSpec::new(TaskDraft {
        context: context.clone(),
        task_id: task_id.clone(),
        revision,
        objective: "Advance a bounded local task value".into(),
        resource_scope: scope.clone(),
        budget,
        target,
    })
    .map_err(|_| CliError("INVALID_REQUEST"))?;
    request(Request::Create(task.clone()))?;
    let action = ActionSpec::new(ActionDraft {
        context: context.clone(),
        action_id: ActionId::new(make_id(eylema_platform::IdKind::Action)?)
            .map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?,
        task_id: task_id.clone(),
        operation_id: OperationId::new("addTaskValue").map_err(|_| CliError("INTERNAL_ERROR"))?,
        operation_version: ContractVersion::CURRENT,
        side_effect_class: SideEffectClass::RecordMutation,
        resource_scope: scope.clone(),
        expected_revision: Some(revision),
        authority: AuthorityRequirement::new(
            PrincipalRef::new("principal:local-cli").map_err(|_| CliError("INTERNAL_ERROR"))?,
            revision,
            scope,
            vec![Permission::ExecuteAction],
            OversightRequirements::new(vec![], true, false, None)
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
        )
        .map_err(|_| CliError("INVALID_REQUEST"))?,
        required_capabilities: vec![
            CapabilityId::new("task:state").map_err(|_| CliError("INTERNAL_ERROR"))?,
        ],
        budget,
        target,
        idempotency_key: IdempotencyKey::new(format!("{}:step:1", task_id.as_str()))
            .map_err(|_| CliError("INVALID_REQUEST"))?,
        payload: codec::encode(&Value::Unsigned(delta)).map_err(|_| CliError("INVALID_REQUEST"))?,
    })
    .map_err(|_| CliError("INVALID_REQUEST"))?;
    let proposal = ActionProposal::new(
        ProposalDraft {
            context,
            proposal_id: ProposalId::new(make_id(eylema_platform::IdKind::Proposal)?)
                .map_err(|_| CliError("ENTROPY_UNAVAILABLE"))?,
            task_id: task_id.clone(),
            plan_revision: revision,
            state_revision: revision,
            policy_revision: revision,
            actions: vec![action],
        },
        &task,
    )
    .map_err(|_| CliError("INVALID_REQUEST"))?;
    let execution = request(Request::Admit {
        task: task.clone(),
        proposal,
        approvals: vec![],
    })?;
    let Value::Text(execution_id) = execution else {
        return Err(CliError("MALFORMED_RESPONSE"));
    };
    println!(
        "{{\"status\":\"ok\",\"task_id\":\"{}\",\"execution_id\":\"{}\"}}",
        json_escape(task_id.as_str()),
        json_escape(&execution_id)
    );
    Ok(())
}

fn inspect(kind: &str, id: &str) -> Result<()> {
    match kind {
        "task" => {
            let id = TaskId::new(id.to_owned()).map_err(|_| CliError("INVALID_REQUEST"))?;
            let value = request(Request::Task(id))?;
            let Value::List(fields) = value else {
                return Err(CliError("MALFORMED_RESPONSE"));
            };
            if fields.len() != 5 {
                return Err(CliError("MALFORMED_RESPONSE"));
            }
            let Value::Bytes(record) = &fields[0] else {
                return Err(CliError("MALFORMED_RESPONSE"));
            };
            let WireRecord::Task(task) =
                codec::decode_record(record).map_err(|_| CliError("MALFORMED_RESPONSE"))?
            else {
                return Err(CliError("MALFORMED_RESPONSE"));
            };
            let number = |v: &Value| {
                if let Value::Unsigned(n) = v {
                    Ok(*n)
                } else {
                    Err(CliError("MALFORMED_RESPONSE"))
                }
            };
            println!(
                "{{\"status\":\"ok\",\"kind\":\"task\",\"task_id\":\"{}\",\"value\":{},\"performed_steps\":{},\"charged_steps\":{},\"charged_events\":{}}}",
                json_escape(task.fields().task_id.as_str()),
                number(&fields[1])?,
                number(&fields[2])?,
                number(&fields[3])?,
                number(&fields[4])?
            );
        }
        "execution" => {
            let id = ExecutionId::new(id.to_owned()).map_err(|_| CliError("INVALID_REQUEST"))?;
            let value = request(Request::Execution(id))?;
            let Value::Bytes(record) = value else {
                return Err(CliError("MALFORMED_RESPONSE"));
            };
            let WireRecord::Execution { execution, .. } =
                codec::decode_record(&record).map_err(|_| CliError("MALFORMED_RESPONSE"))?
            else {
                return Err(CliError("MALFORMED_RESPONSE"));
            };
            println!(
                "{{\"status\":\"ok\",\"kind\":\"execution\",\"execution_id\":\"{}\",\"task_id\":\"{}\",\"state\":\"{}\"}}",
                json_escape(execution.fields().execution_id.as_str()),
                json_escape(execution.fields().task_id.as_str()),
                match execution.fields().state {
                    ExecutionState::Accepted => "accepted",
                    ExecutionState::Queued => "queued",
                    ExecutionState::Running => "running",
                    ExecutionState::CancelRequested => "cancel_requested",
                    ExecutionState::Succeeded => "succeeded",
                    ExecutionState::Failed => "failed",
                    ExecutionState::Cancelled => "cancelled",
                    ExecutionState::OutcomeUnknown => "outcome_unknown",
                }
            );
        }
        _ => return Err(CliError("INVALID_ARGUMENT")),
    }
    Ok(())
}

fn stop_execution(raw_id: &str) -> Result<()> {
    let id = ExecutionId::new(raw_id.to_owned()).map_err(|_| CliError("INVALID_REQUEST"))?;
    let value = request(Request::Cancel(id.clone()))?;
    let Value::List(fields) = value else {
        return Err(CliError("MALFORMED_RESPONSE"));
    };
    if fields.len() < 2 {
        return Err(CliError("MALFORMED_RESPONSE"));
    }
    let Value::Text(outcome) = &fields[0] else {
        return Err(CliError("MALFORMED_RESPONSE"));
    };
    println!(
        "{{\"status\":\"ok\",\"execution_id\":\"{}\",\"outcome\":\"{}\"}}",
        json_escape(id.as_str()),
        json_escape(outcome)
    );
    Ok(())
}

fn read_request_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|_| CliError("INPUT_UNAVAILABLE"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CliError("INPUT_UNAVAILABLE"));
    }
    if metadata.len() as usize > MAX_REQUEST {
        return Err(CliError("REQUEST_TOO_LARGE"));
    }
    let file = File::open(path).map_err(|_| CliError("INPUT_UNAVAILABLE"))?;
    if !file
        .metadata()
        .map_err(|_| CliError("INPUT_UNAVAILABLE"))?
        .is_file()
    {
        return Err(CliError("INPUT_UNAVAILABLE"));
    }
    let mut bytes = Vec::new();
    file.take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError("INPUT_UNAVAILABLE"))?;
    if bytes.len() > MAX_REQUEST {
        return Err(CliError("REQUEST_TOO_LARGE"));
    }
    Ok(bytes)
}

fn doctor() -> Result<()> {
    validate_private_dir(&data_dir()?)?;
    let session = load_session()?;
    let (address, _) = parse_origin(&session.origin)?;
    TcpStream::connect_timeout(&address, Duration::from_millis(500))
        .map_err(|_| CliError("SERVICE_UNAVAILABLE"))?;
    match request(Request::Task(
        TaskId::new("task:doctor-probe").map_err(|_| CliError("INTERNAL_ERROR"))?,
    )) {
        Ok(_) | Err(CliError("INVALID_REQUEST")) => {}
        Err(error) => return Err(error),
    }
    println!(
        "{{\"status\":\"ok\",\"service\":\"reachable\",\"origin\":\"{}\"}}",
        json_escape(&session.origin)
    );
    Ok(())
}

fn json_escape(text: &str) -> String {
    text.chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            other if other.is_control() => "?".chars().collect(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_must_be_exact_loopback_http() {
        assert!(parse_origin("http://127.0.0.1:1234").is_ok());
        assert!(parse_origin("https://127.0.0.1:1234").is_err());
        assert!(parse_origin("http://0.0.0.0:1234").is_err());
        assert!(parse_origin("http://example.com:1234").is_err());
    }

    #[test]
    fn machine_json_escapes_control_characters() {
        assert_eq!(json_escape("a\nb\"c"), "a\\nb\\\"c");
    }

    #[test]
    fn service_error_vocabulary_preserves_contract_codes() {
        assert_eq!(remote_code("FORBIDDEN"), "FORBIDDEN");
        assert_eq!(remote_code("OUTCOME_UNKNOWN"), "OUTCOME_UNKNOWN");
        assert_eq!(remote_code("untrusted"), "SERVICE_ERROR");
    }
}
