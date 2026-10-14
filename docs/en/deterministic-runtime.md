# G10 — Deterministic embedded executor

[Türkçe](../tr/deterministic-runtime.md). Runtime contract0.1.0, unpublished package0.1.0. [Plan](../../tools/work-plans/g10.json), [issue12](https://github.com/eyleme-dev/eyleme/issues/12), [codec/platform](codec-platform.md).

The owned safe [runtime](../../crates/eylema-runtime/src/lib.rs) supplies in-process task admission, a FIFO queue, bounded single-step execution, snapshots, observations and events. Its production dependency is only eylema-contracts; the owned platform library is a development dependency for actual native consumers/tests. It has no server, production background worker, arbitrary skill loading, external effect, journal or browser. The host executable's help/version remains bootstrap behavior; the separate native example exercises this actual library.

## Host authority and supported effects

Runtime::new returns Runtime and a private, nonserialized HostKey. Only the matching key can enroll a bounded Grant (verified-by-the-trusted-embedding-host principal, resource scope, permissions, policy revision), returning an opaque Session. Safe Arc identity binds these handles to one runtime; another runtime's key/session fails. Rust request principal/authority reference strings never mint a session. This trusted embedded capability boundary is not network authentication or a human verification method; G14 supplies the service's local session boundary.

Admission and every step recheck the session, scoped ExecuteAction permission, declared principal/policy and every requested permission. readTaskValue independently requires Read even if a request omits that requirement. Required observation needs ObserveEvents. Required checkpoints return APPROVAL_REQUIRED; required intervention/governance returns CAPABILITY_UNAVAILABLE until G11 supplies proof/behavior. References alone are never accepted as approval. Supported execution is native macOS CPU with task:state capability. Unknown operations/capabilities, GPU/browser targets and inconsistent effect kinds/arguments fail before an effect.

| Operation | Argument and effect |
|---|---|
| readTaskValue | Empty payload, Read class; observes owned value, records NotApplied for mutation |
| addTaskValue | Canonical EYL unsigned u64, RecordMutation class; checked addition to owned task value |

Task creation initializes value0. Each successful add advances the live TaskSpec revision by one; overflow fails before that step's mutation, preserving previous confirmed effects. One active execution leases its task. Its immutable proposal/approval base remains the admitted snapshot; its internal expected live revision advances only with its own observed mutations. New stale submissions fail REVISION_CONFLICT. Per-step observed revision/lease checks prevent silent rebinding; the current API offers no external task writer. ExecutionRecord state_revision denotes the plan base; events/task snapshots report observed live revision. A failed plan releases the lease but preserves partial value/revision/evidence.

## Admission, replay and resource bounds

Canonical approval bytes from G09 bind effect fields while excluding transport contexts. Idempotency is keyed by principal, operation, resource IDs and idempotency key, with host-supplied standard SHA-256 of that canonical declaration. Identical replay returns the same queued/completed execution before new admission, including after its deadline; it does no new effect. Changed content or a partially reused batch conflicts. Proposal ID + plan revision also cannot be rebound to different canonical bytes using fresh idempotency keys. New plan/state versions need fresh appropriate bindings.

Records and replay references remain for the entire process lifetime within finite capacity; there is no eviction, automatic retry, restart retention, durable acknowledgement or external exactly-once claim. When capacity is full, new admission fails BUDGET_EXCEEDED; do not silently create another operation after unknown process loss. G12/G13 implement persistence/recovery separately.

Defaults are structural limits, not measured performance:64 tasks,128 executions,16 sessions,32 queued executions,4096 event slots,4096 replay keys and16MiB resident wire accounting. Configured count limits must be1..65536; resident accounting1byte..1GiB. Task/serialized proposal bytes are charged separately; metadata/allocator overhead is bounded by counts but not measured by this wire budget. No runtime-source Wasm claim is supplied; portable selection remains contracts/platform.

Task step/event budgets are cumulative. Admission charges all planned steps plus N+3 event slots (accepted, running, N observations, terminal); task creation charges one event. Unused reservations after failure are retained conservatively, without automatic refunds. Performed steps are reported separately. The single worker provides concurrency1; a task lease also prevents overlapping plans for that task. Rejecting admission commits no task/queue/ledger/event state; fallible host time/hash/ID calls can have occurred. IDs and terminal capacity are reserved before effects; duplicate source IDs fail. Deadline is checked before admission and again after metadata reservations, and before each step. Clock failure leaves queued work unchanged. These cooperative checks do not prove hard realtime interruption or physical termination. Deadline failure has no effective-stop evidence.

## Observations and actual evidence

Every event's evidence reference resolves to an owned Observation containing its event/task/execution ID, observed timestamp and before/after value. These are actual observations of synchronous owned memory, not external-effect evidence. Event/snapshot reads enforce scoped Read/ObserveEvents. Events are ordered in the retained task stream; bounded limits1..256 and unknown cursor CURSOR_EXPIRED are explicit. Completed/failed snapshots use G08 constructors; admission is Queued, not success. Effects are independently Applied/NotApplied; terminal results retain correlation and partial observations.

Local macOS27.0.1 ARM64/M4Pro24GiB, Rust1.99.0, Python3.14.8:

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG cargo run --package eylema-runtime --example executor-proof --locked --offline
```

Passed: unchanged Rust guard/inventory/fmt/clippy/native45tests/build and selected two portable-library Wasm compilation. Fourteen new executor regression tests cover success, overflow/partial effects, cross-runtime/claimed authority, denied permission/checkpoint/intervention/governance/observation, replay/conflict, deadline at admission/step/after host reservation, FIFO/lease/stale state/policy/target/capability, resource budgets, invalid config/session/ID/cursor, clock failure and immutable proposal-revision identity. Deterministic clock/ID fixtures are development tests, not OS clock or entropy execution proof; fixture hashing uses actual native CommonCrypto.

The [actual native consumer](../../crates/eylema-runtime/examples/executor-proof.rs) uses SystemTime, CommonCrypto/Security services, a host-enrolled scoped session, real queued/running/succeeded states, two steps0→2→5, six observed events and identical queued/completed replay. This native execution is separate from document CI. Development compile/style failures were corrected without weakening guards; current passing checks bind the delivered revision. Actual human approval/revocation/effective cancellation, journal/restart/HTTP/CLI/browser acceptance, GPU/model/training and performance: not_run, later Goals. No publication or license decision is supplied.

G10 statements about missing approval/revocation/cancellation describe its accepted revision. [G11 human oversight](human-oversight.md) now extends this native core with retained decisions and truthful cooperative cancellation; its evidence and remaining authentication/host/browser boundaries are separate.

G54 adds [typed tool selection and source-gated execution](typed-tools.md) over these same two handlers. Its sealed facade retains source restrictions for cumulative values and rejects recovered hosts without durable typed lineage; the earlier executor interfaces are not a privacy bypass for that facade.
