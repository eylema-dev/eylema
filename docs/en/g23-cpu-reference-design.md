# G23 bounded CPU reference — approved written design

Date:14October2026. Status: written design and concrete plan approved; G23-only numerical contribution profile admitted after Task0. Bounded reference implemented locally; final production review/CI acceptance pending. English canonical; [Turkish](../tr/g23-cpu-reference-design.md).

## Intent and authority

The maintainer approved narrow G23 evaluation/scope preparation after accepted G80. Build owned Rust CPU reference operators as a foundation for interchangeable local language/decision/perception/world/policy profiles. Each architecture retains independently verified operator/profile gates; one Qwen-like reference never establishes universal support. Subsequent actual maintainer decisions approved this written design and concrete inline plan, then admitted the G23-only numerical contribution profile after the private trial and separate reference review. Full models and performance remain outside those decisions.

Design-time core base `177e9d322b82682683ae84ec8e84a13a04de11e9`; guide base `2d76bf240da23682db2fd0f7f9e59ec89f52c5b5`. Activation rebinds accepted guide `ab74acc4b4af09b4f3a8c22b84b542ce0e3a9505`. Rebind current accepted G18/G27 evidence and current main before activation. At design time, the existing platform CPU addition was a test-only integer reference for the narrow Metal proof and no `eylema-tensor` crate existed. Existing contracts/model manifests and their support states stay unchanged.

## Approach

| Approach | Decision |
| --- | --- |
| Separate owned pure CPU reference crate | Select: independently test numerical/shape behavior; host admission and model decoding remain separate. |
| Put kernels in the existing platform Metal proof | Retain its narrow device proof; different semantics and lifetimes would obscure profile acceptance. |
| Import another tensor/inference engine | Conflicts with the current first-party production policy; this design introduces no such dependency. |

`crates/eylema-tensor` is safe owned Rust (`forbid(unsafe_code)`), a pure computation library with no filesystem/network/process/credential/hardware/provider access. Register only that first-party crate in root `Cargo.toml`/`Cargo.lock`. The r4 scope preparation permits these registration paths; no third-party production/dev crate or hidden engine. Existing task/EYL/journal/version/admission/host interfaces and qualified contribution records remain intact.

## Typed bounds and numerical contract

An owned `Tensor` contains finite F32 values and canonical contiguous row-major shape, rank1..4; positive dimensions and checked extent at most4096elements. Construction rejects shape/extent mismatch, zero dimension, overflow, NaN and infinity before computation. No strides/views/implicit broadcasts/quantization/BF16/FP16 layout support. Every operator validates all inputs and finite intermediates/output; errors never return a partial result. Allocated result buffers are bounded by the same extent; this crate does not reserve real device memory or change admission.

Reference computation uses explicit F32 operations in deterministic index order, without fused/SIMD/parallel reassociation. Matrix multiply-add count is checked and at most262144 per operation. Dot/normalization/softmax use that ordered F32 contract; independent expectations use hand literals and development-only Decimal80 or separately authored higher-precision arithmetic, never production helpers. Reference result thresholds are fixed before implementation: absolute error<=3e-5 or relative error<=3e-5 for the finite published fixtures, with exact shape/index/mask/ID/error outcomes. These are test tolerances, not universal numerical guarantees or performance claims.

## First explicit operator profile

Use the existing sixteen versioned Qwen contract operator IDs as the first finite reference coverage set; immutable specification hashes are supplied by the accepted profile. Exact public types/functions and fixture encodings are fixed in the human-reviewed implementation plan, not invented by a host adapter.

| Operator group | Required behavior and bounds |
| --- | --- |
| embedding gather / tied logits / linear without bias | Valid token rows, declared orientation, compatible dimensions; embedding vocabulary<=64,width<=16; linear rank2 and explicit matrix dimensions; logits use the declared tied table without hidden copies. |
| RMSNorm / SiLU / SwiGLU / residual add | Explicit epsilon>0; last-axis normalization, elementwise compatible shapes and finite results; SwiGLU follows the pinned gated-product definition. |
| reshape/transpose / dtype-layout | Checked equal extent and a true axis permutation; materialized row-major output; only F32 layout supported. |
| default RoPE | Explicit positions in0..4095, finite theta>0 and even rotary dimension; batch<=2,headcount<=4,sequence<=16,headwidth<=16; no default context/window/theta inferred from a model name. |
| grouped-KV repeat / causal mask / scaled attention / F32 softmax | Exact query-to-KV head ratio, explicit mask/positions and positive scale; same finite axis bounds. Softmax subtracts row max. Masked entries are typed mask bits, not nonfinite tensor values; a fully masked row rejects, never becomes NaN or guessed zero. |
| KV cache append / bounded greedy | Cache state is caller-owned, explicit current length/capacity<=16 and shape binding; an invalid append returns no replacement. Greedy uses finite declared scores and smallest original token ID on equal maxima; no generation loop or sampling. |

Each group binds the exact accepted source operator semantics. Unsupported operator/layout/profile stays explicit; no model format or another architecture is accepted merely because its name/role matches. Language, reranker and embedding manifests can share eligible primitives, but profile-level completeness/support stays with later loader/tokenizer/decoder/native-execution gates. Existing Qwen local profiles remain unsupported for complete native execution.

## Independent acceptance and supervised qualification

Keep original G23-P1/A1/CP1. Fixtures cover every selected operator with nonsymmetric matrices, distinct axes/token rows, shifted softmax, normalization, gated products, positional pairs, GQA/causal/padding masks, append rejection and deterministic ties. Negative cases include invalid shape/unit/axis/index/ratio/epsilon/theta/rank, extent/work overflow, NaN/infinity/nonfinite intermediates, full masks and unsupported identity/layout. Independent profile tests distinguish complete primitive coverage from missing operators; no end-to-end model or hidden fallback runs. A deliberate arithmetic/index/mask defect must be caught by its fixture, not by source-text assertions.

The current owner-qualified `sol61-high-contracts-2026-10-14` profile covers design/contracts/development tests/docs and expressly excludes numerical kernels. The maintainer approved preparing a bounded numerical evaluation using the same configured `gpt-6.1-sol/high`, with serving identity unknown. This design proposes an explicitly supervised trial-only CPU-reference contribution scope: independent vectors, finite negative regressions and genuine separate qualified review establish evidence; actual human admission of the evaluated CPU-reference scope is required before PR qualification/merge. A record string or this design does not self-authorize numerical production work. Freeze that actual decision and eligibility before code activation; missing qualification remains pending.

Run unchanged full native/repository/PR/fingerprint/whitespace gates with exact revisions, preserve existing startup-risk/failure records and report skipped/not_run truthfully. Operator correctness does not prove weights/tokenizer/full-model/Metal/Wasm execution/training/throughput or empirical memory performance. Any selected portable compilation requires explicit owned-package inventory; compilation is not target execution.

## Delivery and resources

Proposed paths: `crates/eylema-tensor/`, root `Cargo.toml`/`Cargo.lock` registration, paired G23 notes/design and translation fingerprints; necessary G23-only guide/card/source-pin/plan/dated evaluation records. No model-contract/schema/checker/permission/G40 change. Original120active minutes/3targetedrepairs/15separate-review minutes,zero provider/model/weight/training trials and new spending; qualification preparation does not reset these budgets. One scoped branch, private usage boundary refresh, independent exact-head review/currentCI/standing-authorized merge and identical-main-tree/mainCI evidence remain required.

Actual human written-design/plan/profile decisions preceded production activation. Follow the approved concrete plan and retained original budget; production still needs full local checks, current CI and genuine separate final-head review.
