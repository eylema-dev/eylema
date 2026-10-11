# Eylema Architecture v0.3 — English canonical

**Architecture revision:** 0.3.0 · **Date:** 11 October 2026 · **Language:** English (canonical)

**Product:** Eylema. **Descriptor:** A Simple, High-Performance Agentic AI Web Platform for Continuous Development and Operations.

**Status:** Research-informed design, not an implemented framework, verified model support, or measured performance.

**Turkish companion:** [Full Turkish architecture](framework-mimari-taslagi-v0.3.md). **Translation revision:** 0.3.0, translated from the Turkish v0.3 source. This English edition establishes the canonical language policy; repository tooling now checks translation byte fingerprints and API identifier parity; semantic equivalence still requires review.

## Language policy

English is the source language for code, API names, schemas, protocols, and documentation. Turkish is the second language and must remain a complete, synchronized companion. Machine identifiers, enum values, schema keys, and protocol versions remain identical across languages. Translations record their source revision; repository fingerprint checks flag unrecorded translation edits. A language label does not establish semantic equivalence without review.

## Central idea

An application declares its state, goals, actions, permissions, resource budget, and acceptance conditions. Large models develop it from these definitions. Local agents operate an accepted release. Observed outcomes and human corrections provide evidence for subsequent development and training.

Simplicity belongs in the application contract and setup; renderer, tensor, and data-engine complexity still exists internally. Ordinary web applications load only necessary modules. 3D, local LLMs, and world models are optional capabilities.

The goal of AI-led development and operations covers coding, testing, review, execution, and improvement by authorized AI teams. Humans define goals, authority, and acceptance policies. Deterministic Rust remains a primary execution path; every frame or request does not require an LLM call.

## Topology

```mermaid
flowchart TB
    H[Human: goals, authority, oversight, corrections] --> C[Shared definitions and API contracts]
    C --> D[Large-model development team]
    D --> P[Code and model PR: test, review, acceptance]
    P --> R[Versioned application and model team]
    R --> B[Browser: Rust/Wasm and WebGPU]
    R --> N[Local Linux: Rust executor]
    B --> O[Operations: state, decision, action, outcome]
    N --> O
    O --> E[Permitted experience and evaluation records]
    H --> E
    E --> T[Data curation and Unsloth training jobs]
    T --> V[Candidate model: task and hardware evaluation]
    V --> P
    E --> D
```

Browser and native nodes execute different work through shared contracts. Task identity, ownership, and redelivery rules govern deduplication; external effects do not receive a universal exactly-once guarantee. Their connection has explicit identity, permission, and data-transfer boundaries. A shared protocol provides neither shared memory nor unrestricted device access.

## Small core, optional capabilities

| Area | Responsibility |
|---|---|
| Definitions and tools | Typed application graph; common source for CLI, API, AI tools, and documentation |
| Web and data | HTML/DOM, events, server operations, identity/authorization, persistence, query contracts |
| Agent execution | State, tasks, queues, deadlines, authority, actions, outcomes |
| Compute and 3D | CPU references, GPU jobs, tensors, scenes, resources |
| Model operations | Language, perception, decision, world models; hardware-based assignment of accepted teams |
| Development and learning | Large-model team, PR workflow, curation, training, release evaluation |

These are logical boundaries, not mandatory services. UI, server, data, and compute engines are planned as our own Rust implementation. Leptos, SQLx, wgpu, and Burn are not default production-engine dependencies.

The initial production core targets zero third-party Cargo packages. The Rust toolchain, browser, OS, GPU driver, and model weights remain explicit platform dependencies. **Unsloth is the selected external dependency of the training/packaging environment**, whose Python and accelerator libraries require a separate inventory and pinned versions. The complete development/training chain is therefore not Rust-only.

## Model team

| Capability | Operational role | Output |
|---|---|---|
| Language model | Interpret requests, explain, analyze extended context | Structured task or proposal |
| Perception model | Interpret images, audio, other observations | Observation and estimated state |
| World model | Predict possible consequences of actions | Future state/representation, cost, termination, calibrated uncertainty where available |
| Decision model | Select or score actions against goals/constraints | Policy output, value, candidate ranking |
| Planner | Search or optimize action sequences | Bounded-horizon plan |
| Rust executor | Execute authorized actions; record observations | Actual execution result |

One model may serve multiple roles. Decisions may combine learned policies, value networks, deterministic rules, and search. World models are enabled where their benefit can be measured, rather than required for every decision.

```text
Observation → State estimate → Candidate futures in world model
                   ↓                         ↓
Goals/constraints → Decision model and planner → Action proposal
                                                    ↓
                                     Authority and state validation
                                                    ↓
                                        Actual action → Observation
```

Keep **observed/authoritative application state**, **estimated state**, and **imagined futures** distinct. Predictions do not automatically become persistent facts. Revalidate actions based on an outdated state revision.

Model contracts specify observation/action schemas, normalization, time steps, coordinates/units, validity periods, recurrent-state ownership, reset rules, and prediction horizons. Reevaluate policies when their world model changes; old latent state is not presumed compatible.

[DreamerV3](https://github.com/danijar/dreamerv3) illustrates policy learning through imagined trajectories; [MuZero](https://deepmind.google/blog/muzero-mastering-go-chess-shogi-and-atari-without-rules/) combines learned dynamics with reward, value, and policy predictions for planning. [NVIDIA Cosmos](https://github.com/NVIDIA/cosmos/blob/main/README.md) includes world reasoning, generation, and action-conditioned model/policy examples. These establish neither support in our engine nor general physical correctness. Visually convincing predictions still require our action contracts and closed-loop task evaluation.

## 3D and WebGPU

The initial 3D scope includes interactive scenes, cameras, object selection/transforms, materials, lighting, asset loading, instancing, and culling. Add level of detail and streaming as workloads require. Precision CAD, constraint solvers, full physics, and video-generating world models remain separate capabilities.

A **shared GPU resource manager** budgets rendering, LLM inference, decision models, and world-model rollouts together. Account for weights, KV/recurrent state, intermediate tensors, textures/meshes, and transfer buffers. Compatible work may share resources within one device context under valid usage/layout rules; universal zero-copy is not promised.

Rendering, fast decisions, language generation, and future prediction run at different frequencies. Split divisible computation into short jobs and bound queues/concurrency. Scheduling pending jobs does not imply instant preemption of arbitrary submitted GPU work. If frame budgets are exceeded, adjust inference frequency, rollout horizon, or scene quality only within the accepted profile.

A 60 FPS target allows approximately 16.7 ms per frame; this is arithmetic, not a measured guarantee. Acceptance measures the combined scene and model team: frame-time p95/p99, task quality, decision latency, memory, and loading cost.

[WebGPU](https://www.w3.org/TR/webgpu/) and [WGSL](https://www.w3.org/TR/WGSL/) define browser graphics/compute. Rust/Wasm is not itself a GPU shader. The intended path generates WGSL from a narrow supported compute definition, with low-level shader access where needed; compiling arbitrary Rust to GPU code is outside the initial scope.

[WebLLM](https://webllm.mlc.ai/docs/) demonstrates browser-local inference and worker separation. It does not remove the architecture, tokenizer, attention/KV, quantization, or kernel work required by our engine. Workers do not add GPU capacity. Browser and native backends require separate support profiles; [CubeCL's platform comparison](https://github.com/tracel-ai/cubecl#supported-platforms) illustrates their differing capabilities. Feature/limit queries and actual measurements govern selection.

## Hardware-based teams and releases

Selection follows: workload → supported model/operators/format → quality threshold → resource budget → combined-scene measurement → accepted team.

A running device assigns only options within an accepted `ModelSetRelease`. Model, quantization, engine, or behavioral-contract changes create release candidates. Discovery triggers evaluation, not automatic activation. PRs, evaluations, controlled deployment, and rollback have independent records.

Each model progresses through four support states: specified; validated against reference outputs; executed on a named local runtime; accepted for a particular scene/team/hardware combination. Training support is recorded separately. LLM support does not establish convolution, recurrent-state, or diffusion support. **No model has yet been classified as running in our engine.**

Version the operations protocol, model team, device assignment, behavior contract, training dataset, recipe, and evaluation set separately; events link their revisions. Reassess relevant compatibility evidence after environment changes.

## Continuous learning and ecosystem

Permitted experience records link context, observation, proposed/executed action, actual outcome, cost, and human correction. Raw records undergo cleaning, provenance/permission checks, deduplication, and quality review. Include representative successes alongside failures and corrections. A model's answer is not automatically a correct label.

Language examples, decision trajectories, world-model transitions, and hardware measurements use distinct data contracts. Split training/test data by related sessions, time, and tasks. Collection permission is separate from execution authority; using the platform does not authorize community training on its data.

Pinned Unsloth recipes select supported architectures/methods and use separate training budgets. Exported candidates must pass numerical compatibility and task evaluations in our Rust CPU/GPU paths. Unsloth fine-tuning/RL/export capabilities do not establish compatibility with every custom world model or our engine. [Unsloth](https://github.com/unslothai/unsloth), [export troubleshooting](https://docs.unsloth.ai/basics/saving-and-using-models/troubleshooting)

Community contributions may include permitted datasets, labeling rules, task/evaluation packages, training recipes, model packages, hardware profiles, and Rust kernels. Each carries provenance, usage terms, revision, and reproducible evidence. Private raw data and hidden evaluation sets do not automatically become public packages.

## Shared API for humans and AI

The API is a core deliverable. Shared typed contracts underpin Rust SDK, HTTP, event streams, CLI, and AI tools, with human guidance and machine-readable capability schemas derived from them. Formats such as OpenAPI/JSON Schema do not mandate an external production engine.

| Human relationship | Contract requirement |
|---|---|
| Human-in-the-loop | Policy-designated actions await a decision bound to specific plan/input/state revisions |
| Human-on-the-loop | Humans observe execution and reasoned outcomes, request intervention/stopping, and verify the resulting state |
| Human-in-command | Humans define goals, authority, budgets, delegation, and revocation policy |

These modes may coexist per action. Low-impact steps do not universally require repeated approval. AI operates within valid authority; actions outside it stop explicitly. Cancellation does not automatically reverse external effects already produced.

Each operation documents purpose, inputs/outputs, preconditions, effects, authority, timeout, retries, idempotency, oversight, and errors. Once implemented, examples must compile/run in CI and be tied to releases. Current examples remain proposed contracts.

## Development sequence and first product evidence

| Slice | Deliverable | Acceptance evidence |
|---|---|---|
| 0 | Narrow goal; API/state/action contracts; support/dependency matrix | Consistent interpretation by human and AI tools |
| 1 | Small web UI, local Rust server/persistence, WebGPU 3D scene | Task, object movement, save, and reopen flow |
| 2 | Deterministic decision baseline, own tensor operations, small learned policy/dynamics model | Prediction/observation error, closed-loop success, CPU/GPU parity |
| 3 | One supported small local language model and shared GPU management | LLM, decisions, and scene meet quality/resource limits together |
| 4 | Experience records, human corrections, first Unsloth recipe/model acceptance | Traceable data-to-model improvement on separated evaluation tasks |
| 5 | Large-AI team orchestration, GitHub PR workflow, hardware-based team release | Real requirement-to-PR flow; justified assignment on two resource profiles |
| 6 | Production web features and contribution ecosystem | Selected production protocols and independent contribution compatibility |

API and evaluation definitions belong in every slice. A small 3D warehouse/placement environment is a proposed starting point: move objects, predict outcomes, decide, observe reality, and record corrections. A later LLM connects natural-language goals to those actions. This scenario has not been implemented.

Building our own web engine, renderer, model runtime, and training ecosystem is substantial work. AI teams may accelerate development; hardware validation and maturation remain necessary. Scope expands through executed evidence, without universal model/device compatibility promises.

## Detailed contracts

- [Development and local operations team protocols](ai-team-protocols-v0.1.en.md)
- [Learning data and training protocol](learning-data-training-protocol-v0.1.en.md)
- [English API reference](api/en/reference-v0.1.md)
- [Development request JSON Schema](ai-team-protocol/request.schema.json)
- [Local model team JSON Schema](ai-team-protocol/model-set-release.schema.json)

These are architecture and structural-contract deliverables. This design exercise has not performed training, model/device benchmarks, framework execution, GitHub PR creation, or scheduled monitoring.
