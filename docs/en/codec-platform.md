# G09 — Canonical codec and platform services

[Türkçe](../tr/codec-platform.md). Runtime contract **0.1.0**, EYL codec **1**, unpublished Rust packages **0.1.0**. [Task plan](../../tools/work-plans/g09.json), [typed contracts](typed-contracts.md), primary issue [eylema#11](https://github.com/eyleme-dev/eyleme/issues/11).

## Wire format and bounded parsing

The owned, safe Rust [codec](../../crates/eylema-contracts/src/codec.rs) starts with the exact seven bytes `45 59 4c 01 00 01 00`: ASCII EYL, codec version 1, runtime version 0.1.0. Versions must match exactly. Values use one-byte tags; lengths/counts are unsigned big-endian u32; signed/unsigned numbers are distinct fixed eight-byte big-endian values.

| Tag | Value |
|---|---|
| 0 / 1 / 2 | Null / false / true |
| 3 / 4 | Signed i64 / unsigned u64 |
| 5 / 6 | Length-prefixed opaque bytes / UTF-8 text |
| 7 | Count-prefixed ordered list |
| 8 | Count-prefixed map: length-prefixed UTF-8 key then value |

Map keys are nonempty, control-free, at most 96 bytes, unique, and sorted by UTF-8 bytes. Encoding sorts keys; decoding rejects unsorted/duplicate keys. Lists preserve order. Text bytes remain exact: composed and decomposed Unicode strings differ; no normalization or float conversion is implicit. Floats have no supported tag.

Bounds: 4 MiB total wire input/output, 1 MiB per byte value, 4096 bytes per text value, depth 32 (root depth 0), 65536 total value nodes including the root. A container's own node counts toward that total. Counts, length arithmetic, minimum remaining wire bytes and remaining node budget are checked before decoding allocations. Unknown tags/versions, invalid UTF-8, trailing/truncated input, excess depth/nodes/aggregate bytes and invalid keys fail. These additional wire limits can reject a composition whose individual G08 records fit their local limits; they are protocol limits, not measured capacity. Caller-owned Rust Value construction is not itself an untrusted-input admission path.

The fixed reference map `{"b": null, "a": unsigned(42)}` encodes as:

```text
45594c010001000800000002000000016104000000000000002a000000016200
```

## Typed records and approval payload

The [typed adapters](../../crates/eylema-contracts/src/codec/typed.rs) cover every G08 task/action/proposal/execution/effect/event/error, version, ID, resource, authority, oversight, budget and target field. WireRecord kinds are task, proposal, execution, event and error. Proposal records carry the task snapshot; execution records also carry the proposal, so decoding re-runs the existing constructors and their internal resource/revision/policy/target/budget bindings. All known fields are required (optional values use explicit null); unknown or missing fields, unsupported enums/versions and narrowing integer overflow fail. Resource, permission, checkpoint, capability and evidence sets are canonically sorted; decoder rejects unsorted sets. Action/effect list order remains significant. Canonical round-trip bytes are stable even when an unordered source Vec was in a different order.

`canonical_approval_payload(task, proposal)` revalidates the proposal against its supplied task, uses kind `approval_payload`, and includes the entire typed task/proposal/action effect declaration. Only transport RequestContext fields (request/correlation IDs and their redundant context version) are excluded. The wire header still binds runtime 0.1.0 and operation versions remain included. Task/proposal/action identities, objective, revisions, operation, opaque payload, scope, authority/policy/oversight/capabilities, budgets, target, idempotency and action order remain bound. Opaque payload bytes are never parsed or rewritten. Changing transport IDs preserves the digest; changing effect payload or action order changes it. Hashing is separate from codec construction. A matching hash is neither authenticated authority nor an accepted approval; G11 supplies actual policy/approval behavior.

Development fixtures in the [native consumer](../../crates/eylema-platform/examples/codec-proof.rs) have exact SHA-256 references: task `e2806de4884f8cf91c7e859ed741607545e2062674658d3b45fb69a4480d5f4d`, approval payload `ff2fe50c966a3f3dace2bc241bf604c42b6c84d75142f0a640e0b516724d20c7`. These are synthetic fixtures, not real task execution evidence.

## Explicit platform boundaries

The first-party [platform library](../../crates/eylema-platform/src/lib.rs) depends only on the owned contracts package. It uses macOS CommonCrypto `CC_SHA256` through libSystem (CC_LONG is SDK u32) and Security framework `SecRandomCopyBytes` with null/kSecRandomDefault. The actual SDK declarations were inspected. Each narrow unsafe extern call records slice/array lengths, pointer lifetime and return checks; the contracts library retains forbid(unsafe_code). Native SHA input is bounded to 4 MiB; a service error or unsupported native target returns an explicit error. There is no owned SHA algorithm, time/counter entropy fallback, copied SDK implementation or third-party Cargo dependency/build script.

IDs use 16 bytes of OS entropy, known entity prefix plus 32 lowercase hex characters. Deterministic formatting is owned Rust; entropy generation is the platform service. Samples demonstrate service execution, not absolute uniqueness or identity authentication. Non-macOS native service execution is unsupported here.

The [Wasm ABI](../../crates/eylema-platform/src/wasm.rs) exposes memory plus eyl_allocate_input, eyl_process, eyl_output_ptr and eyl_output_len. Modes 0/1/2/3 are generic canonical round-trip, typed record round-trip, approval payload and formatting kind-byte + 16 entropy bytes. Status 0 means success, 1 invalid owned buffer length/lock, 2 invalid codec/kind/mode. Allocation accepts 1..4 MiB and clears input/output state; invalid allocation/process clears the output. The host may write only its allocated input range, processes synchronously on one instance and copies output before any next mutating call or await. Outputs live until the next mutation. This is a trusted linear-memory host boundary, not a sandbox against a hostile host that can write arbitrary Wasm memory.

The minimal [host binding](../../platform/browser/codec-host.mjs) copies input before allocation (including aliased Wasm views), checks ranges/status/output size, copies output immediately and delegates digest/entropy to standard WebCrypto. An absent/failing provider throws; no silent fallback is supplied. JavaScript does not implement codec, domain validation, ID formatting or cryptography. Browser secure-context/API availability is a host precondition; actual browser acceptance is not supplied by Node testing.

Primary API references: [Apple CommonCrypto](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html), [Apple randomization](https://developer.apple.com/documentation/security/randomization-services), [WebCrypto Recommendation](https://www.w3.org/TR/2017/REC-WebCryptoAPI-20170126/). OS SDK/browser engines remain dependencies.

## Executed evidence and reproduction

Local macOS 27.0.1 ARM64 / Apple M4 Pro 24 GiB, Rust 1.99.0, Python 3.14.8, Node v26.11.1. Node is development-only and separately inventoried in devguide; no npm dependency is installed. Inherited RUST_LOG is explicitly removed for the unchanged environment guard.

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG node tools/check_codec_platform.mjs
```

Passed: unchanged Rust fmt/clippy, owned offline/locked workspace inventory, native tests/build, selected contracts/platform Wasm compilation. Eight codec regression tests exercise all typed families/error codes, truncation at every offset, keys/UTF-8/version/scalars/limits, unknown/missing/narrowed fields, declared binding drift, canonical sets and transport/effect/order distinctions. Four platform tests include actual CommonCrypto empty/abc vectors and actual Security IDs; injected failure tests only check propagation and are not platform execution proof.

The opt-in [comparison harness](../../tools/check_codec_platform.mjs) first runs the unchanged Rust guard/gate, then actually instantiates the Rust Wasm module in Node v26.11.1 and compares 33 accepted and 30 rejected native fixtures. Accepted output bytes and actual Node WebCrypto SHA values equal actual macOS CommonCrypto values; fixed reference bytes/digests, limits, 500 repeated calls, input aliasing, copied-output lifetime, invalid ABI lengths/modes and unavailable/failing crypto paths pass. G09 Node proof is local, separate from hosted CI, whose unchanged Rust gate proves native execution and selected Wasm compilation only. Browser execution, task executor/admission, real approval, service/journal, GPU/model/training and performance acceptance: not_run, assigned to later Goals.

## G28 separate artifact hash stream

G28 adds the owned Sha256Stream interface to the same inventoried CommonCrypto provider. It preserves every existing wire/browser hash bound and ABI. The separate stream owns its initialized context, caps total2GiB/chunk1MiB, poisons failures and consumes finalization. Installed SDK/C/Rust layout and independent native vectors passed; this does not prove actual model/large-artifact or new browser/Wasm execution. See [weight-byte loading](weight-loader.md) for API, memory and trust boundaries.
