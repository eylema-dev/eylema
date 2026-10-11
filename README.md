<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/eylema-logo/logo-dark.png">
    <img src="assets/eylema-logo/logo.png" alt="Eylema" width="520">
  </picture>
</p>

# Eylema

**A Simple, High-Performance Agentic AI Web Platform for Continuous Development and Operations.**

Code, identifiers, file names and contribution templates use English. Documentation is available in English and [Turkish](README.tr.md), with English as the canonical source. The product name **Eylema** was accepted on **11 October 2026**; the description above states its intended purpose.

Public documentation repository: [eylema-dev/eylema](https://github.com/eylema-dev/eylema), under the [eylema-dev organization](https://github.com/eylema-dev). The proposed CLI name and package prefix are `eylema`; they are not implemented or published. The project owner reported registering **`eylema.dev` through GoDaddy** on 11 October 2026. License selection remains pending.

The [name and availability record](docs/en/name-availability.md) preserves the checks made before organization creation and the subsequent registration update. `eylema-dev` and this public repository were established on 11 October 2026. The primary domain is `eylema.dev`; domain registration does not imply an accepted platform deployment.

Brand assets: [SVG / PNG logos and usage guide](assets/eylema-logo/README.md), [preview](assets/eylema-logo/brand-preview.png), [download the logo pack](assets/eylema-logo.zip).

**Status: architecture/API drafts and contribution tooling.** Repository CI validates documents, schemas and PR structure; it does not implement the Rust runtime, model execution, autonomous AI development team or performance benchmarks.

## Contribute

Start with [contribution rules](CONTRIBUTING.md) and the shared [AI agent contract](AGENTS.md). Follow the [first-release roadmap](ROADMAP.md), [quality gates](docs/en/quality-gates.md) and [CI tooling inventory](docs/en/tooling.md). Issue and PR templates collect scope, acceptance and evidence. Maintainer review remains separate from CI; required-check branch rules are not enabled by these files.

## Product direction

Build an end-to-end web platform that AI teams can develop and operate through explicit, inspectable contracts. Applications declare state, goals, actions, authority, budgets and acceptance conditions. Human participation, oversight and command remain practical capabilities of the same system.

- Own Rust production components for web, data, agents, compute and tooling; optional 3D, local language, decision and world-model capabilities.
- Browser Rust/Wasm/WebGPU and native Linux execution, with separately verified capabilities.
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
| Project structure 0.1.0 | [First application and component proposal](docs/en/project-structure.md) | [İlk uygulama ve bileşen önerisi](docs/tr/project-structure.md) |
| Architecture 0.3.1 | [Architecture](docs/en/architecture.md) | [Mimari](docs/tr/architecture.md) |
| API and oversight 0.1.0 | [API reference](docs/en/api-reference.md) | [API referansı](docs/tr/api-reference.md) |
| Development and operations 0.1.0 | [Team protocols](docs/en/development-and-operations.md) | [Takım protokolleri](docs/tr/development-and-operations.md) |
| Learning and training 0.1.0 | [Learning protocol](docs/en/learning-and-training.md) | [Öğrenme protokolü](docs/tr/learning-and-training.md) |

Machine-readable drafts: [development request schema](ai-team-protocol/request.schema.json), [illustrative request](ai-team-protocol/request.example.json), [model team release schema](ai-team-protocol/model-set-release.schema.json). Schema validity is not authorization or evidence of successful execution.

English identifiers, paths, fields, enums and error codes are shared across languages. Current documents have matching English file names under `docs/en/` and `docs/tr/`; translations track source revisions. Earlier architecture versions and the original API outline are retained in `docs/archive/` as historical records. See [project structure](docs/en/project-structure.md) for the layout and migration notes.

## First proof

The selected first working application is a task console: create a task, run deterministic steps, inspect progress, stop remaining work, save state and reopen it after a service restart. Build the Rust contracts, executor and journal, then demonstrate the minimal web interface. [Acceptance scenarios and proposed components](docs/en/project-structure.md) define what that demonstration must prove.

The later 3D environment, learned policy/dynamics model, local language model, permitted training loop and development-team PR flow remain separate roadmap milestones. The first browser demonstration requires neither model access nor a GPU. No working application is delivered by this documentation revision.
