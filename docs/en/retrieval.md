# Project-private owned retrieval

[Turkish companion](../tr/retrieval.md). G70 implements a bounded first-party Rust retrieval library and native authority adapter. It does not run an embedding model or expose a new HTTP/UI service. Vectors in this acceptance are public synthetic inputs; G68 model execution remains separate.

`eylema-retrieval` owns UTF8 windows, versioned representation identity, unit-vector dot scores, lexical set Jaccard, deterministic hybrid scores and complete-snapshot persistence. `eylema-host::retrieval` reads only current authorized ProjectStore content; source references and synchronized put digests are preserved through chunk/range/text-hash results. Raw library values are not capabilities. No third-party crate or database engine was added.

## Supported bounds

| Admission | Ceiling |
|---|---|
| Sources / source bytes | 16 /8192 UTF8 bytes each |
| Chunks / chunk bytes | 128 /1024, scalar boundary and exact complete coverage |
| Vector dimensions | 1..4096, explicit unit norm tolerance1e-5 |
| Query bytes / unique terms | 2048 /32, normalized term64bytes |
| Chunk unique terms | 128 |
| Requested cutoff / returned hits | 16 /32, complete cutoff tie group |
| Serialized result | 65536bytes including provenance |
| Snapshot / records / journal | 1MiB /4 /5MiB |
| Operation | 100M declared work units,64MiB logical owned content,30s cooperative deadline |

Joint bounds may reject a maximum combination. Unknown token limits return `TokenCounterUnavailable`; bytes are never reported as tokens. Scalar dot and norms use fixed-order F64 over stored F32 values. Scores are millionths with ties-to-even; lexical scores are exact rational set Jaccard. Hybrid weights are explicit1..999999 millionths. Source ID ordering only presents equal scores; it never creates a unique winner. A tie exceeding max_hits fails atomically.

Unicode semantics are versioned to Rust1.99: original maximal alphanumeric runs are lowercased without NFC, locale or stemming claims. Model/artifact/tokenizer/preprocessing fingerprints and semantic space/profile versions must match exactly. Changed bindings require a new explicit index, never relabeling or model fallback.

## Current authority and retention

Create/replace needs Collect for every source and ancestor; search needs Execute for every explicit requested source before index status is exposed. Search stages the whole result then checks every source/session at one common persisted trusted time. A different or reopened ProjectStore invalidates retained handles. Explicit reopen requires the current owner/fresh Execute session, matching put digests and exact current source bytes.

Replace writes a complete next generation. Source replacement uses a new immutable DataRef; omission deletes it from current results. A failed synchronization or failed post-write authority/control check produces no acknowledged generation and requires explicit reopen. Separate data/index journals are not one transaction. History remains on disk: no physical erasure, encryption, compaction or hostile same-user isolation is claimed. The adapter provides no training/export permission or content-sharing API.

A strict map-free EYL preflight limits nesting, nodes, container lengths and blob/text extents before the generic journal decoder allocates. Bad checksums, malformed semantic snapshots, unknown versions, torn tails and nonmonotonic generations fail without older-generation fallback. Logical accounting includes retained/replay/staging/encoded copies; allocator overhead/slack, caller-held G63 data, OS buffers and RSS are distinct. Native filesystem calls remain potentially blocking;30s is cooperative, not hard real time.

## Evidence status

At numerical source58cec7c70bf8d536506bdcc90419adc518c6a27a, the actual macOS native-file test passed creation, synchronization, cooperative locks, close/reopen with fresh authority, all three scoring modes, source replacement/deletion and empty-index reopen on synthetic1024D vectors. Full ranks were `{a,d},b,c`; literal fixed relevance sets have recall@2 and recall@3=1.0. This is deterministic fixture behavior, not model or EN/TR relevance quality. Native search counters were4285333/4255047/4291121 declared units and83901/86721/86877 logical bytes for vector/lexical/hybrid; these are a single test observation, not performance benchmarks.

The20retrieval tests,9host retrieval tests and actual native-file test passed, including128-tie rejection, normalized shrinking/expansion, 14checksummed schema/version/project/binding/duplicate/order/range/term/vector/hash/generation variants, torn history, IO uncertainty, denial-before-status, common-time expiry, malformed-vector preflight and cancellation/deadline after synchronization. Native test target repeats the9shared host tests; these repetitions are not additional scenarios. The private raw-dot test compares24literal scalar cases at2/1024/4096dimensions with error<=2e-12; micro/norm expectations stay unchanged. Full local checks, current CI and separate final-head review are independent delivery gates; this note alone does not accept G70. Source/session expiry and IO faults are controlled injections, not physical power loss or real-person authentication. No weight download, model/provider/training execution, GPU, SQL/graph, ANN, throughput or general retrieval quality was tested.

Run targeted checks with Rust1.99.0:

```sh
cargo +1.99.0 test -p eylema-retrieval --locked --offline
cargo +1.99.0 test -p eylema-host --locked --offline --test retrieval --test retrieval_native
```

Canonical full local Python/repository/real-PR/diff gates and `tools/check_rust.py --provision`, exact-head CI, genuinely separate owner-qualified review, authorized merge and post-merge main CI are independent delivery gates. The actual owner admitted configuredgpt-6.1-sol/high only for G70's bounded scalar/index/authority scope; hidden serving identity remains unknown. Preparation, repairs and review consume the original120active minutes/3repairs/900review seconds, plus two actual owner-approved targeted repairs (total5); the last changes only replay-test coverage, with identical production bytes and no extra active/review/call allowance.
