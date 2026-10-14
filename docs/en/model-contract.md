# Pinned model contract — G27

G27 adds actual owned Rust validation of declared model provenance and role interfaces. It does not load weights, run a tokenizer/template/model, verify absent large artifact bytes, train, grant authority or invoke a provider. [Model catalog](https://github.com/eylema-dev/eylema-models/blob/codex/g27-model-contract/catalog/qwen3-0.6b.v1.json) and the matching native fixture bind the selected Qwen candidate; candidate registration is separate from execution acceptance.

## Closed metadata and support

`ModelManifest` is immutable after validation. `model-profile.v1` uses the existing EYL codec and unchanged task/wire/journal contracts. Maximum encoded metadata is32768bytes,1–64components,1–64operators and1–128intended role interfaces. Flat ASCII names, nonzero20byte source commit/32byte hashes, positive sizes with checked total, exact source/rights/component references and finite dimensions/workload bounds are checked. The owning publisher/source/license component is pinned; unresolved or foreign terms fail. A rights declaration is neither project access nor a legal determination of every downstream use.

The strict decoder rejects unknown authority/support fields, wrong types, noncanonical/trailing bytes, dangling/duplicate references, zero identities and overbounds. Hash origins remain declared `publisher_advertised` or `audit_measured_small_bytes`; transport can never turn either into a verified artifact capability. The concrete catalog audit records which small bytes were measured. An untrusted caller may lie in metadata; these constructors confer no trusted evidence, identity, permissions or executable support.

`check_assignment` matches the exact G46 role interface: model identity is separate from role, schema/units/preprocessing/action/reset must agree, and intended profiles cannot be overwritten by duplicates. CPU/Metal assignments return `UnsupportedLocal`, with a bounded reason, including `ModelExecutionNotImplemented`. `ExternalConfigured` fails in LocalOnly; ExplicitExternal returns only `ExternalConfigurationRequired`, never an external call, fallback or authorization. No Accepted/Executable constructor or model engine exists in G27.

## Baseline and limits

Qwen/Qwen3-0.6B sourcec1899de289a04d12100db370d81485cdf75e47ca is the retained candidate. Config/license/tokenizer_config bytes and the4168byte UTF8 chat-template slice were measured; model.safetensors and tokenizer.json identities are publisher-advertised LFS SHA256 only. Neither large file was downloaded or byte-verified. Third-party rights remain independent of Eylema licensing. Exact hashes/sizes and audited source/reference pins are in the catalog; no third-party reference implementation or template is copied into production or executed.

Declared Qwen profile: bfloat16 safetensors/noquantization,28layers, hidden1024/intermediate3072,16Qheads/8KVheads/head128, vocabulary151936 and upstream capacity40960. Q projection width2048 legitimately differs from hidden1024. Checked grouped-head/projection/workload arithmetic rejects incompatible heads/overflow. Narrow planned batch1/context4096/max128newtokens is not a measured capacity, latency or quality claim. The authored pinned operator contract specifies eager causal attention, FP32 RMS/softmax/position arithmetic, default RoPE, Q/K head normalization, tied logits, KV reset and greedy bounded generation. Physical loading/precision/tokenization/numerical conformance are later G23/G28/G29/G30 gates.

Nine optional role records distinguish perception, reasoning, planning, decision, policy, world_model, specialist, critic and representation. STT/vision, TTS/text-domain and embedding/reranking are explicit domain subdivisions. The Qwen catalog declares only reasoning/planning/specialist intent; all roles have null default assignment and no accepted model quality. Loops need only their chosen roles, not all nine.

## Actual evidence and boundaries

Eleven native tests include actual catalog fixture decoding, exact source/weight metadata and canonical roundtrip, role mismatch, strict fields/bounds/rights and local-to-external rejection. Core and model catalog EYL bytes have identical SHA256c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2. Eight development-only Python tests exercise offline catalog/role/operator/hash/byte consistency and malformed input; Python is not a production engine. Full native gate, local Python gates, current CI, separate review and accepted-main proofs are recorded in the linked G27 card/PRs. Static matching and constructor execution do not prove inference, quality, GPU, training, model byte verification or permissions. Shared canonical JSON schemas stay in core; no validator/source-role/dependency exception is added.

Trust boundaries: reviewed audit inputs/source retrieval and clocks, own typed contract and declared profile semantics. Later loading must verify actual bytes before use and independently establish every required operator/backend/tokenizer. Unknown/unsupported profiles fail without a fallback. Withdrawal/collect/train/disclosure/export permissions remain in their separate project-data gates.

Primary sources: [pinned publisher files](https://huggingface.co/Qwen/Qwen3-0.6B/tree/c1899de289a04d12100db370d81485cdf75e47ca), [audited reference commit](https://github.com/huggingface/transformers/tree/0720e206c6ba28887e4d60ef60a6a089f6c1cc76). The reference was read/hash-audited only; Transformers/PyTorch was not installed or executed.

Source precision BF16 is separate from the planned FP32 CPU reference and FP16 Metal execution/conversion profiles. Those conversions and numerical acceptance remain later gates; this catalog does not replace the original target obligations.
