<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/eylema-logo/logo-dark.png">
    <img src="assets/eylema-logo/logo.png" alt="Eylema" width="520">
  </picture>
</p>

# Eylema

**A Simple, High-Performance Agentic AI Web Platform for Continuous Development and Operations.**

Code, identifiers, file names and contribution templates use English. Documentation is available in English and [Turkish](README.tr.md), with English as the canonical source. The product name **Eylema** was accepted on **11 October 2026**; the description above states its intended purpose.

Core project repository: [eylema-dev/eylema](https://github.com/eylema-dev/eylema), under the [eylema-dev organization](https://github.com/eylema-dev). The package prefix and native host CLI are `eylema`; local service start, task run/inspect, service stop and doctor commands are implemented. Task cancellation still requires an independently host-verified human session. The project owner reported registering **`eylema.dev` through GoDaddy** on 11 October 2026. First-party source and documentation are licensed under [Apache-2.0](LICENSE). Separately identified third-party material, model weights and datasets retain their own terms.

The [name and availability record](https://github.com/eylema-dev/devguide/blob/main/docs/en/name-availability.md) preserves the checks made before organization creation and the subsequent registration update. `eylema-dev` and this public repository were established on 11 October 2026. The primary domain is `eylema.dev`; domain registration does not imply an accepted platform deployment.

Brand assets: [SVG / PNG logos and usage guide](assets/eylema-logo/README.md), [preview](assets/eylema-logo/brand-preview.png), [download the logo pack](assets/eylema-logo.zip).

**Status: owned Rust task core, durable recovery, scoped native loopback HTTP service, native task CLI and local browser task console.** [G16 browser-console notes](docs/en/local-browser.md) cover the same-origin interface, reload recovery and truthful stop authorization. [G14 local HTTP evidence](docs/en/local-http.md) covers actual socket/file behavior, session/Host/Origin/CSRF checks and bounded input/deadline rejection. [G13 recovery evidence](docs/en/runtime-recovery.md) covers retained identities/results/idempotency and actual child-process interruptions with truthful unknown outcomes and fresh enrollment. [G12 journal evidence](docs/en/local-journal.md) covers canonical storage, explicit failures and actual macOS synchronization/close/reopen; semantic runtime restart recovery is covered by G13. [G11 oversight evidence](docs/en/human-oversight.md) records native behavior and the explicit simulated-human host boundary. [G10 executor evidence](docs/en/deterministic-runtime.md) covers bounded native admission/steps/events and actual owned state; [G09 codec/platform evidence](docs/en/codec-platform.md) covers actual native/Node-hosted Wasm comparison. [G07 workspace evidence](docs/en/rust-workspace.md) covers real native compilation/process tests and selected portable-library Wasm compilation. [G15 local CLI evidence](docs/en/local-cli.md) covers child-process CLI/service identity, error, cancellation authorization, shutdown and journal reopen behavior. Real person authentication/browser oversight, model execution, autonomous development and performance acceptance remain later Goals.

G20 WebGPU scene notes: [local WebGPU view](docs/en/local-webgpu.md) describe the bounded scene, controls, and explicit unsupported/device-loss states. Browser/GPU acceptance is recorded separately from repository checks.

G21 adds bounded, cooperative host-resource admission for declared Metal, view and model work classes: [resource manager](docs/en/resource-manager.md). Its byte limits account for caller-declared transient reservations, not physical GPU memory, model weights or actual browser/model execution.

G22 adds bounded [combined GPU measurement hosts](docs/en/combined-gpu-measurement.md); repeated real hardware acceptance and raw distributions are tracked in eylema-evals.

G39 adds a [controlled native development workspace](docs/en/development-workspace.md): detached accepted-base worktrees, bounded single-process macOS isolation, scoped artifacts and preservation of dirty caller work. Its native proof uses synthetic host authority; provider/compiler/GitHub execution remains separate.

G40 is a [bounded Codex adapter draft](docs/en/codex-adapter.md). Native broker/parser/process tests pass; actual local CLI permission controls pass, but real configured provider calls have not produced an accepted proposal.

## Contribute

Start with [contribution rules](https://github.com/eylema-dev/devguide/blob/main/CONTRIBUTING.md) and the shared [AI agent contract](https://github.com/eylema-dev/devguide/blob/main/AGENTS.md). Follow the [first-release roadmap](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/roadmap.md), [quality gates](https://github.com/eylema-dev/devguide/blob/main/docs/en/quality-gates.md) and [CI tooling inventory](https://github.com/eylema-dev/devguide/blob/main/docs/en/tooling.md). Issue and PR templates collect scope, acceptance and evidence. Maintainer review remains separate from CI; required-check branch rules are not enabled by these files.

## Product direction

Build an end-to-end web platform that AI teams can develop and operate through explicit, inspectable contracts. Applications declare state, goals, actions, authority, budgets and acceptance conditions. Human participation, oversight and command remain practical capabilities of the same system.

- Own Rust production components for web, data, agents, compute and tooling; optional 3D, local language, decision and world-model capabilities.
- Browser Rust/Wasm/WebGPU and native macOS CPU/Metal on one M4 Pro/24 GB device for the first experimental release; Linux and second physical-device acceptance are future scope.
- Frontier-model development teams using a standard requirement-to-PR protocol.
- Hardware-selected local model teams, versioned and evaluated under combined application load.
- Permitted experience, human corrections and verified outcomes feeding curated training data.
- Unsloth as an explicit external training/packaging tool; exported candidates verified in our own runtime before acceptance.
- Public contribution contracts for code, data, evaluation, training recipes, models and hardware profiles.

## What “world-leading” must mean

The ambition is to become a leading platform for AI-led web development and operations. It is an objective to demonstrate, not a current ranking or a promise of universal superiority. Public comparisons must publish hardware, workload, quality threshold, versions, configuration, limitations and reproducible results.

| Dimension | Evidence to require |
|---|---|
| AI development | Correct completion of held-out change requests; repair count, elapsed time, model cost and human intervention |
| Application reliability | End-to-end behavior, persistence/recovery, authorization, upgrades and explicit failure outcomes |
| Combined performance | Scene frame-time p95/p99, action/task latency, quality, peak memory, startup and measurable energy together |
| Decision and world models | Closed-loop task success, constraint violations, multistep prediction error and comparison against a deterministic baseline |
| Continuous learning | Improvement on independent tasks within comparable resource budgets, including regressions and unsuccessful candidates |
| Human agency | Effective approval, observation, intervention and revocation; measured intervention limits and understandable evidence |
| Simplicity and extensibility | First working application, conceptual/API surface, independent contribution success, supported targets and dependency inventory |

Quality floors are mandatory before optimizing speed or cost. Compare complete application outcomes as well as kernels; report cold and warm operation separately. Avoid one synthetic score hiding tradeoffs. Benchmark-only tools or reference implementations remain outside the production dependency boundary. Exact numerical targets follow the first workload and hardware selection.

## Current documentation

| Document | English source | Turkish companion |
|---|---|---|
| Project structure 0.3.0 | [First application and component proposal](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/project-structure.md) | [İlk uygulama ve bileşen önerisi](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/project-structure.md) |
| Architecture 0.4.1 | [Architecture](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/architecture.md) | [Mimari](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/architecture.md) |
| API and oversight 0.3.0 | [API reference](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/api-reference.md) | [API referansı](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/api-reference.md) |
| Development and operations 0.2.0 | [Team protocols](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/development-and-operations.md) | [Takım protokolleri](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/development-and-operations.md) |
| Learning and training 0.2.0 | [Learning protocol](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/learning-and-training.md) | [Öğrenme protokolü](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/learning-and-training.md) |

Machine-readable drafts: [development request schema](ai-team-protocol/request.schema.json), [illustrative request](ai-team-protocol/request.example.json), [macOS model-set 0.2.0 schema](ai-team-protocol/v0.2/model-set-release.schema.json), [synthetic candidate](ai-team-protocol/v0.2/model-set-release.example.json), [preserved historical 0.1.0 schema](ai-team-protocol/model-set-release.schema.json). Schema validity is not authorization or evidence of successful execution.

English identifiers, paths, fields, enums and error codes are shared across languages. Current documents use matching English file names under `docs/` in `eylema-docs-en` and `eylema-docs-tr`; translations track source revisions. Earlier Turkish architecture versions and the original API outline are retained in `eylema-docs-tr/archive/` as historical records. See [project structure](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/project-structure.md) for the layout and migration notes.

## First proof

The first user is a developer building a single-user local agent application. The selected first working application is a task console: create a task, run deterministic steps, inspect progress, stop remaining work, save state and reopen it after a service restart. Build the Rust contracts, executor and journal, then demonstrate the minimal web interface. [Acceptance scenarios and proposed components](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/project-structure.md) define what that demonstration must prove.

The later 3D environment, learned policy/dynamics model, local language model, permitted training loop and development-team PR flow remain separate roadmap milestones. The first browser demonstration requires neither model access nor a GPU. The G15 CLI and G16 browser console now run and inspect the local task service; the non-human CLI session receives a truthful `FORBIDDEN` response to a stop request.

## Organization repositories

[English product docs](https://github.com/eylema-dev/eylema-docs-en) · [Turkish product docs](https://github.com/eylema-dev/eylema-docs-tr) · [Developer guide](https://github.com/eylema-dev/devguide) · [Repository map](https://github.com/eylema-dev/devguide/blob/main/docs/en/repository-map.md)

`eylema` owns the initial Rust workspace/bootstrap host, future task CLI/components, machine-readable schemas and existing brand assets. Product prose moves to the language repositories; shared contribution policy moves to devguide. Examples, evaluations, model metadata and training recipes have separate repositories with explicit scaffold-only status. The website is managed separately in [eylemadotdev](https://github.com/eylema-dev/eylemadotdev). No runtime has been implemented by this split.

Read [the migration record](https://github.com/eylema-dev/devguide/blob/main/docs/en/repository-migration.md) for exact sources, destinations, linked PRs, integration checks and rollback. New document links target the coordinated post-merge layout; the PRs and pinned repository set provide review copies before then.

The maintainer accepted the [first-product decisions and test inventory](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/project-structure.md) on 12 October 2026: deliver the usable task-console preview and an early need-to-PR proof before optional model/training work. MacBook Pro M4 Pro/24 GB remains primary; Mac mini M4/24 GB and Linux RTX 4070 Ti Super/16 GB VRAM + 32 GB RAM are additional test equipment awaiting their own execution evidence. Existing Rust types, EYL/EYLJ formats and scoped platform bindings remain the implementation baseline.

- [G38 development coordinator](docs/en/development-coordinator.md): Owned Rust need-to-review state, host-resolved authority, finite budgets/cancellation and stale-evidence rejection; process/provider/GitHub isolation and real delivery remain G39–G43.
