# Bounded owned CPU primitive reference

English canonical; [Turkish](../tr/cpu-reference.md). G23 supplies an owned safe Rust F32 reference library, `eylema-tensor`, for independently tested small primitive operations. This does not establish a tokenizer, model decoder, weight loader, full Qwen execution, GPU/Wasm execution, training or throughput/memory performance. Existing model support statuses remain unchanged.

## Caller contract

`Tensor::new` owns finite contiguous row-major values, rank 1..4, positive checked dimensions and at most 4096 elements. Invalid rank, shape/count, checked overflow, NaN and infinity reject before copying. Values/shapes are immutable through the public API. Results are complete owned tensors or an error; inputs and old cache are never replaced on failure. Storage reservations are fallible and bounded. F16, BF16 and quantized layouts reject; there are no views, implicit broadcasts or dtype conversions.

Reference arithmetic uses explicit scalar F32 multiply/add in index order, with finite intermediate/output checks and no fused/SIMD/parallel reassociation. Matrix work is at most 262144 multiply-adds for an entire operation, including all three SwiGLU projections. This is bounded reference behavior, not an optimized engine. Standard Rust and its platform math/toolchain remain explicit inputs; using Rust does not eliminate the host OS.

## Primitive interfaces and evidence map

The first identity set is the sixteen `qwen3.*.v1` operators from the accepted original Qwen specification SHA-256 `acbbf68755dee5ca2b9d25bdcd3aa4206ce7156e032c38518a1fc66d7819fb06`. The [library](../../crates/eylema-tensor/src/lib.rs) exports concrete types/functions; [reference vectors](../../crates/eylema-tensor/tests/reference.rs), [failure vectors](../../crates/eylema-tensor/tests/failures.rs) and [identity boundaries](../../crates/eylema-tensor/tests/identity.rs) are separate behavior checks.

| Versioned operator suffix | Owned operation / independent proof |
| --- | --- |
| `embedding_gather`, `linear_no_bias`, `tied_logits` | Valid table rows, output-first `[out,in]` weights and tied table orientation; distinct table/token/matrix fixtures |
| `rmsnorm_fp32`, `silu`, `swiglu`, `residual_add` | Last-axis scale/epsilon, stable sigmoid branches, gate/up product plus down projection, exact-shape residual; literal normalization/activation/gated results and overflow rejection |
| `reshape_transpose`, `dtype_and_layout` | Checked equal extent, materialized true permutation, F32-only layout; coordinate-distinct transpose and unsupported precision cases |
| `rope_default`, `grouped_kv_repeat` | Declared positions/theta/rotary prefix, split-half pairs and exact head ratio; frozen frequencies and token/head ordering |
| `causal_mask`, `scaled_attention`, `softmax_fp32` | Explicit positions/valid keys, typed expanded masks, scale and last-axis normalization; future/padding denial, shifted scores, multiple heads, full-mask/nonfinite rejection |
| `kv_cache_append`, `greedy_bounded` | Caller-owned append capacity and head/token axes; old cache preserved on rejection, finite scores and smallest original ID at ties |

Embedding vocabulary <=64, width <=16 and token count 1..16. Linear takes rank2 `[rows,in]` and `[out,in]`; no bias. RMSNorm uses an explicit finite positive epsilon and a rank1 last-axis scale. SwiGLU validates total composed work before projections.

RoPE, GQA, attention and cache use batch <=2, heads <=4, sequence <=16 and head/value width <=16. RoPE positions are 0..4095; theta is finite positive and the rotary prefix is positive/even and within width. Unrotated channels are retained. Parameters are explicit: the source's default theta is 1000000 and RMS epsilon is 1e-6; no model name supplies defaults automatically.

Causal masks materialize `[batch,heads,Q,K]`; positions/valid-key lengths and all axes are checked. A typed false bit excludes a key. Softmax subtracts the allowed row maximum; a fully masked row rejects instead of guessing zero or producing NaN. Attention requires matching Q/K/V/mask axes and a finite positive scale; GQA repeats must be requested explicitly.

Cache capacity is 1..16. First append binds batch/head/key-width/value-width; later appends concatenate token lanes per head within capacity. `len`/`is_empty` and immutable key/value getters expose state. Callers must isolate/drop cache on model/role/step identity changes; this pure primitive cannot authorize state transfer. Greedy accepts 1..64 finite scores with equally sized unique original IDs. It selects an ID, not an EOS/generation/sampling loop.

## What coverage means

`reference_coverage` checks the immutable spec hash, recognized unique IDs and at most sixteen entries. A complete set means **primitive identity coverage**, not a native-model support decision. The original model's head count16/width128 and BF16 input are outside this small profile even when identity coverage is complete; the identity test demonstrates their rejection. Another architecture, format or training operator needs its own acceptance.

Expected values are hand literals or independently authored higher-precision references. Finite published fixtures use absolute error <=3e-5 **or**, with nonzero reference, relative error <=3e-5. Zero reference uses only absolute error. Shapes, masks, errors and IDs are exact. These tolerances are fixture limits, not universal accuracy promises. Separate disposable-source mutations of arithmetic, transpose and masks must be caught by behavioral fixtures; original source remains intact.

The maintainer approved the written design, concrete inline plan and G23-only numerical contribution profile after a private three-group/13-test development trial and separate Decimal80 review. That governance decision does not accept production by itself. Final full local checks, current CI and genuine separate reviewed-revision evidence are required before standing-authorized merge. No third-party production/dev crate, hidden inference/training engine, weight/model/provider/training trial or permission change is introduced.
