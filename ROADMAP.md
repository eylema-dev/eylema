# Eylema first-release roadmap

Revision **0.1.0**, 11 October 2026. English is canonical; [Türkçe](ROADMAP.tr.md). Target: **v0.1.0 experimental**, not a production-ready universal framework. Milestones are acceptance gates, not delivery-date promises. Only repository documents exist today; M0 is proposed by the contribution-foundation PR until merged and checked on `main`.

## The first complete application

A small placement environment lets a user create a task, move a 3D object, save and reopen state, observe a deterministic agent, compare a learned policy/world-dynamics candidate, and issue a natural-language goal through one explicitly supported local language model. Human approval, observation, stop and authority changes are visible. A permitted episode can become a curated training sample; an evaluated model update has a reversible assignment path. A frontier-model development task ends in a real PR with checkable evidence.

The first release supports a documented Linux device and a documented browser/device combination, not all GPUs or models. A second measured hardware profile is needed for the assignment milestone. Record actual memory, latency/frame-time and quality baselines before approving numerical limits; do not invent them before hardware and workload selection.

| ID | Release gate | Scope | Acceptance evidence | Depends on |
|---|---|---|---|---|
| M0 | Contribution foundation | Agent contract, bilingual docs, issue/PR templates, read-only CI | Validators reject negative fixtures; PR and main CI are observed; maintainer review/ruleset status is explicit | None |
| M1 | `v0.1.0-alpha.1` | First-party Rust workspace, typed task/state/action contracts, deterministic executor | Pinned toolchain; zero external production Cargo dependencies; Linux and Wasm compilation; Linux unit/recovery tests; bounds and denied-action tests; CI wired before new Rust source is accepted | M0 |
| M2 | `v0.1.0-alpha.2` | Minimal owned web UI, local server and persistent task state | Create, inspect, cancel, save, restart and reopen flow; duplicate-request behavior; permission denial; no false stop acknowledgement; documented network/auth/TLS boundary | M1 |
| M3 | `v0.1.0-alpha.3` | Browser GPU boundary and small 3D scene | Real WebGPU execution on named browser/driver; device loss and unsupported-path errors; bounded memory/job scheduling; frame-time distribution under load; host bindings and WGSL inventory | M2 |
| M4 | `v0.1.0-beta.1` | CPU tensor references, learned decision policy and small world-dynamics model | CPU/GPU parity within declared tolerance; held-out closed-loop success; prediction error; constraints respected; beats or explicitly fails to beat deterministic baseline | M3 |
| M5 | `v0.1.0-beta.2` | One pinned small local language model in our engine | Verified weights/tokenizer/operators/quantization; load/first-result/warm metrics; held-out task quality; shared scene/policy/LLM budgets; fail clearly for unsupported models; no hidden hosted fallback | M4 |
| M6 | `v0.1.0-rc.1` | Versioned model assignment and permitted learning loop | Two measured profiles; accepted team manifest; canary and rollback exercise; data consent/provenance/deduplication/holdout; pinned external Unsloth recipe; candidate revalidated in owned runtime | M5 |
| M7 | `v0.1.0` | Need-to-PR development protocol and experimental release | One real task through intake, plan, implementation, checks and independent review; two configured provider adapters evaluated on the same contract; idempotent PR retry; documented limits, license and reproducible release evidence | M6 |

## First executable backlog

These IDs are planning references, not already-created GitHub issues. Split a row into a scoped issue with acceptance tests before implementation.

| ID | Next work item | Done when |
|---|---|---|
| EYL-001 | Select license and contribution terms | Maintainer records the decision and provenance rules; no implied contributor agreement |
| EYL-002 | Fix first workload and hardware matrix | One Linux profile, one browser profile, the second assignment profile and reproducible scene/task data are documented; unavailable equipment stays `not_run` |
| EYL-003 | Bootstrap owned Rust workspace and CI | Exact Rust release and lockfile; Linux/Wasm build commands and actual Linux tests run; dependency inventory proves no external runtime crates; documentation-stage source guard replaced by real Rust gates |
| EYL-004 | Implement task state machine | Success, failure, denied action, timeout, duplicate delivery and cancellation behavior covered, with no exactly-once claim for external effects |
| EYL-005 | Implement durable local task journal | Truncated record, crash/restart and incompatible-format paths tested; explicit version and recovery behavior |
| EYL-006 | Build first web task screen | Create/inspect/stop/reopen journey is demonstrated in a browser with bounded permissions and visible errors |
| EYL-007 | Prototype WebGPU resource boundary | One compute operation and one 3D scene execute on actual hardware; report CPU reference and unsupported/device-loss behavior |
| EYL-008 | Select first model contracts | Decide candidate policy/dynamics/LLM architectures by operator coverage, redistribution terms and measured resource limits; selection alone is not support |

## v0.1.0 release checklist

- Every milestone's required checks has current evidence on the exact release candidate; skipped required hardware checks block the affected support claim.
- Quality thresholds, p95/p99 latency/frame time, memory budgets and startup behavior are defined and met for the declared profiles. Report failures as well as gains.
- HITL/HOTL/HIC contracts, revocation, intervention and rollback limitations are exercised. A model's output cannot widen authority.
- English and Turkish guides reproduce the first application and report supported/unsupported targets. API/code/schema versions agree.
- Distribution license, model/data rights, toolchain/driver/browser inventory and known limitations are recorded. No kernel rewrite, production crypto implementation or hardware access beyond platform permissions is implied.
- A maintainer accepts the release. CI alone cannot promote models, merge changes or publish a release.

## Deliberately outside the first release

A general CAD kernel, every model architecture, distributed training, a Rust-only Linux kernel, arbitrary GPU driver support, automated procurement, unattended privileged self-modification and complete production web feature parity are later decisions. The roadmap preserves the long-term architecture while proving one bounded vertical slice first.
