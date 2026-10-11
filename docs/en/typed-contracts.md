# G08 — Shared typed task contracts

English is canonical. [Türkçe](../tr/typed-contracts.md). Runtime contract **0.1.0**; package build identity and model-set schema versions are independent.

The owned [portable library](../../crates/eylema-contracts/src/lib.rs) now defines validated task, action, proposal, execution snapshot, effect, event, authority requirement, composed oversight, budget and error values. Editable drafts become private validated values through fallible constructors. Immutable field access cannot bypass construction. The native host's [consumer test](../../apps/eylema/tests/contracts.rs) uses the same public types and rejection semantics as the [contract tests](../../crates/eylema-contracts/tests/contract_validation.rs). The host command still provides bootstrap help/version only.

Exact runtime version 0.1.0 is accepted; older, newer minor/patch and major versions fail until a reviewed compatibility change. IDs are distinct Rust types and bounded opaque ASCII strings, with no path separators, whitespace or wildcards. They are not generated globally unique IDs or filesystem paths. Positive revisions and Unix-millisecond timestamps remain declared values; construction does not consult a clock. Human messages support English/Turkish UTF-8 under byte limits. Thirteen language-neutral API error codes retain their English identifiers; retry delays require an explicitly retryable error.

Structural limits are 96 bytes per ID, 4096 bytes per human text, 64 references per bounded reference list, 1024 actions per proposal and at most 1 MiB declared payload. These are protocol input bounds, not measured performance or device-capacity guarantees. Budgets require positive finite step/event/payload/concurrency values and an explicit deadline. Proposals bind task/state/policy/plan identities, unique actions, exact resource subsets and requested execution targets; action declarations cannot exceed the parent budget or aggregate payload limit. Non-read task actions require a state precondition. Backend pairs permit native CPU/no GPU, native GPU/Metal, browser Wasm CPU/no GPU and browser WebGPU/WebGPU; those declarations do not establish availability or execution.

Authority values describe required permissions, resource scope, a principal reference and policy revision. Checkpoints, observation, intervention and governance references compose independently. A principal name, capability, policy reference or validated value grants no permission. Authentication, policy resolution, delegation, expiry/revocation and admission must be implemented and verified in later Goals. No approval decision or cryptographic payload trust is implemented here.

Execution snapshots bind the exact proposal/task/plan and cannot precede its state revision. Success requires complete known effect records and result references. Cancellation receipt stays active; effective cancellation requires stop and partial-result references. Unknown effects cannot be promoted to successful, failed or cancelled known outcomes; terminal uncertainty requires OUTCOME_UNKNOWN. References are declarations, not verified evidence. The later executor resolves observations, authorizes effects and controls transitions; these constructors do not execute, cancel or undo anything. Events require execution identity when applicable, bounded distinct evidence references and explicit revisions/timestamps; global ordering and delivery deduplication are later runtime responsibilities.

Reproduce on the accepted native ARM64 environment with Rust 1.99.0:

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
cargo test --locked --offline --package eylema-contracts
cargo test --locked --offline --package eylema --test contracts
```

The existing gate runs native formatting, warnings-denied clippy, tests and build, then compiles only the selected contracts library for wasm32-unknown-unknown. Wasm compilation is separate from browser/Wasm execution. Current evidence and exact revisions belong in the delivery PR. G09 adds bounded wire coding/canonical hashes/platform identity; G10 supplies admission/execution; G11 supplies actual oversight. Service, browser, journal, GPU, models, training, performance and full-product acceptance remain later scope. All Cargo packages remain first-party and unpublished; no dependency, settings or license choice is introduced.
