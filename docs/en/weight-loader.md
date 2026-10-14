# G28 owned weight-byte loading

[Türkçe](../tr/weight-loader.md). Own safe Rust `eylema-model` loads a bounded declared Safetensors subset into immutable owned bytes. G28's native tests are byte/layout/service evidence, not model execution. [Design](g28-weight-loader-design.md) and [plan](https://github.com/eylema-dev/devguide/blob/codex/g28-weight-loader-plan/docs/plans/g28-weight-loader.md) bind the accepted scope; PR/current CI/separate review/main proof remains required for delivery.

## Caller contract

`TensorSpec::new` validates flat ASCII names and positive rank1..4 shapes. `WeightLayout::new` binds the exact canonical G27 manifest digest, configured weights component and sorted unique expected tensor map. It permits uniform stored BF16/F32 only, manifest `safetensors.v1`, `bfloat16`/`float32`, `none` quantization. The expected map's publisher/architecture provenance is caller input, not independently established model completeness or rights.

`load` accepts a caller-owned authorized Read implementation and explicit LoadBudget. It checks exact declared component size, artifact/resident/read-call limits and immutable manifest/layout equality before consuming input. It reads/validates the bounded header before whole-artifact reservation, hashes the complete prefix/header/payload in file order, requires terminal EOF and expected SHA256, then returns LoadedWeights. Failure returns no partial snapshot. Existing assignment/schema/task/journal contracts are unchanged; no model becomes executable.

Accessors expose immutable borrowed tensor slices/specs and actual artifact/manifest/layout digests. Returned storage owns its lifetime: overwriting/removing the input file afterwards cannot change it. Neither this library nor a hash match grants file/project/execute/collect/train/disclosure/export authority. No path/network opening, external model fallback, tokenizer, conversion or inference occurs.

## Explicit limits and unsupported cases

Whole artifact <=2147483648bytes, header<=1048576,1..1024tensors,rank1..4,positive checked dimensions. Names<=128ASCIIbytes, decoded-key equality and uniqueness; metadata<=128string pairs,keys<=128decodedbytes,values<=4096; depth<=6(root0),16384value nodes. Unknown/duplicate/missing tensor fields, bad UTF8/escapes/surrogates/numbers, mismatched dtype/shape, out-of-range/overlapping/gapped/unindexed payload fail. Optional metadata is validated then discarded; it is never instructions or ordinary debug output.

Reader calls<=65568 including EOF probe; interruptions<=16; body chunks<=65536bytes. Caller resident content cap covers exact artifact size plus4194304bytes bounded header/parser/layout/scratch allowance, before artifact allocation. Growable owned content storage reserves fallibly; OOM/allocator/OS bookkeeping and caller/other-job storage are not observed physical RAM. Huge declared-size rejection fixtures do not allocate a huge buffer. The2GiB ceiling is a logical target, not measured2GiB capacity or a fit recommendation.

Scalar/empty/packed/quantized/sharded formats reject. BF16/F32 bytes remain opaque: the file format permits nonfinite stored values, and no conversion/finite-value/operator conformance is supplied. Later model/profile gates must establish full tensor architecture, tokenizer and numeric support. Generic synchronous Read cannot guarantee time cancellation; caller must supply a bounded-blocking/deadline reader. Snapshot lifetime is process-local; durable storage and guaranteed erasure are not supplied.

## Native service boundary

`eylema_platform::sha256_stream::Sha256Stream` owns a private macOS CommonCrypto context; configured total1..2147483648bytes and chunks<=1048576. Invalid/excessive updates poison; subsequent update/finish fail, including empty updates. Finish consumes state. No cloning, cryptographic algorithm or hidden engine. Existing one-shot SHA retains its4MiB wire limit. Unsupported native/Wasm service returnsUnavailable; selected existing portable library compilation is not execution of this loader.

Installed SDK ABI was independently checked with a temporary C syntax/layout assertion and Rust layout checks: u32CC_LONG, context104bytes/align4, count/hash/wbuf offsets0/8/40. Synchronous Init/Update/Final require status1 and live checked input/output/context lifetimes. [Apple API](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html) and SDK remain dependencies. Controlled failed-status unit tests prove safe handling; the actual OS service was not forced to fail.

## Actual bounded evidence

Seven new stream tests (five integration/two unit) exercise empty/abc/million-a independent vectors, chunk boundaries,5MiB repeated bytes versus frozen Python hashlib, unchanged old wire rejection, cumulative/excess limits, poison/status and checked layout. Thirteen native model tests exercise independently frozen F32/BF16 artifact slices and canonical manifest/layout digests, malformed bounded headers, exact whole-file comparison, partial/interrupted/error/extra input, pre-read budgets and real small synthetic-file overwrite/removal. Four disposable compiled source mutants test ignored digest, omitted prefix hashing, wrong equal-byte shape and accepted overlap; their behavioral failure and restored-green evidence are kept privately.

Only own127/134byte synthetic artifacts and bounded synthetic hash data were used; no real weight/model/provider/training call. Tests do not establish real Qwen loading, architecture correctness,2GiB memory/throughput, GPU/model/training performance or full Wasm/other-device execution. Source/main CI and final genuine separate review are separate required evidence, not inferred from these notes. Existing G40 failure/budgets stay unchanged.

Separate review found declared1..7byte artifacts could consume the8byte prefix before rejection. A panic-on-read BF16/F32 regression failed first; the loader now rejects these sizes before reading. Repaired-head checks and separate review are required for delivery.
