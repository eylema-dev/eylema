# Declared resource estimator

G80 adds a pure owned Rust sizing module beside the existing [resource manager](resource-manager.md). [Turkish](../tr/resource-estimator.md). The [approved design](g80-resource-estimator-design.md) fixes scope; the [technical plan](https://github.com/eylema-dev/devguide/blob/main/docs/plans/g80-resource-estimator.md) records its implementation and acceptance gates.

## Caller contract

`eylema_runtime::resource_estimate::component_interval` sizes one declaration; `estimate` combines a borrowed `Snapshot` into a fixed-capacity `EstimateReport`. Neither allocates heap/device memory, obtains leases, probes hardware, changes admission, downloads weights, runs models/training, calls a provider, routes work or purchases equipment. Existing task/EYL/journal/host interfaces stay unchanged. Native execution eligibility, measured performance, calibration and investment recommendations have separate gates; `planning_only` is always true.

The caller supplies every required component, coefficient, placement and overlap. This API cannot discover omitted requirements or authenticate publisher/hardware/source identities. A complete declaration is not a complete physical measurement. Use `Claim::Unknown` for missing coefficients; explicitly absent components use `Formula::NotApplicable`. Required resident weights cannot be not applicable. Zero metadata or zero reserve can be an explicit known value; absent information cannot become zero.

Every known formula/interval carries `Evidence`: origin (`MeasuredArtifact`, `PublisherDeclaration`, `SuppliedAssumption`), bounded source reference, positive revision and inclusive validity window. Mixed provenance needs separate components or nested metadata claims. The report retains component evidence and an immutable borrowed snapshot, including original formulas/nested provenance. Requested context and batch are sizing settings; they do not describe measured use. Labels, profile names, fingerprints and estimates never grant rights, authority or trust.

`InstanceKey` includes profile, revision, artifact digest, backend, owner scope and physical pool. Each resident binds its full `expected_key`; snapshot identity/revision also bind expected values. Different roles share one resident ID only when this complete instance is shared. Duplicate exact instances reject ambiguous deduplication; different owner/artifact/revision/backend/profile/pool keys count independently. Each invocation retains its own state/workspace. Missing artifact identity, stale evidence or requested-identity mismatch prevents definite fit.

## Explicit formulas

All units are typed bits/bytes/counts/seconds. All arithmetic uses checked `u64` intermediates, never floating-point, wrapping or saturation. Overflow is an error even if wider arithmetic could represent the final result. Byte packing and rational scaling round upward, including interval lower endpoints. Intervals are conservative declared bounds, not confidence intervals.

| Formula | Rule and required declarations |
| --- | --- |
| `Packed`, `Weights` | Continuous bit packing: ceil(elements × bits / 8). Weights add explicit metadata bounds and use total resident parameters; active MoE parameters never replace resident parameters. |
| `Quantized` | Packed parameter bytes + ceil(parameters / group_size) × scale_bytes + metadata bounds. Tail groups count; row alignment, additional codebooks/zero points and source/destination/staging need explicit sizes. Unsupported NF4/custom layout stays unknown, not inferred from bits. |
| `Kv`, `Recurrent` | Full requested context × batch × layers × KV heads × head width × 2, then cache-bit packing. Recurrent uses its own state-elements/batch/precision and explicit rational coefficient; no dense-cache fallback. |
| `RagVectors` | count × dimensions × precision, then packing; index, retained sources, working buffers and persistent storage are separate. |
| `Lora` | Sum rank × (input + output) across audited targets, then adapter-bit packing and explicit copies ratio. Rank must fit both dimensions. Gradients/optimizer/base/activation/checkpoints/temp are separate; no universal coefficient. |
| `Bytes`, `Scaled` | Caller-supplied byte interval, optionally multiplied by an explicit positive rational coefficient, with endpoints rounded upward. Modality/workspace/index state needs audited or supplied intervals. |

`Quantized` is a declared packing formula, not recognition or support of a weight format. QLoRA sizing combines a declared quantized base with separate adapter/gradient/optimizer/activation/checkpoint/temp requirements. Conversion inputs/outputs/staging declare their lifetimes and physical pools; do not also add the same resident allocation under an auxiliary name. The estimator does not infer that two unrelated component IDs are the same allocation. Image/video/STT/TTS buffers use explicit auxiliary components; new architectures use explicit recurrent/declared coefficients or `UnsupportedArchitecture`. No kernel, training implementation or model support is implied, including the selected Qwen profiles.

## Cooperation and physical memory

Use at most16 resident instances,32 invocations,32 overlap groups and64 auxiliary components; the implementation also bounds8 pools,32 members per group and32 LoRA targets. Output holds at most144 component entries. Entry IDs are globally unique; group IDs and pool IDs are unique within their namespaces. Identifiers/source references are nonempty ASCII, at most128bytes, using letters/digits and `._:/@-`. An explicitly unknown missing identity may carry an empty source reference. No free-form text/credentials are required.

Each pool sums resident weights and persistent auxiliaries once, then adds the maximum summed invocation/transient-auxiliary lower and upper bounds across explicit groups. A member may occur in several alternative groups but cannot repeat inside one group. Every invocation/transient auxiliary must occur somewhere; persistent auxiliaries cannot be group members. Missing schedule (`None`) makes every pool unknown. An explicit empty schedule is valid only without transient work. An incomplete schedule leaves affected peaks unknown. Concurrent states are separate from shared weights.

Unified RAM/VRAM is one physical pool. Discrete host RAM/device VRAM have distinct physical IDs; duplicate physical declarations reject alias double counting. Storage is separate from memory. Capacity, gross currently available bytes and OS/allocator reserve are distinct claims: usable budget = available − reserve. Capacity alone never establishes current fit; stale/missing capacity/available/reserve cannot yield a budget. Available above capacity or reserve above available is a conflicting declaration.

| Result | Meaning |
| --- | --- |
| `EstimatedFit` | Complete upper bound <= known usable budget in every pool. |
| `ExceedsDeclaredBudget` | A complete lower bound exceeds a known usable budget in at least one pool; another pool cannot absorb the shortage. |
| `Unknown` | Budget crossing, missing required coefficient/schedule/budget, stale evidence or identity mismatch. Known component subtotals remain visible; incomplete total is `None`, not zero. |

Overall definite shortage takes precedence over unknown pools; otherwise any unknown prevents fit. `snapshot_status` carries snapshot identity/schedule uncertainty. Pool-budget reasons remain inspectable through the borrowed original pool claims. Invalid units/shapes/denominators/limits/duplicates/dangling references/conflicts and arithmetic overflow return `EstimateError`; uncertainty is distinct from malformed input. This is declaration consistency, not source authentication or an admission guarantee.

## Independent synthetic example

One unified pool with usable budget1000 and a separate storage budget400:

- Quantized base12..14bytes:10parameters,4bits,group4,scale2bytes,metadata1..3bytes.
- Persistent adapter40,gradient40,optimizer80,recurrent72,vector48,index16bytes.
- One overlapping invocation:activation100/workspace10; checkpoint20,modality8,RAG-working4bytes.
- Retained sources300bytes in storage.

The literal expected memory total is450..452bytes and storage300bytes; both estimates fit. Missing optimizer coefficient makes memory total `None`/`Unknown` while storage retains its separate result. These tiny synthetic numbers prove arithmetic/aggregation, not useful model execution or empirical resource sizing. Another fixture shares one1000byte resident across two independent invocations: singleton groups peak1220bytes; their joint group peaks1330bytes. Two distinct owner instances peak2330bytes.

## Verification boundaries

Native fixtures exercise the real module using independent literal expectations: dense/MoE, packing/quantized tail/rational rounding, KV/recurrent, RAG/adapter/training declarations, missing/stale/invalid/overflow, exact identity/sharing, overlap, unified/discrete/storage, crossing budgets and maximum/rejected limits. Existing admission tests retain lease/cancellation behavior. Full repository/native gates, exact-head separate review and latest CI/main evidence are recorded in the delivery PR/current G80 card. Static document checks alone prove none of these runtime outcomes.

The previously observed unchanged G67 development-host worker-startup timeout remains a reproducibility risk; no threshold/test/source correction or waiver is part of G80. Physical allocations, actual models/weights/tokenizers, GPU/model/training performance, complete workload discovery, Turkish retrieval quality, calibration and investment outcomes are not accepted by this sizing module.
