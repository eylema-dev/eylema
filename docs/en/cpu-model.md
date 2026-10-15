# Bounded owned CPU model execution

[Turkish](../tr/cpu-model.md). Corrected-source G30 bounded pretrained-model acceptance passed on the named host below after the independently reviewed scalar-work repair. Final evidence review, current delivery CI and coordinated main acceptance remain pending.

The `eylema-model::cpu` API admits only the separately identified Qwen CPU manifest. It binds the complete 311-tensor BF16 layout, immutable G28 snapshot and original G29 tokenizer parent. Every stored scalar must be finite; the physically stored `lm_head.weight` must equal the embedding bytes. The model borrows one snapshot and tokenizer; sessions copy only bounded view metadata and own fresh KV/workspace. G23 operators and generic unsupported-local assignment are unchanged.

`qwen_layout` supplies the exact layout for the existing loader. `CpuModel::admit`, `begin`, `CpuSession::prefill` and `decode` provide admission and numerical execution. `CpuLogits::values` and `last_layer_hidden` expose numerical diagnostics, never accepted role judgments. `max_new_tokens` bounds sampled proposals from `generate` and reserves prompt capacity. Direct `decode` appends numerical input and returns logits only; it remains context/work/deadline bounded and grants no generated-text allowance. Model, tokenizer and manifest borrows must remain alive. Failed computation or repeated prefill poisons the session, including partially appended caches.

Callers supply context1–64, generation1–8, at most2560MiB logical resident content,80billion work units and a monotonic deadline no longer than600seconds. Bounds are checked before known allocations/reads and throughout blocks no larger than4096charged units. A shared control tracks cancellation, deadline and atomic cumulative work; limits only tighten. Reuse does not reset progress. Independent concurrent jobs use independent controls. Artifact capacities, conservative128MiB tokenizer/frontend allowance,4MiB loader allowance,128KiB metadata allowance, complete KV/workspace and escaped logit buffers are accounted conservatively. Admission work is exposed separately; each session/proposal reports its own control's work. Peak logical reservations are distinct from measured RSS, allocator overhead, OS memory and throughput. There is no global job scheduler or secure-erasure promise.

Numerical work counts each arithmetic/transcendental/finite-or-comparison/read-conversion operation before its bounded block: RMS costs6n+4, residual addition2n, attention per head at prefixp/widthw costs p*(2w+3)+(6p+1)+w*(2p+1). Read conversion remains conservatively charged in synthetic tests. Exact/one-below tests exercise real kernels; an exhausted block is rejected before its operations. Fixed control/index bookkeeping remains outside this scalar metric.

F32 arithmetic uses ascending scalar reductions, full-domain RMS/softmax, per-head Q/K normalization, half-split RoPE, contiguous GQA, both residuals, SwiGLU and tied logits. Query width2048 differs from hidden width1024. Small frozen Decimal80 vectors cover1218final-state/logit/KV values, including head width128. Exhaustive BF16 conversion, context/resource failures, prefix cache consistency and real mid-computation cancellation have local regression coverage. These synthetic tests do not establish pretrained-model quality or performance.

`generate` accepts separate reasoning, planning, specialist and critic request variants. Critic receives a bounded fixed-order JSON candidate/evidence object through ordinary typed user formatting. Each call starts a fresh session. Output variants and immutable receipts bind the exact role interface, CPU/tokenizer/artifact identities, numeric profile, token counts, work, logical peak and EOS/token-limit outcome. Text is an **unverified proposal**, with no calibrated verdict, score, tool action or permission. Empty, invalid UTF8 and control-sentinel output reject. Debug output contains types, counts and public identity digests, never prompt or generated text.

The additive `encode_prompt_controlled`/`decode_controlled` tokenizer paths preserve legacy G29 results and propagate cancellation inside normalization, BPE and decoding. G29's4096token input bound and128token reservation stay independent of the smaller CPU prompt-plus-generation check. No truncation, fallback, model download or hidden engine is part of the library.

## Historical bounded local run

The following immutable measurements belong to pre-repair source`7bd5debc4745ffda9abb36a81790b1c28511d5e5`. Its work counters omitted finite predicates, square roots and divisions in the kernels named above; historical work totals are undercounted and cannot establish the corrected resource contract. They have not been recomputed or relabelled as repaired-source execution. Numerical/role/stop observations remain historical evidence. The original six-job allowance is exhausted; a corrected-source repeat requires explicit additional owner authorization and reuses the already verified weights/reference.

At that source, AppleM4Pro/macOSARM64 with24GiB unified RAM, Rust1.99.0 and CPython3.14.8: one public1503300328byte artifact was downloaded in162.19seconds and whole-file SHA256`f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b` verified. The existing accepted G29 tokenizer/config were independently rehashed. No account, provider, GPU or training engine was used.

One independent F64 job computed28layer states and151936logits in24.15seconds. The owned Rust release harness ran exactly six candidate jobs in59.40seconds, including snapshot/tokenizer/admission setup. All180608hidden/logit comparisons passed the frozen per-value bound`absolute_error <= 0.001 + 0.0002*abs(reference)` with identical greedy token21806. Maximum hidden absolute error0.007344306 and logit absolute error0.000050857 are reported without substituting a maximum-only test for the per-value relative bound.

| Candidate | Input/generated tokens | Unverified output UTF8 bytes | Work units |
|---|---:|---:|---:|
| Reasoning |21/8|22|50240061792|
| Planning |19/8|35|46645938084|
| Specialist |23/7|9|53835117641|
| Critic |30/8|26|66425102697|

All four actual output digests differ and variants/receipts match their roles. These short public-input outputs establish plumbing/smoke behavior, not expert or Turkish task quality. The sixth job observed numerical work after workspace initialization, cancelled, returned no proposal and rejected cache reuse. Admission consumed3317825536work units; the full six-job total including admission was222263133222, below320billion. Each job stayed below80billion. Conservative peak logical content was1658000646bytes. Per-job sampled native process RSS ranged1209253888–1538179072bytes; F64 supervisor sampled peak process RSS1214775296bytes including mapped pages. Samples are not universal fit/throughput or OS isolation guarantees.

Four temporary actual-source mutants—interleaved GQA grouping, wrong half-rotation sign, stale prefix keys and ignored cancellation—each compiled and failed the frozen relevant behavioral test. Every restoration passed the twelve-CPU-test suite, and the original source tree was restored exactly. Asset-dependent ordinary CI skips remain explicit; the actual acceptance above was a separate authorized local run. Private receipts, logs, model bytes and numerical reference arrays remain outside Git. GPU, training and larger context execution remain not_run.

## Corrected-source actual acceptance

The maintainer explicitly authorized six additional local Rust jobs after the scalar-work repair; the lifetime total is12Rust candidate jobs, one artifact download and one F64 reference. Source`93bed57af6cbc8a33e619475e1bcedc88a06a0bd` ran the same six-job harness once, reusing the locally rehashed artifact/tokenizer and unchanged stored F64 reference. No new reference computation, download or provider/training call occurred. Historical receipts/logs were preserved byte-for-byte. The supervisor used its own process group, a1200second deadline and1MiB log cap; no termination occurred.

All180608per-value hidden/logit comparisons passed the original frozen bound, with identical maximum errors and greedy21806above. All four role output digests are unchanged from the historical run, remain distinct and have the same token/byte counts. Corrected work receipts are:

| Candidate | Work units |
|---|---:|
| Numerical |1795363620|
| Reasoning |50248076316|
| Planning |46653333550|
| Specialist |53843758391|
| Critic |66435991686|
| Effective stop |3950042|

Admission remains3317825536units. Corrected aggregate work222298299141is below320billion, and every job stays below80billion. Effective stop observed numerical progress, returned no proposal and poisoned cache reuse. Native harness elapsed60.69seconds (supervised process61.24seconds); sampled native RSS1206648832–1538965504bytes and conservative logical peak1658000646bytes are separate measurements. Available-memory preflight estimated4031856640bytes from free/inactive/speculative OS pages, not a hard reservation or universal fit guarantee. This proves the fixed small CPU numerical/generation/control path only; model usefulness, calibration, larger contexts, other architectures, GPU and training remain unproved.
