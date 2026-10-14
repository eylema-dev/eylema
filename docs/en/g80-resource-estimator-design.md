# G80 resource estimator — written design for human review

Date: 14 October 2026. Status: proposed design; no estimator implementation or acceptance. English canonical; [Turkish](../tr/g80-resource-estimator-design.md).

## Purpose and existing foundation

Implement the planned G80 checked resource estimator in owned Rust. Help Eylema users compare local specialist portfolios using resident/active parameters, precision, context, batching, overlap, RAG and model-specific training requirements. Produce traceable memory/storage intervals and unknowns; actual performance calibration and investment recommendations remain G81/G82. [Accepted planning direction](https://github.com/eylema-dev/devguide/blob/main/docs/en/model-resource-planning.md) and [G80 card](https://github.com/eylema-dev/devguide/blob/main/records/platform-goal-plan.json) retain original A1/CP1/R1, dependencies and budgets.

The existing [resource manager](../../crates/eylema-runtime/src/resource.rs) accounts for caller-declared transient reservations, queue/active limits and cooperative cancellation. The accepted model manifest validates dimensions/extents and reports unsupported execution. Neither currently predicts complete resident model or training requirements. This design extends that foundation with a distinct, pure sizing layer. Existing admission behavior, EYL/task/journal versions and authorization stay unchanged.

## Approaches considered

| Approach | Tradeoff | Decision |
| --- | --- | --- |
| Owned pure estimator alongside the resource manager | Typed, deterministic calculation can be tested without allocations, models or devices; callers explicitly consume results | Recommended |
| Add estimation directly to admission | Fewer call boundaries but couples uncertain forecasts with existing lease/cancellation behavior | Retain admission as its current bounded execution contract |
| Put all sizing inside the model manifest | Close to artifact identity, but portfolio/RAG/training/pool logic exceeds the manifest's declarative responsibility | Manifest supplies identity/dimensions; estimator owns sizing |

## Inputs, identity and finite bounds

A planning snapshot contains explicit physical memory pools, model instances, invocation/overlap groups and optional RAG/training/storage components. Every value has units and evidence provenance: measured artifact, publisher declaration, supplied assumption or unknown. Requested settings are distinct from measured use. No source/model string, profile status or output estimate grants execution or data access.

Proposed limits: at most16 resident model instances,32 invocations,32 explicit overlap groups and64 auxiliary components. All identities and referenced profiles are bounded, versioned and uniquely resolved. Model instance keys include profile/artifact fingerprint, revision, backend, owner scope and physical pool; shared weights are counted once only for the same declared instance. Separate contexts/activation states remain separate. Reusing weights across roles does not infer shared cache or cross-project sharing.

Freshness checks bind the exact requested snapshot/expected profile revision and explicit validity window. Unknown, missing, stale, conflicting or mismatched identities cannot yield a definite fit. These are declaration consistency checks, not authentication of publisher, hardware or human permission.

## Checked sizing and uncertainty

| Component | Required explicit inputs and rule |
| --- | --- |
| Dense and MoE weights | Total resident parameter count, audited bits/packing and metadata interval; active MoE parameters describe compute scope and never replace total resident weights |
| Quantized storage/conversion | Explicit grouping/scales/metadata and source/destination/staging residency; unsupported layout or missing overhead leaves the relevant component unknown |
| KV/recurrent state | Requested full context/batch, layers, K/V heads/head width and cache precision; recurrent state has its own explicit formula/coefficient profile |
| Workspace/modality | Audited or supplied intervals per invocation; image/video/STT/TTS buffers are explicit auxiliary requirements, no universal multiplier |
| RAG | Vector count/dimension/precision plus separate index, retained source, working and persistent storage intervals |
| LoRA/QLoRA/training | Audited target shapes/rank, base and adapter storage, gradients, optimizer, activation/checkpoint and temporary state; unsupported target or unknown coefficient stays unknown |

Use checked integer arithmetic and rational coefficients, round byte packing upward, reject invalid units/shapes/denominators and overflow; never wrap, saturate or silently convert unknown to zero. Unknown required coefficients yield an incomplete estimate with named reasons and no definite-fit verdict. A genuinely absent component is explicitly not applicable, not missing. Formula provenance and assumptions accompany every component subtotal and interval.

Intervals are conservative declared bounds, not statistical confidence. Generic dense/MoE/quantized/recurrent sizing is a what-if calculation profile, never proof that the Rust engine supports that model family. Current selected Qwen language/score/embedding profiles still have unsupported native execution; a memory estimate cannot promote them.

## Cooperation, pools and result

Resident weights and persistent model states are combined with the maximum summed invocation/auxiliary interval across explicitly declared overlap groups. Omitted overlap information leaves peak unknown; the estimator does not invent a schedule. Shared weights deduplicate by exact instance, not merely model name or role. Persistent storage is reported separately from RAM/VRAM.

Unified memory is one shared physical pool. Discrete host RAM and device VRAM are separate pools with explicit placements and transfer/staging requirements; neither may absorb the other's shortage silently. Capacity, available budget and OS/allocator reserve are distinct inputs. A reported total capacity without known available budget cannot prove current fit. Duplicate unified-pool declarations or conflicting physical identities reject double counting.

Results carry per-pool/per-component lower and upper bounds, required unknowns, assumptions, source identities and one of estimated_fit / exceeds_declared_budget / unknown. Runtime eligibility and measured performance remain separate fields/boundaries. An interval crossing the available budget is unknown; an incomplete interval never becomes estimated_fit. No allocations, leases, model downloads, model/provider calls, training, hardware probes, automatic routing, purchases or in-flight reassignment occur.

## Ownership and compatibility

Proposed implementation location: `crates/eylema-runtime/src/resource_estimate.rs`, its small module export in `src/lib.rs`, independent native tests under `tests/resource_estimate.rs`, and paired documentation. This is a new sizing subsystem; exact types/call contracts will be frozen in the implementation plan after human design review. Current `resource.rs` admission, model decoder, source schemas, production dependency inventory and host/browser/CLI interfaces remain unchanged. Devguide tracks the current card, scope, accepted dependencies and source pins.

G80 depends on accepted G27/G46 and preserved G18/G21 evidence; exact immutable remote delivery/CI and current base must be rebound before code activation. Source development uses the owner-qualified gpt-6.1-sol/high contracts/validators/development-test/document profile; hidden serving identity unknown. Tensor kernels and inference/training performance are separate qualification domains. No third-party production crate or hidden inference/training engine.

## Acceptance and delivery

Independent hand-derived fixtures cover dense/MoE resident versus active, packed/quantized metadata, requested context/batch and recurrent state, RAG/storage, LoRA/QLoRA/training state, shared models across roles, overlap peaks, unified/discrete placement and budget-crossing intervals. Failure fixtures cover overflow, invalid units/shapes, stale/conflicting profile, missing coefficients, duplicate instances/pools and unknown model types. Tests must detect a real arithmetic/identity/aggregation error; expected values cannot call the estimator under test.

Run exact local repository gates plus the full unchanged native Rust gate; retain failures and actual evidence. Preserve G67's previously observed unchanged worker-startup timeout risk; it was not fixed by metadata delivery. Genuine separate final-head review and latest CI precede standing-authorized PR merge; identical reviewed/main tree and exact-main CI precede accepted delivery. Every other Goal/history/G40 allowance stays unchanged. Source/model/CI/estimate text never authorizes work.

Implementation budget remains120 active minutes/3 targeted repairs/15 separate-review minutes, zero new spending/model/provider/weight/training trials. This written-design preparation is documentation only; code activation waits for actual written-spec approval and then the human-reviewed implementation plan using the established inline method. Private usage ledger refreshes at the boundaries; Standard API equivalent is not an actual subscription bill and unknown pricing/coverage is retained.
