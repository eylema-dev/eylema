# G28 bounded owned weight-loader design

English canonical; [Türkçe](../tr/g28-weight-loader-design.md). Written design for human review, **not production implementation or acceptance**. The maintainer approved scope/design preparation on 14 October 2026; the written design and G28-only profile binding are now approved; concrete-plan review remains pending. Source base `6605080fcae225e099775f7856349c0aac6aa011`, guide base `affbcd456fc467b10155eb3b71b4860042220b30`; core issue [G28 / #25](https://github.com/eylema-dev/eylema/issues/25).

## Purpose and contribution boundary

Load and validate actual artifact bytes with owned Rust, so a later local model engine can consume an immutable, identified weight snapshot. G09 platform/codec and G27 manifest remain accepted dependencies. Preserve G28-P1/A1/CP1, original required dependencies and 120 active minutes / 3 targeted repairs / 15 separate-review minutes; all preparation consumes that same allowance. No model/weight download, provider/training trial, new spending or third-party Cargo package. G40 and every other card remain unchanged.

Architect/author/verifier setting is recorded `gpt-6.1-sol/high`; latest completed response metadata confirms that configuration, not hidden serving identity. A genuine separate qualified reviewer and final-head evidence remain required. Existing `sol61-high-contracts-2026-10-14` covers owned contracts/validators, development tests and bilingual docs, valid through 13 November 2026. Structural artifact validation fits that declared domain. New incremental native binding is not explicitly listed in its G27 evidence: **the actual owner confirmed that G28-only binding after reviewing this written design; concrete-plan approval and executed quality gates remain required**. This read-only assessment does not qualify new numerical/cryptographic kernels, performance or another Goal. The G23-only numerical profile cannot qualify G28. No existing qualification record is expanded by this design.

## Scope and architecture

The approved preparation adds only root `Cargo.toml`/`Cargo.lock` registration of `crates/eylema-model/`, plus `crates/eylema-platform/src/lib.rs`, new `src/sha256_stream.rs` and `tests/sha256_stream.rs` in that platform crate. Keep existing paired documentation/fingerprint scope. The model crate is safe Rust with `forbid(unsafe_code)` and only owned contracts/platform dependencies. Proposed modules separate bounded header parsing, expected layout, snapshot loading and error categories; no build script, external parser/inference engine or copied implementation.

Input is an already validated immutable G27 `ModelManifest`, a separately declared exact `WeightLayout`, an explicit `LoadBudget` and a caller-owned `Read` implementation. The loader never opens a path, follows a link, downloads a file or grants project access. The caller must supply authorized input and a reader with a bounded blocking/deadline policy; a generic synchronous `Read` cannot itself guarantee wall-time cancellation. Load budget is a logical byte/work cap, not host authority or physical capacity evidence.

The layout binds the canonical manifest digest, its exact configured weights component, supported stored precision and a sorted, unique complete tensor map. Constructors validate and privately own the map. They do not establish that a caller-supplied map is the complete architecture of a real model. Returned results bind both declared layout and actual byte identity, without changing `check_assignment` or any unsupported local-model status. Missing source, rights, expected-map provenance or architecture conformance remains a separate model admission gate.

Snapshot loading keeps one owned artifact buffer. Read and validate the small header first; reject bad metadata before allocating the whole artifact. Reserve the artifact fallibly once, append the exact prefix/header, then read payload in at most 64 KiB chunks into bounded scratch storage. Hash every prefix/header/payload byte in original file order with the native stream. Reject early EOF, extra data, reader errors, excess attempts or digest mismatch; return no partial result. Once full validation succeeds, expose borrowed read-only tensor byte slices from that owned buffer. Changing or closing the caller's original file afterwards cannot change the returned bytes. This is an in-process snapshot, not durable storage or guaranteed erasure.

## Fixed profile bounds

| Bound | Initial contract |
|---|---|
| Whole artifact, including header | At most 2 GiB; exact positive component size from manifest |
| JSON header | At most 1 MiB; prefix extent checked before allocation |
| Tensor count / rank | 1..1024 / 1..4; every dimension positive |
| Stored tensor precision | Uniform `BF16` or `F32`, row-major, little-endian; manifest `safetensors.v1`, `bfloat16` or `float32`, `none` quantization must match |
| Tensor name | 1..128 ASCII bytes, existing flat-name character subset; unique decoded names |
| Metadata | Optional `__metadata__` string map; at most 128 pairs, keys <=128 decoded bytes, values <=4096; never authority or emitted logs |
| JSON depth / value nodes | At most 6 / 16384, counted before descent/insertion |
| Reader calls / interruptions | At most 65568 total calls, including final EOF probe; at most 16 interrupted retries |
| Resident content budget | Artifact bytes plus fixed 4 MiB bounded header/parser/layout/scratch allowance; checked before artifact reservation |

Caller resident budget must cover that checked sum, and artifact cap must cover exact manifest component size. All explicitly owned growable content buffers reserve fallibly within their bounds. Allocator/OS bookkeeping, caller's already-owned input, other models, global OOM and observed RSS are outside this logical accounting; no exact physical peak or successful 2 GiB allocation is inferred. The retained Qwen component's declared size is 1503300328 bytes, within the proposed ceiling; the real artifact has not been downloaded, loaded or verified here. No 2 GiB allocation is made during G28 acceptance.

Reject unsupported legal Safetensors features explicitly: scalar/empty tensors, packed/sub-byte dtypes, quantized layouts and sharded indexes. BF16/F32 payload remains opaque; no float conversion, NaN/Inf screening, tensor arithmetic, tokenizer or model execution is supplied. Later conversion/operator acceptance must enforce finite numeric values and precision semantics. No fallback or automatic model promotion.

## Header, tensor and identity checks

The official format has an unsigned little-endian 8-byte header length, UTF-8 JSON beginning with `{`, tensor-relative half-open data offsets and a fully indexed contiguous payload. Read the fixed publisher [format specification](https://github.com/safetensors/safetensors/blob/e246a2560645b7525f5775669ed816eb57c5bcc8/README.md#format); no external library is installed or executed.

The owned parser accepts only objects, strings, unsigned decimal integers and arrays needed by that selected profile. Integers reject signs, leading zeroes, fractions/exponents and checked overflow. JSON whitespace and string escapes/UTF-8 are handled explicitly; decoded keys determine equality, including escaped ASCII aliases. Invalid escape/surrogate/UTF-8/control sequences reject. Header padding after its final object is only ASCII spaces. Duplicate keys at any object depth reject. Each tensor object has exactly `dtype`, `shape`, `data_offsets`; unknown/missing/duplicate fields reject. Metadata values must be strings; metadata is validated and discarded, not interpreted or retained as instructions.

Checked dimension product times dtype width must equal `end-start`. Offsets satisfy `0 <= start < end <= payload_length`; sorting intervals must yield exactly `[0,payload_length)` with no hole/overlap or unindexed suffix. Name/dtype/shape must match the declared map exactly; no missing/extra tensor. Precision cannot change by merely accepting matching byte lengths. Whole-file length and SHA256 must match the exact manifest component. Digest source stays publisher-advertised/measured as originally declared; actual byte comparison is separately recorded and is not publisher authentication, rights authority or model eligibility.

Proposed public interface names (signatures fixed in the concrete plan):

```text
StoredDtype::{BF16,F32}; TensorSpec; WeightLayout; LoadBudget;
LoadedWeights; LoadError;
WeightLayout::new; load; LoadedWeights::tensor_bytes;
Sha256Stream::new; Sha256Stream::update; Sha256Stream::finish;
```

Private fields and read-only borrowed accessors prevent unchecked mutation; no `DerefMut`, mutable tensor bytes or parser state escape. Categorized failures include invalid profile/header/layout, size/work/resident bound, missing/extra tensor, hash mismatch, allocation failure, I/O and unavailable/failed platform service. Never log raw artifact/header/metadata, private paths or caller data.

## Native SHA256 lifecycle and explicit platform input

Keep existing one-shot `sha256` and its 4 MiB wire limit unchanged. New stream accepts a declared byte limit 1..2 GiB, individual chunks <=1 MiB and checked cumulative count. An empty update is a bounded no-op; an empty digest remains valid for reference tests. Oversized/overflow/failed service update poisons the stream; subsequent update/finalization rejects. Finalization consumes the stream, so reuse is impossible through the safe API. No cloning of live native state or host pointer retention.

macOS CommonCrypto remains the provider. The inspected macosx27.0 SDK declares `CC_LONG` as `uint32_t`, `CC_SHA256_CTX` as count[2], hash[8], wbuf[16] of that type; expected size104/alignment4 with a C-compatible owned context. `CC_SHA256_Init/Update/Final` return int and must return1. Use narrow documented unsafe calls only in the platform crate, with a live aligned context, checked chunk conversion and borrowed input/32-byte output lifetimes. The context fields contain no pointers; no SDK implementation is copied. All context access outside FFI remains private and safe. Unsupported native/Wasm targets return `Unavailable`; selected existing Wasm compilation remains separate from execution.

Primary [Apple API documentation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html) and SDK `usr/include/CommonCrypto/CommonDigest.h` were read only; header SHA256 `83a9705bbc8d44f27ee61801064ee99288496a038d623f152aa30db6819e9ca6`. Native calls, ABI execution and failure behavior are **not_run** at this design stage. Required later checks independently establish SDK layout and known digest/chunk/lifecycle behavior before accepting the wrapper. No authored crypto kernel, external crate, host capability opening or change to existing browser hashing.

The later ABI check uses one temporary development-only C layout assertion against the installed SDK (at most 30 seconds and 64 KiB output), plus Rust size/alignment checks. It neither ships C production code nor calls a model/hash provider. Existing SDK/Clang tooling is an explicit development input; no installation/build script is added. A compile/read-only SDK declaration alone does not establish native digest behavior.

## Acceptance, delivery and remaining human gates

| Criterion | Concrete-plan evidence required |
|---|---|
| G28-P1 | Positive small synthetic BF16/F32 byte loading; exact layout/whole-file identity and immutable returned snapshot; malformed offset/dtype/shape/profile rejection |
| G28-A1 | Independently frozen container bytes/digests and reference tensor slices; duplicate/escaped/unknown fields, holes/overlap, zero/overflow shape, missing/extra/changed bytes, short/extra/error/interrupted input and budget rejection |
| G28-CP1 | Actual eligible task-scoped model/role/owner binding, genuine final-head review, current local/CI acceptance and standing conditional merge authority |
| Native dependency | Independent empty/abc/standard long SHA256 vectors, split patterns/block boundaries, >4 MiB synthetic stream comparison, poison/limit/unsupported-target and SDK ABI checks; old wire-limit regressions unchanged |

Use owned synthetic fixtures only, each <=8 MiB, no more than32 MiB aggregate simultaneously resident synthetic input/output plus bounded metadata in a test process. Independently freeze Python-standard-library reference bytes/digests before Rust checks; Python is development tooling, not a production parser/hash engine. Actual reader-error and source-mutation regressions must be demonstrated; no source-text assertions or fabricated RED. Large size/overflow cases use metadata/fake readers and fail before allocation, not huge files. Count exact executions/skips and preserve failures. A tiny fixture cannot establish full Qwen architecture/layout, large-file capacity, throughput, model or training support.

After human written-design approval, write the concrete inline implementation plan and obtain its required review. Original 120/3/15 allowance includes preparation/evaluation; no reset. Only then implement, run the unchanged full core gates and fetched guide checks, obtain genuine separate qualified final-head review and current CI, merge under standing authority, verify identical main tree/exact main CI, and bind the coordinated G28 card/source pin. This design does not mark any G28 acceptance passed, call an unassigned reviewer independent, or grant runtime authority. Withdrawal, project privacy, job admission and physical resident-model capacity remain their accepted separate gates.
