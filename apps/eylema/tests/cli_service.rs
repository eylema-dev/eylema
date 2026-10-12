#![cfg(target_os = "macos")]

use eylema_contracts::{
    codec,
    transport::{self, Request},
    *,
};
use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn context() -> RequestContext {
    RequestContext {
        request_id: RequestId::new("request:cli-test").unwrap(),
        correlation_id: CorrelationId::new("trace:cli-test").unwrap(),
        contract_version: ContractVersion::CURRENT,
    }
}

fn task() -> TaskSpec {
    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 60_000;
    TaskSpec::new(TaskDraft {
        context: context(),
        task_id: TaskId::new("task:cli-test").unwrap(),
        revision: Revision::new(1).unwrap(),
        objective: "Exercise one bounded CLI service action".into(),
        resource_scope: ResourceScope::new(vec![ResourceId::new("state:cli").unwrap()]).unwrap(),
        budget: Budget::new(1, 12, 1024, 1, Timestamp::new(deadline).unwrap()).unwrap(),
        target: ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .unwrap(),
    })
    .unwrap()
}

fn proposal(task: &TaskSpec) -> ActionProposal {
    let scope = task.fields().resource_scope.clone();
    let action = ActionSpec::new(ActionDraft {
        context: context(),
        action_id: ActionId::new("action:cli-test").unwrap(),
        task_id: task.fields().task_id.clone(),
        operation_id: OperationId::new("addTaskValue").unwrap(),
        operation_version: ContractVersion::CURRENT,
        side_effect_class: SideEffectClass::RecordMutation,
        resource_scope: scope.clone(),
        expected_revision: Some(task.fields().revision),
        authority: AuthorityRequirement::new(
            PrincipalRef::new("principal:local-cli").unwrap(),
            Revision::new(1).unwrap(),
            scope,
            vec![Permission::ExecuteAction],
            OversightRequirements::new(vec![], true, false, None).unwrap(),
        )
        .unwrap(),
        required_capabilities: vec![CapabilityId::new("task:state").unwrap()],
        budget: task.fields().budget,
        target: task.fields().target,
        idempotency_key: IdempotencyKey::new("task:cli-test:step:1").unwrap(),
        payload: codec::encode(&codec::Value::Unsigned(7)).unwrap(),
    })
    .unwrap();
    ActionProposal::new(
        ProposalDraft {
            context: context(),
            proposal_id: ProposalId::new("proposal:cli-test").unwrap(),
            task_id: task.fields().task_id.clone(),
            plan_revision: Revision::new(1).unwrap(),
            state_revision: task.fields().revision,
            policy_revision: Revision::new(1).unwrap(),
            actions: vec![action],
        },
        task,
    )
    .unwrap()
}

fn cli(data: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_eylema"))
        .args(args)
        .env("EYLEMA_DATA_DIR", data)
        .output()
        .unwrap()
}

#[test]
fn native_cli_and_http_service_share_ids_codes_and_private_session_custody() {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let suffix = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
    let root = std::env::temp_dir().join(format!("eylema-cli-{suffix}"));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();

    let mut service = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .arg("start")
        .env("EYLEMA_DATA_DIR", &root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut ready = false;
    for _ in 0..60 {
        let output = cli(&root, &["doctor"]);
        if output.status.success() {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        ready,
        "doctor should connect to the actual owned loopback service"
    );
    let session = root.join("session.txt");
    let metadata = fs::metadata(&session).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    let task = task();
    let create_path = root.join("create.eyl");
    fs::write(
        &create_path,
        transport::encode_request(&Request::Create(task.clone())).unwrap(),
    )
    .unwrap();
    let created = cli(&root, &["create", create_path.to_str().unwrap()]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stdout)
    );
    assert!(
        String::from_utf8(created.stdout)
            .unwrap()
            .contains("\"task_id\":\"task:cli-test\"")
    );

    let admit = Request::Admit {
        task: task.clone(),
        proposal: proposal(&task),
        approvals: vec![],
    };
    let admit_path = root.join("run.eyl");
    fs::write(&admit_path, transport::encode_request(&admit).unwrap()).unwrap();
    let ran = cli(&root, &["run", admit_path.to_str().unwrap()]);
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stdout)
    );
    let run_output = String::from_utf8(ran.stdout).unwrap();
    assert!(run_output.contains("\"task_id\":\"task:cli-test\""));
    let execution_id = run_output
        .split("\"execution_id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();

    let inspected_task = cli(&root, &["inspect", "task", "task:cli-test"]);
    assert!(
        inspected_task.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected_task.stdout)
    );
    assert!(
        String::from_utf8(inspected_task.stdout)
            .unwrap()
            .contains("\"value\":7")
    );
    let inspected_execution = cli(&root, &["inspect", "execution", &execution_id]);
    assert!(inspected_execution.status.success());
    let inspection = String::from_utf8(inspected_execution.stdout).unwrap();
    assert!(inspection.contains(&execution_id));
    assert!(inspection.contains("succeeded"));
    let oversized = root.join("oversized.eyl");
    fs::write(&oversized, vec![0u8; 64 * 1024 + 1]).unwrap();
    let oversized_request = cli(&root, &["create", oversized.to_str().unwrap()]);
    assert_eq!(oversized_request.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(oversized_request.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"REQUEST_TOO_LARGE\"}\n"
    );
    let missing = cli(&root, &["inspect", "execution", "execution:not-present"]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(missing.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"INVALID_REQUEST\"}\n"
    );

    let demo = cli(&root, &["run", "demo", "7"]);
    assert!(
        demo.status.success(),
        "{}",
        String::from_utf8_lossy(&demo.stdout)
    );
    let demo_output = String::from_utf8(demo.stdout).unwrap();
    assert!(demo_output.contains("\"task_id\":\"task:"));
    assert!(demo_output.contains("\"execution_id\":\"execution:"));
    let demo_task = demo_output
        .split("\"task_id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let demo_execution = demo_output
        .split("\"execution_id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let demo_inspection = cli(&root, &["inspect", "task", &demo_task]);
    assert!(demo_inspection.status.success());
    assert!(
        String::from_utf8(demo_inspection.stdout)
            .unwrap()
            .contains("\"value\":7")
    );
    let rejected_demo = cli(&root, &["run", "demo", "0"]);
    assert_eq!(rejected_demo.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(rejected_demo.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"INVALID_REQUEST\"}\n"
    );

    // The non-human CLI session never claims cancellation authority.
    let denied = cli(&root, &["stop", &demo_execution]);
    assert_eq!(denied.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(denied.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"FORBIDDEN\"}\n"
    );

    let stopped = cli(&root, &["stop", "service"]);
    assert!(stopped.status.success());
    for _ in 0..100 {
        if service.try_wait().unwrap().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        service.try_wait().unwrap().is_some(),
        "service stop request should join the worker cleanly"
    );
    assert!(!session.exists());
    let stopped_doctor = cli(&root, &["doctor"]);
    assert_eq!(stopped_doctor.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(stopped_doctor.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"SERVICE_NOT_RUNNING\"}\n"
    );

    let mut reopened = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .arg("start")
        .env("EYLEMA_DATA_DIR", &root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut ready_again = false;
    for _ in 0..60 {
        if cli(&root, &["doctor"]).status.success() {
            ready_again = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(ready_again, "reopened service should pass doctor");
    let recovered = cli(&root, &["inspect", "execution", &execution_id]);
    assert!(recovered.status.success());
    assert!(
        String::from_utf8(recovered.stdout)
            .unwrap()
            .contains(&execution_id)
    );
    assert!(
        cli(&root, &["inspect", "task", &demo_task])
            .status
            .success()
    );
    assert!(cli(&root, &["stop", "service"]).status.success());
    for _ in 0..100 {
        if reopened.try_wait().unwrap().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(reopened.try_wait().unwrap().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_stop_succeeds_only_with_host_verified_human_session() {
    use eylema_host::{
        DurableHost,
        native::{NativeHash, NativeServices},
    };
    use eylema_journal::{Journal, NativeStorage};
    use eylema_runtime::{Grant, Limits as RuntimeLimits};
    use eylema_web::{Config, Server};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    use std::{
        fs::File,
        io::Write,
        net::Ipv4Addr,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    fn sync(file: &File) -> std::io::Result<()> {
        eylema_platform::full_sync_file(file).map_err(std::io::Error::other)
    }
    let suffix = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
    let root = std::env::temp_dir().join(format!("eylema-cli-human-{suffix}"));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let storage = NativeStorage::create_new(&root.join("journal.eylj"), sync).unwrap();
    let journal =
        Journal::create_new(storage, NativeHash, eylema_journal::Limits::default()).unwrap();
    let (host, owner) =
        DurableHost::create_new(journal, NativeServices, RuntimeLimits::default()).unwrap();
    let mut server = Server::bind(
        host,
        owner,
        Config {
            address: (Ipv4Addr::LOCALHOST, 0).into(),
            ..Config::default()
        },
    )
    .unwrap();
    let credentials = server
        .register_verified_human(Grant {
            principal: PrincipalRef::new("principal:local-cli").unwrap(),
            resource_scope: ResourceScope::new(vec![ResourceId::new("state:cli").unwrap()])
                .unwrap(),
            permissions: vec![
                Permission::CreateTask,
                Permission::Read,
                Permission::ExecuteAction,
                Permission::ObserveEvents,
                Permission::RequestCancellation,
            ],
            policy_revision: Revision::new(1).unwrap(),
        })
        .unwrap();
    let session_path = root.join("session.txt");
    let mut session_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&session_path)
        .unwrap();
    writeln!(
        session_file,
        "{}\n{}\n{}",
        server.origin(),
        credentials.bearer(),
        credentials.csrf()
    )
    .unwrap();
    session_file.sync_all().unwrap();
    drop(session_file);

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = std::thread::spawn(move || server.run(&worker_stop));

    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 60_000;
    let task = TaskSpec::new(TaskDraft {
        context: context(),
        task_id: TaskId::new("task:human-stop").unwrap(),
        revision: Revision::new(1).unwrap(),
        objective: "Remain queued long enough to exercise verified CLI stop".into(),
        resource_scope: ResourceScope::new(vec![ResourceId::new("state:cli").unwrap()]).unwrap(),
        budget: Budget::new(16, 96, 2048, 1, Timestamp::new(deadline).unwrap()).unwrap(),
        target: ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .unwrap(),
    })
    .unwrap();
    let scope = task.fields().resource_scope.clone();
    let actions = (0..16)
        .map(|index| {
            ActionSpec::new(ActionDraft {
                context: context(),
                action_id: ActionId::new(format!("action:human-stop-{index}")).unwrap(),
                task_id: task.fields().task_id.clone(),
                operation_id: OperationId::new("addTaskValue").unwrap(),
                operation_version: ContractVersion::CURRENT,
                side_effect_class: SideEffectClass::RecordMutation,
                resource_scope: scope.clone(),
                expected_revision: Some(task.fields().revision),
                authority: AuthorityRequirement::new(
                    PrincipalRef::new("principal:local-cli").unwrap(),
                    Revision::new(1).unwrap(),
                    scope.clone(),
                    vec![Permission::ExecuteAction],
                    OversightRequirements::new(vec![], true, false, None).unwrap(),
                )
                .unwrap(),
                required_capabilities: vec![CapabilityId::new("task:state").unwrap()],
                budget: task.fields().budget,
                target: task.fields().target,
                idempotency_key: IdempotencyKey::new(format!("task:human-stop:step:{index}"))
                    .unwrap(),
                payload: codec::encode(&codec::Value::Unsigned(1)).unwrap(),
            })
            .unwrap()
        })
        .collect();
    let proposal = ActionProposal::new(
        ProposalDraft {
            context: context(),
            proposal_id: ProposalId::new("proposal:human-stop").unwrap(),
            task_id: task.fields().task_id.clone(),
            plan_revision: Revision::new(1).unwrap(),
            state_revision: task.fields().revision,
            policy_revision: Revision::new(1).unwrap(),
            actions,
        },
        &task,
    )
    .unwrap();

    let create_file = root.join("create.eyl");
    fs::write(
        &create_file,
        transport::encode_request(&Request::Create(task.clone())).unwrap(),
    )
    .unwrap();
    assert!(
        cli(&root, &["create", create_file.to_str().unwrap()])
            .status
            .success()
    );
    let admit_file = root.join("admit.eyl");
    fs::write(
        &admit_file,
        transport::encode_request(&Request::Admit {
            task: task.clone(),
            proposal,
            approvals: vec![],
        })
        .unwrap(),
    )
    .unwrap();
    let admitted = cli(&root, &["run", admit_file.to_str().unwrap()]);
    assert!(
        admitted.status.success(),
        "{}",
        String::from_utf8_lossy(&admitted.stdout)
    );
    let admitted = String::from_utf8(admitted.stdout).unwrap();
    let execution_id = admitted
        .split("\"execution_id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();

    let stopped = cli(&root, &["stop", &execution_id]);
    assert!(
        stopped.status.success(),
        "{}",
        String::from_utf8_lossy(&stopped.stdout)
    );
    assert!(
        String::from_utf8(stopped.stdout)
            .unwrap()
            .contains("\"outcome\":\"requested\"")
    );
    let mut cancelled = false;
    for _ in 0..40 {
        let output = cli(&root, &["inspect", "execution", &execution_id]);
        if String::from_utf8_lossy(&output.stdout).contains("\"state\":\"cancelled\"") {
            cancelled = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        cancelled,
        "verified request should become an effective cancellation"
    );

    stop.store(true, Ordering::Release);
    worker.join().unwrap().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn doctor_does_not_create_or_repair_a_missing_data_directory() {
    use std::os::unix::fs::DirBuilderExt;
    let suffix = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
    let root = std::env::temp_dir().join(format!("eylema-cli-doctor-{suffix}"));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let absent = root.join("not-created");
    let output = cli(&absent, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "{\"status\":\"error\",\"code\":\"STORAGE_UNAVAILABLE\"}\n"
    );
    assert!(!absent.exists());
    fs::remove_dir_all(root).unwrap();
}
