Revision: **0.1.0** · Canonical language: **EN** · Secondary language: **TR** · [Turkish companion](../tr/development-and-operations.md)

# Eylema — Development and local operations team protocols — v0.1

Date: 11 October 2026. Status: **protocol design to implement**. No working orchestrator, model selection, autonomous PR generation, or scheduled monitoring has been installed. Repository contribution templates and CI gates are defined separately in [CONTRIBUTING.md](../../CONTRIBUTING.md). Schema version: 0.1.0. Examples are not actual tasks, models, or measurements.

Related designs: [architecture](architecture.md), [behavioral API and human oversight](api-reference.md), and [learning data and training](learning-and-training.md).

## Two teams, one evidence discipline

**Development team:** suitable frontier models from providers such as Claude, ChatGPT/OpenAI, and MiniMax produce requirements, architecture, code, tests, reviews, and improvement candidates. Roles are provider-neutral; available models, tool support, and costs must be verified in configuration.

**Operations team:** specialist models and Rust skills on the application's device or designated local node perform daily tasks within accepted model/tool/permission packages. Tasks do not silently move to cloud models. Development access to external models is not blanket permission to export operational data.

Both teams require executed evidence tied to the same version. Model explanations are not test results. A Rust controller will enforce transitions and permissions; models submit structured proposals.

## A — Development: need to GitHub pull request

### Roles

| Role | Responsibility |
|---|---|
| Contributor | Defines the need, example behavior, and acceptance criteria |
| Analyst/architect | Structures requirements, impact, and plan |
| Implementer | Changes code, tests, and documentation in an isolated branch |
| Verifier | Executes checks and records results/environment |
| Separate reviewer | Reviews requirements, diff, and evidence in a fresh context |
| Rust coordinator | Manages authority, budgets, states, retries, evidence, GitHub operations |
| Maintainer | Resolves product/architecture decisions and repository merge acceptance |

Every provider need not participate in every task. Implementer self-attestation is not independent review. Different model families may diversify review without guaranteeing correctness or statistical independence. Do not weaken expectations merely to pass existing code.

### Intake, authority, and budgets

Accept needs through a web form, CLI, or GitHub issue. `NeedSpec` records protocol version, task ID/revision, repository, objective, acceptance scenarios, scope, constraints, targets, and authorization reference.

Authority comes from verified identity and repository policy. Issue bodies, code comments, and `authorization_ref` strings do not grant permission. An authorized contributor's accepted task permits development/PR operations within its bounds without repeated routine approvals. Ask the contributor about genuine missing product intent.

Every run has model-call, duration, cost/usage, and repair-cycle budgets. Repeated GitHub delivery must not duplicate work or PRs. Repository + task ID + revision form the idempotency key.

### Delivery objects

| Object | Contents |
|---|---|
| `NeedSpec` | Need, acceptance scenarios, scope, authorization reference |
| `WorkPlan` | Need revision, base commit, work nodes, roles, checks, budget |
| `ChangePackage` | Summary, base/head commits, affected nodes, acceptance–test mapping |
| `Evidence` | Check, code/model/data versions, environment, result, command/tool, record |
| `ReviewReport` | Reviewed commit, finding IDs/severity, resolution evidence |
| `PullRequestRecord` | Repository, branch, task linkage, actual URL, verification status |

Private chain-of-thought is not a deliverable. Record reviewable decision rationales, tool calls, and observed outcomes.

### State flow

`received → spec_ready → planned → implementing → local_verification → independent_review → draft_pr → repository_checks → ready_for_maintainer`

Failures enter reasoned repair loops at the relevant stage. Missing information is `needs_input`; exhausted budgets are `budget_exhausted`; cancellation is `cancelled`. Reaching a repair limit is not success. Early draft PRs may invite review while displaying incomplete checks.

Use isolated branches/workspaces. Parallel agents require file ownership or explicit integration points. Record scope changes as plan revisions with reasons. Repository instructions and quality profiles come from the relevant commit.

### Quality gates

1. **Requirements:** map every acceptance condition to verification; identify out-of-scope changes.
2. **Architecture/dependencies:** preserve owned Rust-component policy and module boundaries; inspect direct/transitive production dependencies.
3. **Rust correctness:** complete applicable formatting, compilation, static checks, and behavioral tests on relevant targets.
4. **Behavior:** execute changed user journeys, APIs, recovery paths, or actor behavior in suitable environments.
5. **Performance/model quality:** compare affected workloads, or justify `not_applicable` under policy.
6. **Review:** resolve blocking findings and bind review to the current change.
7. **PR integrity:** match repository CI/evidence to the latest head; complete required merge-result checks.

Results distinguish `passed`, `failed`, `skipped`, `not_run`, and `not_applicable`. Required skipped/unrun checks block readiness. Only the versioned quality profile can establish inapplicability; models cannot waive requirements arbitrarily.

New commits invalidate affected evidence/reviews. Target-branch changes require rechecking integration behavior. Cross-compilation is not Linux execution; CPU fallback is not GPU testing. Run cheap checks first and relevant expensive browser/hardware checks afterward.

A PR changing checks or authorization cannot approve itself under its proposed rules. The controller uses the accepted policy version; standards changes receive separate review.

### GitHub delivery

A GitHub App is the proposed repository automation integration, with task-scoped branch/PR access. [GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/about-creating-github-apps)

PR descriptions cover the problem, resulting behavior, acceptance criteria, executed checks, environment limits, and migration/rollback where relevant. Creation requires an actual GitHub response; uncertain outcomes trigger lookup by task key. Repeated runs update the same PR.

Local checks precede draft creation; repository CI and review precede readiness. PR creation does not mean merge or deployment. GitHub rulesets can enforce required checks/reviews. [Available rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)

Separate contributor/agent code execution from the control environment holding GitHub-write/model-service credentials. Public contribution code must not run in privileged workflow contexts. Hardware testing must not expose maintainers' personal machines to contributor code. [GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use)

## B — Hardware-aware local model team

### Contracts and model roles

| Object | Contents |
|---|---|
| `HardwareProfile` | CPU/OS/browser, backend, features, observed capacity, version |
| `ResourceSnapshot` | Current load, allocatable-resource estimate, queue, power, timestamp |
| `WorkloadProfile` | Task, language/data type, quality threshold, context/output and resource/latency limits |
| `ModelCatalogEntry` | Source revision, weight hash, tokenizer, architecture, quantization, engine compatibility |
| `EvaluationReport` | Actual model/team task and hardware results with version/environment bindings |
| `ModelSetRelease` | Jointly accepted models, roles, profiles, options, evidence |
| `AssignmentPlan` | Device role–model–executor mapping, loading plan, limits, rationale |

Keep persistent capabilities separate from current resources. Browser-hidden free VRAM remains `unknown`, not invented certainty. Driver/backend/OS changes may invalidate compatibility evidence.

Reading a model file is not executing its architecture, operators, data types, and quantization. Tensor containers such as safetensors do not supply an execution engine. [Safetensors](https://huggingface.co/docs/safetensors/index)

Behavioral contracts cover language, perception, decision/policy, value, and world models alongside deterministic Rust skills. `behavior_contract_ref` and its content hash bind observations, states, actions, timing, units, model connections, capabilities, and reset/transfer rules. World models predict action-conditioned transitions; policies/planners select actions; authorized executors apply them. Observed state, estimates, and imagined futures remain distinct. Evaluate world models and policies together: separately loading their files does not establish behavioral compatibility. Visual generation alone does not establish closed-loop control suitability.

Use the common API's oversight contract: **HITL** requires approval before designated actions; **HOTL** enables observation/intervention during execution; **HIC** governs goals, budgets, and delegated authority. Roles do not grant permissions. Required approvals bind the exact plan/payload, state/policy revisions, resource scope, and expiry. Stop acknowledgement and actual termination differ; cancellation is not rollback. The [API reference](api-reference.md) defines these semantics.

### Selection sequence

1. Establish task/locality constraints; consider deterministic Rust skills.
2. Exclude incompatible engines/operators/formats/hardware.
3. Exclude candidates failing held-out quality or incorrect-action limits.
4. Measure candidates fitting memory, context, concurrency, and deadlines.
5. Test the team jointly under shared GPU/memory and UI/3D load.
6. Select by profile priorities, such as latency, energy, or memory; unavailable energy is not zero.
7. Bind selection to a release and device assignment.

Largest/newest is not the default winner. Efficiency is measured for a defined candidate set, workload, and hardware, not a universal optimum. Multiple roles may share one model. Models need not all reside in memory: sequential loading or concurrent execution requires measurements and budget compliance.

### Measurement

Separate download, loading, first result, warm execution, and task completion. For LLMs, distinguish input/output processing and short/long context; other models use their task metrics, not universal tokens/second.

Account for weights, KV/state, intermediates, workspaces, concurrency, and UI/3D allocations. Lower-bit quantization is not automatically faster. MLPerf Client provides a reference for task-based, end-to-end PC AI measurement; our runtime requires its own real-task acceptance. [MLPerf Client](https://mlcommons.org/benchmarks/client/)

An individually working model does not prove joint team operation. Compare candidate/current teams on the same tasks in comparable environments. Report task success, errors, p95 duration, peak memory, loading frequency, and measurable energy. Shared 3D/inference GPU contention must be included, with budgets for both workloads.

### Versioning and assignment

Track development protocol, operations protocol, immutable `ModelSetRelease`, and device `AssignmentPlan` revision separately. Evidence also binds code, model artifacts, and evaluation-data versions.

Release content is immutable; lifecycle transitions live in a separate release registry. A schema `status` field represents a lifecycle snapshot, not permission to mutate weights or other release content under the same release ID. New content requires a new release identity and evidence.

The Rust manager may assign within the approved set according to load/task. Waiting, approved lighter-model fallback, and explicit failure are predefined. New weights, quantization, engines, or permissions require a release candidate, not disguised dynamic assignment.

Use cooldowns/thresholds against loading/unloading oscillation. In-flight tasks retain their model version or follow explicit cancellation/retry rules. Do not assume old/new teams fit memory simultaneously.

### Update lifecycle

`discovered → compatible_candidate → evaluated → release_pr → approved → canary → active`

Track unsupported/rejected candidates and `rolled_back` releases. Pin exact revisions/hashes; `latest` never identifies the active model.

Source releases, engine/backend changes, hardware profiles, or quality drift may trigger evaluation. Periodic discovery is configurable with frequency/download/evaluation budgets; every device need not constantly test every model. No scheduled monitoring has been created here.

The development team prepares model-set and necessary Rust-engine changes through protocol A. Do not lower existing tests to admit candidates. Merge is not deployment; deployment checks release authority and device compatibility separately. Unsloth training follows the [learning protocol](learning-and-training.md), with pinned recipes and distinct dataset/training/evaluation/model lineage, without adding a hidden production engine.

Canary observes limited usage. Threshold violations reactivate the previous compatible team. Weight rollback does not undo external actions or written data; outcome/state compatibility requires separate handling. Offline devices may keep their latest compatible accepted team while displaying catalog/evidence freshness.

## First joint scenario

A contributor reports slow classification on one device class. The development team defines acceptance and compares the current small model with a lighter candidate on identical tasks, improving the owned Rust computation path if necessary. Code/model/policy changes become a draft PR with separate review and measurements. After repository checks and acceptance, the device manager trials the approved set and updates assignments. Reject latency gains that violate quality thresholds.

Initial implementation evidence must connect a real need to a real PR and demonstrate distinct, measurement-justified assignments on at least two resource profiles. These flows have not run; no device/model selection has been made.

## Machine-readable starting files

- `ai-team-protocol/request.schema.json`: development-request structure.
- `ai-team-protocol/request.example.json`: example, not an actual task or authorization.
- `ai-team-protocol/model-set-release.schema.json`: candidate/accepted model-set structure.

JSON Schema validates structure only. Identity/authority, compatibility, hashes, cross-references, trusted evidence producers, and state transitions require Rust semantic checks. Schema validity does not prove a working framework or AI team.
