#![cfg(target_os = "macos")]
use eylema_platform::process_group::OwnedGroup;
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn exited_leader_retains_ownership_until_lingering_pipe_child_is_stopped() {
    let mut unrelated = Command::new("/bin/sleep").arg("10").spawn().unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .args(["-c", "/bin/sleep 10 & exit 0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut group = OwnedGroup::spawn(&mut command).unwrap();
    let mut pipe = group.take_stdout().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        send.send(pipe.read_to_end(&mut data)).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = group.poll().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "owned leader must exit");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(status.success(), "leader exit status remains successful");
    assert!(
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_ok(),
        "descendant no longer retains stdout"
    );
    reader.join().unwrap();
    assert!(unrelated.try_wait().unwrap().is_none());
    assert!(
        group.stop().unwrap().is_none(),
        "stop is idempotent after reaping"
    );
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
}

#[test]
fn explicit_owned_group_stop_reaps_running_leader() {
    use std::os::unix::process::ExitStatusExt;
    let mut command = Command::new("/bin/sleep");
    command
        .arg("10")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut group = OwnedGroup::spawn(&mut command).unwrap();
    assert!(group.poll().unwrap().is_none());
    assert_eq!(group.stop().unwrap().unwrap().signal(), Some(9));
    assert!(group.poll().unwrap().is_none());
}

#[test]
fn naturally_completed_group_without_descendants_keeps_exit_status() {
    let mut command = Command::new("/usr/bin/true");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut group = OwnedGroup::spawn(&mut command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let result = loop {
        if let Some(status) = group.poll().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(result.success());
}
