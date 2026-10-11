---
docs_language: en
source_revision: 0.1.0
status: design
canonical_language: en
---

# Eylema — Shared API and human oversight — v0.1

English is the canonical source; [Türkçe](../tr/reference-v0.1.md) is its complete translation. Operation identifiers, field names, states and error codes remain English in every language. This document specifies a proposed contract. The named endpoints, SDK, generators and conformance suite are not implemented or demonstrated by this document. No OpenAPI specification is delivered in this revision.

## One contract, several clients

Human interfaces, development agents, local models and deterministic Rust skills use the same operations and authorization rules. Provider identity, model capability and a human name never establish permission. The controller resolves an authenticated principal, resource scope, accepted policy revision and any required approval. A string such as `authorization_ref` locates a record; it is not proof of authority.

Owned, versioned typed definitions will become the canonical machine contract. The proposed build produces OpenAPI, JSON Schema, MCP tool descriptions and a Rust SDK from those definitions. Adapters cannot widen authority, invent defaults or translate failures into success. This generation design does not introduce third-party production dependencies. Existing schemas cover selected artifacts, not this entire API.

Permissions compose per operation: inspecting a model does not permit training it; proposing a release does not permit activating it. A deployment may assign several permissions to one principal while requiring separate approval for selected transitions. Browser clients cannot acquire native filesystem, process or GPU privileges by selecting `native_linux` in a request.

## Oversight mechanisms

The European Commission describes HITL, HOTL and HIC as oversight approaches. They are **composable mechanisms**, not mutually exclusive modes or user roles. HOTL also encompasses oversight during system design; runtime monitoring alone does not exhaust the concept. HIC includes deciding whether and how a system is used. [Commission overview](https://digital-strategy.ec.europa.eu/en/library/ethics-guidelines-trustworthy-ai), [explanations in footnote 13](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:52019DC0168).

Our operational mapping below is a design choice, not a claim of legal compliance:

- **Human-in-the-loop (HITL):** human participation at specified decision points. A policy may require a human decision before a consequential action, dataset admission or release transition. It need not require approval of every routine action.
- **Human-on-the-loop (HOTL):** inspect evidence, review behavior, monitor events and intervene through authorized commands. Design reviews and operational monitoring both contribute. A stop request must report whether interruption actually became effective.
- **Human-in-command (HIC):** establish purposes, budgets, delegation and versioned policies; override permitted decisions, revoke delegation or discontinue use. HIC is broader than approving individual actions.

`oversight_requirements` records independently applicable checkpoints, observation/intervention requirements and governance references. It is not an enum containing only `HITL`, `HOTL` or `HIC`. Models cannot expand their own delegation. Policy changes are judged under the currently accepted policy, not the policy they propose.

## Common operation metadata

Every published operation must provide this template before implementation is accepted:

| Metadata | Required meaning |
|---|---|
| `operation_id`, `operation_version` | Stable identifier and compatible contract version |
| `request_schema_ref`, `response_schema_ref`, `error_schema_ref` | Exact versions, required fields, bounds and rejected examples |
| `side_effect_class`, `resource_scope` | Read, record mutation, compute or external effect; exact authorized resources |
| `execution_context`, `required_capabilities` | Supported `browser` / `native_linux` targets and actual prerequisites |
| `permissions`, `oversight_requirements` | Principal permissions and composed oversight obligations |
| `preconditions`, `expected_revision` | State assumptions and concurrency checks |
| `approval_binding`, `policy_revision` | Applicable decision record and governing policy |
| `deadline`, `budget`, `max_concurrency` | Time, resource and concurrency limits |
| `idempotency`, `retry`, `cancellation`, `compensation` | Replay, recovery, interruption and remedial-action rules |
| `audit_events`, `result_evidence` | Observable transitions and evidence needed for success |

Capabilities describe availability, never permission. An unavailable GPU yields an explicit unsupported result or a previously authorized fallback. Unknown memory capacity remains unknown. Each operation publishes its limits; clients do not infer them from a model description.

## Draft operation catalogue

Paths identify proposed control-service routes, not live URLs. Every row inherits the metadata template. Its context is the control service; `execution_context` selects a supported worker target only where work is dispatched. Read operations do not change domain state; access auditing may still occur. The catalogue is a baseline, not complete CRUD coverage.

| `operation_id` | Method and path | Effect and resource scope |
|---|---|---|
| `inspectRuntime` | `GET /v0.1/runtime` | Read authorized runtime state and observed limits |
| `listCapabilities` | `GET /v0.1/capabilities` | Read operation, target and backend availability |
| `createTask` | `POST /v0.1/tasks` | Record task objective, scope and budget |
| `getTask` | `GET /v0.1/tasks/{task_id}` | Read task snapshot and related records |
| `createActionProposal` | `POST /v0.1/action-proposals` | Record immutable plan revision and action payload |
| `getActionProposal` | `GET /v0.1/action-proposals/{proposal_id}` | Read proposed effects, prerequisites and review evidence |
| `createApproval` | `POST /v0.1/approvals` | Record scoped decision by an authorized approver |
| `revokeApproval` | `POST /v0.1/approvals/{approval_id}/revocations` | Revoke future use; preserve decision history |
| `createExecution` | `POST /v0.1/executions` | Dispatch the exact authorized proposal |
| `getExecution` | `GET /v0.1/executions/{execution_id}` | Read lifecycle, effects and result evidence |
| `requestExecutionCancellation` | `POST /v0.1/executions/{execution_id}/cancellation-requests` | Request interruption of that execution |
| `listEvents` | `GET /v0.1/events` | Read scoped, cursor-based audit events |
| `registerModel` | `POST /v0.1/models` | Record model revision, artifacts and compatibility claims |
| `registerBehaviorContract` | `POST /v0.1/behavior-contracts` | Record world/policy/skill semantics and state rules |
| `createDatasetVersion` | `POST /v0.1/datasets/{dataset_id}/versions` | Record immutable data lineage and admission decision |
| `createTrainingRun` | `POST /v0.1/training-runs` | Schedule bounded training on an authorized data version |
| `createEvaluationRun` | `POST /v0.1/evaluation-runs` | Schedule evaluation against fixed criteria and hardware |
| `createRelease` | `POST /v0.1/releases` | Record an immutable candidate model-set manifest |
| `transitionRelease` | `POST /v0.1/releases/{release_id}/transitions` | Request governed approval, rollout, rollback or retirement |
| `createPolicyRevision` | `POST /v0.1/policies/{policy_id}/revisions` | Request governance, delegation or revocation revision |

Registration does not download, execute or trust arbitrary artifacts. References are resolved only within authorized sources. Resource creation returns its identifier and revision. Asynchronous work returns a task/execution reference and acceptance state; acceptance is not completion. Proposed HTTP semantics use `201` for created records and `202` for queued work, with errors carrying machine-readable reasons.

Training, evaluation, policy activation and release transition reuse the proposal/approval/execution lifecycle whenever their declared effects require it. Creating a policy candidate does not activate it. Plan steps have their own effect scopes; approving a parent objective does not silently authorize new effects. Governance changes cannot bypass the currently required checkpoint.

## Approval and execution

An approval binds `proposal_id`, `plan_revision`, canonical `payload_sha256`, `state_revision`, `policy_revision`, `resource_scope` and `expires_at`. Canonical serialization and hash rules must be fixed before interoperable approval is implemented. The decision record identifies its verified principal; a claimed `human` field is insufficient when verified human approval is required.

At execution, the controller rechecks permission, revocation, expiry, budget, payload and current preconditions. A changed plan or material state requires a new decision when the binding no longer matches. Where supported, resource adapters use conditional writes against the expected revision. An external world without such guarantees must expose the remaining race and observed outcome, not claim atomicity.

Revocation prevents subsequent authorized use once effective; it does not retroactively erase an action already performed. Override commands require their own permission and audit record. An execution records each effect and its confirmation independently.

## Events, timing and failures

Requests carry `request_id`, contract version and correlation context; mutating requests include `expected_revision` when updating an existing resource. Events carry `event_id`, `task_id`, `execution_id` when applicable, state revision, timestamp and evidence references. Trace summaries record reasons, tool inputs and observed results, not hidden model reasoning.

`listEvents` accepts scope filters, bounded `limit` and opaque `cursor`; responses include `next_cursor`. Ordering is guaranteed only within the declared stream, not globally. Delivery can repeat an event; deduplicate by `event_id`. Cursor expiry returns `CURSOR_EXPIRED`; clients retrieve a current task/execution snapshot and resume from a new cursor. Retention and visible redactions are documented per deployment.

Errors include `code`, human-readable `message`, optional `field_path`, `retryable`, `correlation_id` and optional `retry_after_ms`. Stable codes include `INVALID_REQUEST`, `FORBIDDEN`, `PRECONDITION_FAILED`, `APPROVAL_REQUIRED`, `APPROVAL_EXPIRED`, `APPROVAL_MISMATCH`, `REVISION_CONFLICT`, `CAPABILITY_UNAVAILABLE`, `BUDGET_EXCEEDED`, `DEADLINE_EXCEEDED`, `IDEMPOTENCY_CONFLICT`, `CURSOR_EXPIRED` and `OUTCOME_UNKNOWN`. Translations change the message, not the code.

A deadline limits admission and cooperative execution; elapsed time does not prove effects stopped. Queue delay, execution time and effective cancellation time are separately observed. Browser suspension, driver scheduling and external calls prevent blanket hard realtime guarantees.

## Retries, cancellation and recovery

Mutations use an `idempotency_key` scoped to principal, operation and resources, with the canonical payload hash. Within the published retention period, an identical replay returns the existing result/reference. Reusing the key for different content returns `IDEMPOTENCY_CONFLICT`. After uncertain network delivery, query the known task/execution or replay the same key before creating another operation. Clients stop automatic retries when the retention boundary is uncertain.

Retry only errors explicitly marked retryable, respecting delay and budget. A timeout can leave an effect completed but unacknowledged; record `OUTCOME_UNKNOWN` until reconciled. No general exactly-once claim is made for external effects.

Cancellation is not undo. `cancel_requested` means receipt; `cancelled` requires evidence that remaining work stopped. An already completed execution may remain `succeeded` despite a late cancellation. Partial effects remain recorded. Compensation is a new authorized action linked to the effect being remedied. Rolling back model weights does not reverse changed data or external actions.

## Models, data and a complete example

Model registration fixes source revision, artifact hashes, architecture, operators, quantization and engine compatibility. A behavior contract fixes observation, state, action, time and units, plus policy/world interfaces and reset/migration rules. Loading both files does not establish their semantic compatibility. Dataset versions retain provenance and permitted use; training creates a candidate, never automatic release authority.

Illustrative flow: a local actor proposes a candidate model set for a bounded canary. It uses `listCapabilities`, `createTask` and `createActionProposal`; a human inspects `getActionProposal`. If policy requires HITL, `createApproval` binds the exact proposal. `createExecution` schedules its `transitionRelease` action. HOTL observes `listEvents` and `getExecution`; HIC governs exposure, budget and delegation. Failed acceptance criteria trigger an authorized rollback proposal. No stage is claimed to have run here.

Required negative examples include a mismatched payload hash, expired approval, stale policy revision, unsupported operator and exhausted budget. They must fail before the unauthorized effect. Runtime interruption and unknown-outcome tests additionally verify truthful partial-effect reporting.

## Documentation and acceptance

Human documentation provides task tutorials, approval previews, inspection and recovery guides. AI documentation provides exact schemas, effect scopes, prerequisites, limits and valid/invalid fixtures. Both are derived from the same revision. Implemented quickstarts become “tested examples” only when matching code revision, environment, commands and observed results exist.

Conformance checks will compare generated surfaces, exercise authorization and replay semantics, and verify language parity for identifiers. Native, browser and GPU evidence remain separate; unrun checks remain `not_run` or justified `not_applicable`.

Related contracts: [architecture](../../architecture-v0.3.en.md), [team protocols](../../ai-team-protocols-v0.1.en.md), [learning and training](../../learning-data-training-protocol-v0.1.en.md), [NeedSpec schema](../../ai-team-protocol/request.schema.json), [synthetic request example](../../ai-team-protocol/request.example.json), [model-set release schema](../../ai-team-protocol/model-set-release.schema.json). Schema validation checks structure; authorization, reference integrity, compatibility and executed evidence require independent semantic checks.
