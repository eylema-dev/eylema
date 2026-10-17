//! Same-binary worker and bounded private pipes. No tool or human authority crosses IPC.
use super::{
    models::{self, AssetPaths, ModelReply, ModelRequest},
    types::*,
    wire::{self, FrameBudget},
};
use eylema_contracts::codec::{self, Value};
use eylema_model::cpu::CpuWorkGroup;
use std::{
    io,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
pub fn validate_envelope(v: &Value, nonce: [u8; 32], sequence: u64) -> Result<&Value> {
    let v = wire::list(v)?;
    if v.len() != 3 || wire::hash(&v[0])? != nonce || wire::number(&v[1])? != sequence {
        return Err(ReportError::InvalidWitness);
    }
    Ok(&v[2])
}
fn encoded(v: &Value) -> Result<Vec<u8>> {
    let b = codec::encode(v).map_err(|_| ReportError::InvalidInput)?;
    wire::decode_closed(&b)?;
    Ok(b)
}
pub struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<Result<Value>>,
    frames: FrameBudget,
    nonce: [u8; 32],
    sequence: u64,
    deadline: Instant,
    poisoned: bool,
    traffic: usize,
    messages: u32,
}
impl Worker {
    pub fn spawn(assets: &AssetPaths, root_work: u64, root_deadline: Instant) -> Result<Self> {
        if !matches!(root_work, 100_000_000_000 | 2_500_000_000_000)
            || root_deadline <= Instant::now()
        {
            return Err(ReportError::Limit);
        }
        let remaining = root_deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(180));
        let id = eylema_platform::random_id(eylema_platform::IdKind::Request)
            .map_err(|_| ReportError::Unsupported)?;
        let nonce = eylema_platform::sha256(id.as_bytes()).map_err(|_| ReportError::Unsupported)?;
        let mut child =
            Command::new(std::env::current_exe().map_err(|_| ReportError::Unsupported)?)
                .arg("report-worker")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| ReportError::WorkerFailed)?;
        let Some(input) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ReportError::WorkerFailed);
        };
        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ReportError::WorkerFailed);
        };
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut budget = FrameBudget::default();
            loop {
                let reply = budget.read(&mut stdout);
                let failed = reply.is_err();
                if tx.send(reply).is_err() || failed {
                    break;
                }
            }
        });
        let mut worker = Self {
            child,
            input: Some(input),
            output: rx,
            frames: FrameBudget::default(),
            nonce,
            sequence: 0,
            deadline: Instant::now() + remaining,
            poisoned: false,
            traffic: 0,
            messages: 0,
        };
        let setup = Value::List(vec![
            Value::Text("report-worker:v1".into()),
            Value::Bytes(nonce.to_vec()),
            assets.value()?,
            Value::Unsigned(root_work),
            Value::Unsigned(remaining.as_millis() as u64),
        ]);
        worker.send(&setup)?;
        Ok(worker)
    }
    fn account(&mut self, n: usize) -> Result<()> {
        self.traffic = self.traffic.checked_add(n + 4).ok_or(ReportError::Limit)?;
        self.messages += 1;
        if self.traffic > 1048576 || self.messages > 256 {
            return Err(ReportError::Limit);
        }
        Ok(())
    }
    fn send(&mut self, value: &Value) -> Result<()> {
        let bytes = encoded(value)?;
        self.account(bytes.len())?;
        self.frames.write(
            self.input.as_mut().ok_or(ReportError::WorkerFailed)?,
            &bytes,
        )
    }
    pub fn dispatch(
        &mut self,
        request: &ModelRequest,
        ledger: &mut JobLedger,
        allocation: u64,
    ) -> Result<Vec<ModelReply>> {
        if self.poisoned || Instant::now() >= self.deadline {
            return Err(ReportError::Deadline);
        }
        request.validate()?;
        ledger.reserve(allocation)?;
        self.poisoned = true;
        self.sequence += 1;
        self.send(&Value::List(vec![
            Value::Bytes(self.nonce.to_vec()),
            Value::Unsigned(self.sequence),
            Value::List(vec![Value::Unsigned(allocation), request.value()]),
        ]))?;
        let mut replies = Vec::new();
        for _ in 0..request.count() {
            let v = self
                .output
                .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| ReportError::Deadline)??;
            self.account(encoded(&v)?.len())?;
            let fields = wire::list(validate_envelope(&v, self.nonce, self.sequence)?)?;
            if fields.len() != 2 {
                return Err(ReportError::WorkerFailed);
            }
            let receipt = wire::list(&fields[1])?;
            if receipt.len() != 7
                || wire::hash(&receipt[3])?
                    != eylema_platform::sha256(&encoded(&fields[0])?)
                        .map_err(|_| ReportError::Unsupported)?
            {
                return Err(ReportError::InvalidWitness);
            }
            replies.push(ModelReply {
                output: fields[0].clone(),
                receipt: fields[1].clone(),
            });
        }
        self.poisoned = false;
        Ok(replies)
    }
    pub fn cancel_and_wait(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .map_err(|_| ReportError::WorkerFailed)?
            .is_some()
        {
            self.poisoned = true;
            return Ok(());
        }
        self.sequence += 1;
        let _ = self.send(&Value::List(vec![
            Value::Bytes(self.nonce.to_vec()),
            Value::Unsigned(self.sequence),
            Value::Null,
        ]));
        self.input.take();
        let grace = (Instant::now() + Duration::from_secs(1)).min(self.deadline);
        while Instant::now() < grace {
            if self
                .child
                .try_wait()
                .map_err(|_| ReportError::WorkerFailed)?
                .is_some()
            {
                self.poisoned = true;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.child.kill().map_err(|_| ReportError::WorkerFailed)?;
        self.child.wait().map_err(|_| ReportError::WorkerFailed)?;
        self.poisoned = true;
        Err(ReportError::OutcomeUnknown)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.cancel_and_wait();
    }
}
pub fn serve() -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut incoming = FrameBudget::default();
    let mut outgoing = FrameBudget::default();
    let setup = incoming.read(&mut input)?;
    let fields = wire::list(&setup)?;
    if fields.len() != 5 || wire::text(&fields[0])? != "report-worker:v1" {
        return Err(ReportError::InvalidInput);
    }
    let nonce = wire::hash(&fields[1])?;
    let assets = AssetPaths::from_value(&fields[2])?;
    let root = wire::number(&fields[3])?;
    let ms = wire::number(&fields[4])?;
    if !matches!(root, 100_000_000_000 | 2_500_000_000_000) || ms == 0 || ms > 180000 {
        return Err(ReportError::Limit);
    }
    let deadline = Instant::now() + Duration::from_millis(ms);
    let mut ledger = JobLedger::new(root)?;
    drop(input);
    let controls = super::control::Controls::default();
    let reader_controls = controls.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut sequence = 0;
        loop {
            let packet = incoming.read(&mut input).and_then(|packet| {
                sequence += 1;
                let body = validate_envelope(&packet, nonce, sequence)?;
                if matches!(body, Value::Null) {
                    reader_controls.stop();
                }
                Ok(packet)
            });
            let failed = packet.is_err();
            if failed {
                reader_controls.stop();
            }
            if tx.send(packet).is_err() || failed {
                break;
            }
        }
    });
    let mut sequence = 0;
    let mut embeddings = 0;
    let mut plans = 0;
    let mut decisions = 0;
    loop {
        if Instant::now() >= deadline {
            return Err(ReportError::Deadline);
        }
        let packet = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| ReportError::Deadline)??;
        sequence += 1;
        let body = validate_envelope(&packet, nonce, sequence)?;
        if matches!(body, Value::Null) {
            return Ok(());
        }
        controls.check()?;
        let dispatch = wire::list(body)?;
        if dispatch.len() != 2 {
            return Err(ReportError::InvalidInput);
        }
        let allocation = wire::number(&dispatch[0])?;
        ledger.reserve(allocation)?;
        let group = CpuWorkGroup::new(
            allocation
                .checked_sub(300_000_000)
                .ok_or(ReportError::Limit)?,
        )
        .map_err(|_| ReportError::Limit)?;
        let request = ModelRequest::from_value(&dispatch[1])?;
        match &request {
            ModelRequest::Documents(x) | ModelRequest::Queries(x) => {
                embeddings += x.len();
                if embeddings > 9 || root == 100_000_000_000 {
                    return Err(ReportError::Limit);
                }
            }
            ModelRequest::Plan(x) => {
                plans += x.len();
                if plans > if root == 100_000_000_000 { 1 } else { 2 } {
                    return Err(ReportError::Limit);
                }
            }
            ModelRequest::Decide(_) => {
                decisions += 1;
                if decisions > 2 || root == 100_000_000_000 {
                    return Err(ReportError::Limit);
                }
            }
        }
        let replies = models::compute(&request, &assets, &group, deadline, &controls)?;
        for reply in replies {
            let packet = Value::List(vec![
                Value::Bytes(nonce.to_vec()),
                Value::Unsigned(sequence),
                Value::List(vec![reply.output, reply.receipt]),
            ]);
            outgoing.write(&mut output, &encoded(&packet)?)?;
        }
    }
}
