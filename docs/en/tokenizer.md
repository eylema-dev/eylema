# G29 bounded owned tokenizer

English is canonical. [Türkçe](../tr/tokenizer.md). The reviewed [design](g29-tokenizer-design.md) fixes this profile's boundaries. Source PR, current CI and genuinely separate final-head review remain pending until recorded delivery; local acceptance is not merge approval.

## Behavior and identity

`eylema_model::tokenizer::Tokenizer` admits caller-authorized readers and an immutable G27 manifest. It verifies the complete tokenizer/config bytes before construction; no file/network opening, hidden engine, Python runtime or unchecked public constructor. Initial production support is the exact Qwen3-0.6B manifest4629bytes/SHA256 `c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2`, source `c1899de289a04d12100db370d81485cdf75e47ca`. Other manifests/templates/context/precision changes fail explicitly pending their own accepted profile. This text frontend does not promote `check_assignment` beyond `UnsupportedLocal`.

The full11422654byte tokenizer SHA256 is `aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4`; config9732bytes SHA256 `d5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101`; template4168bytes SHA256 `a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8`. Public source data retains publisher terms; no model weights are distributed. Only public tokenizer/Unicode notice data were read within the accepted20MiB allowance; cached identified config was reused.

Five owned Rust units supply bounded admission, Unicode16 NFC, the fixed ordered split, reversible byte-BPE and typed prompt/strict decoding. Heap-sorted fallible indexes avoid hidden allocation; non-cryptographic vocabulary lookup hints always check full string equality. They are not cryptographic identities. Identity hashing remains the accepted macOS CommonCrypto stream; authored crypto or third-party runtime crates are absent.

## Text and typed prompt subset

`encode_text`, `encode_prompt` and `decode` use explicit caller `TextBudget`; encoded IDs and source digests have readonly accessors. `PromptMessage` supports optional first System, followed by alternating User/Assistant and ending User, at most8 messages. It reproduces the pinned no-tools/non-thinking prefix; there is no arbitrary role/Jinja/tool/multimodal/reasoning-field engine. All26 added-token literal spellings reject in ordinary input before and after normalization. Only typed formatter code inserts structural IDs. Added IDs decode to literal data, never task/action authority.

Unicode16 unassigned scalars, malformed/incomplete UTF8 decode, unknown IDs, duplicate fields/IDs, absent byte/merge references, unsupported semantics, changed/truncated/extra artifacts and exhausted limits reject categorically. Debug exposes counts/digests or role/content byte count, not raw text, private metadata, I/O messages or paths. There is no persistence, collection, training or ecosystem/project export capability.

## Resource and platform limits

| Limit | Initial profile |
|---|---|
| Artifact input | Tokenizer16MiB/config64KiB; <=160000 vocabulary and160000 merge entries; decoded entry4096bytes |
| Requested resident content | <=128MiB including retained data, raw/parser/index/scratch/output and Unicode working views; fallible reservations checked before growth |
| Reads | Shared1024calls/16Interrupted retries including EOF; known size/profile/resident limits reject before input |
| Admission work | <=134217728units per artifact; logical node cap1000000 per immutable parse and depth16; repeated views still charge shared work |
| Text/prompt |8192UTF8bytes per text;8messages/32768aggregate content bytes; normalization total working scratch64KiB |
| Output/context |4096token hard cap; prompts reserve manifest generation128 from context4096 by checked subtraction |
| Text work/decode | <=2000000scan/normalization/merge units; strict decode<=4096IDs/65536output bytes |

These are requested-content/work limits, not measured RSS, throughput or model capacity. Arbitrary blocking readers need caller-owned deadlines. macOS native acceptance is distinct from selected existing portable-library Wasm compilation; another target's tokenizer execution is not accepted.

## Data provenance and actual evidence

The generated Unicode data is287516bytes/SHA256 `6797aff88e12c5a56e4e28a7be08b66dfa7e2bcd9d37e6c834b7f7c407100fe6`, derived reproducibly from existing reviewed CPython3.14/Unicode16 data. It carries [Unicode notice](../../crates/eylema-model/data/UNICODE-LICENSE.txt) and [provenance](../../crates/eylema-model/data/unicode16.provenance.json). Property/composition source identities are `3627a624f63370f0e726dde726f42934c8ca898c5e18fb69abdeee5a27ff00d8` and `e83834fdad296e46779fb702e11dd9b8686409a6346461d02fe3a22afaf26dd4`. Python is development-only; runtime normalization/classification uses authored Rust and immutable attributed data.

The independently frozen reference SHA256 `01c8a64e4142175690a15282b6913f53508e9b9e9d66cd0ac8c84c7b6c577ff5` precedes production code. Four stdlib-only anchors passed; published Hello-world IDs are [9707,1879]. Native actual full-artifact acceptance compares13 authored EN/TR/Unicode/space/CRLF/overlap examples and4 typed prompts, exact token IDs and strict decoded text. This is tokenizer evidence, not Turkish model quality or neural inference. Ordinary CI honestly ignores the two private-public-asset acceptance tests; they require explicit local artifact inputs. Small synthetic data exercises the same admitted algorithms through test-only fixed expectations absent from production.

Three bounded repair passes were charged: reduce repeated vocabulary work without raising its cap/preserve budget-error category; preserve CRLF bytes in frozen-reference-to-Rust test transport without changing expected IDs; reject spaced surrogate escapes and enforce aggregate normalization scratch before growth. Full checks and compiled source-mutant receipts must accompany final delivery. No weights, reference engine, model/provider/training call, GPU or performance acceptance is claimed.

Final local whole-workspace verification:350native tests passed/3explicit ignores (existing Metal plus two private-public-asset cases);47Python passed; full fmt/clippy/build/dependency/selected existing Wasm compilation passed. Both explicit local asset cases passed; four compiled source mutants failed and restored originals passed. Ignored CI cases are not inferred as executed. Separate final-head review/latestCI/coordinated main delivery remain pending.

Known input-byte work and shared read-call lower bounds (including both EOF probes) reject before raw reservation or either reader invocation. Passing these necessary floors does not guarantee that parsing work, short reads or retries will fit; incremental checks remain mandatory.

R5 extends the independently frozen reference to13text/14prompt cases; the original13/four records remain unchanged. Typed headers and content form one contiguous ordinary BPE span between structural tokens, including empty/leading LF/CRLF/whitespace and Unicode cases. Content remains separately guarded and NFC-bounded; a fixed LF header boundary cannot compose with content. Temporary combined bytes and copying work count against the same resident/work budgets. Exact output-token limits are tested for each new prompt.
