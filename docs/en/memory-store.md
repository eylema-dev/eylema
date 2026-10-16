# Durable attributed memory

[Türkçe](../tr/memory-store.md). G88 adds `eylema_contracts::memory::durable` and `eylema_host::memory_store`. The existing G87 exact-reference registry remains unchanged. This is an owned Rust store and knowledge-graph/retrieval composition, not a selected CortexDB, SQLite or DuckDB engine. Backend interchange remains a separate Goal.

## Writing and reviewing

Create/open a MemoryStore against the actual live ProjectStore and its owner. A sealed MemoryOwner enrolls a MemorySession retaining an existing G63 DataSession; enrollment does not acknowledge source permission. Every use checks the original live identities and current G63 source/purpose/ancestry rights. Equal-looking stores and old cached handles cannot substitute.

`propose` accepts a closed RecordDraft, exact generation/content revision, bounded optional vector and a new projection DataRef. The source session must have Collect permission for all influences and the new projection. Records represent entities, attributed facts, entity relations and episodes. A proposal is Proposed, never reusable knowledge merely because a caller claims confidence. `review` additionally requires the sealed memory owner, actual data owner and pinned review source. It produces a new explicitly reviewed revision/projection and preserves the original assertion origin.

The projection is immutable Derived UTF8 text with every source/review/control in its ancestry. G63 creates it with NONE permissions. The data owner must separately apply the existing explicit source policy before Execute recall; the memory facade never grants a policy. All existing parent/depth/expiry checks still apply. Projection synchronization precedes the memory event. A crash between them can leave an orphan projection that memory recall cannot discover. A receipt identifies a synchronized generation/revision/journal digest after final current-authority checks.

Known work and append capacity must fit before projection/event writes. The live write prepays its later state-validation scan and retains the already performed vector admission; replay independently validates vectors/state. Write/flush/sync or postcommit receipt uncertainty poisons the writer and returns WriteOutcomeUnknown/NeedsReopen. Reopen inspects committed state without an automatic retry or rollback claim. A tombstone excludes current memory while retaining append-only history/backups; it is not physical erasure.

`observe` accepts the actual ToolHost, scoped tool session and execution ID. Its crate-private bridge obtains the live authorized report and complete controls from every retained execution influencing the cumulative task. A caller-built ToolReport cannot impersonate that witness. The facade separately checks Collect on every influence, rejects over-limit provenance and derives actual status/value. Failed, partial, stopped, pending and unknown states never become completed success.

## Knowledge, graph and RAG

Origin is Observation/DocumentClaim/Estimate/Prediction; admission is Proposed/ExplicitlyReviewed/VerifiedOutcome. Default reusable knowledge admits explicitly reviewed document claims and genuine verified observations. Reviewing a prediction or increasing confidence does not turn it into an observation. Explicit Evidence queries retain the original labels. New attributed revisions preserve prior journal history.

Queries bind the exact project/profile/revision/generation, role, local purpose, validity time, kind/origin/knowledge filters, entity/predicate filters, graph seeds/hops, representation and output bounds. Role is a selector, not caller identity. Structural graph edges use their own edge-predicate filter; result predicates do not erase the authorized path required to reach facts. Entity subjects and entity-valued objects must have current authority. Graph traversal and all source/lineage/review/time filters precede ranking. Unauthorized and absent explicit seeds share a public authority error.

Graph-only order is minimum hop then stable record ID. Indexed recall builds a temporary G70 Snapshot from actual authorized G63 projection bytes/digests and the exact representation binding. Lexical/vector/hybrid score and tie rules remain G70's. Required path links remain attributed provenance. More than sixteen eligible indexed projections is an explicit limit; candidates are not silently omitted. Optional synthetic/caller vectors are not proof of model inference. A verified-token request without an accepted counter is unavailable.

SingleValue predicates preserve conflicting overlapping values as attributed Conflict; SetValue predicates permit multiple values. Historical effective time never restores current withdrawn/expired sources or records. The store does not implement general natural-language contradiction detection. A conflict group that cannot fit the hit/byte budget returns an incomplete-conflict/limit error with no settled single answer. NoEvidence, Conflict, Forbidden, backend failure, cancellation and deadline remain distinct.

A cached query stores references/generation/live-store identity, never content or a permission receipt. Every reuse rebuilds and reauthorizes. Before returning bytes, every used source/projection/entity/path and TTL is rechecked at one common persisted trusted-time observation. Withdrawal, deletion and expiry therefore affect cached and reopened views. Already returned bytes cannot be recalled.

## Identities and limits

A raw-payload SHA256 identifies bytes; a G63 put digest identifies the actual chained immutable write receipt. They are different. Current G63 metadata revisions also reflect policy changes: the original put revision is not automatically the current authorization revision. Returned citations retain current authorization revisions and actual put pins.

| Profile bound | Limit |
|---|---|
| Current records / entities | 128 total / 64 entities; relations share the total |
| Memory journal | 512 frames including header; 8192 payload bytes/frame; 2097152 total bytes |
| Identifiers / literal / projection | Existing 96-byte identifier gate remains stricter than the 128-byte facade ceiling; literal256 / actual UTF8 projection1024 bytes |
| Direct influences / graph | 8 influences; 8 seeds; 4 hops; 128 visited records |
| Indexed view / first representation | 16 projections, one chunk each / 1024 dimensions |
| Returned hits/sources / bytes | 32 / 65536, including provenance/conflicts |
| Shared operation | 100000000 work units / 64MiB logical live reservation / 5 seconds cooperative deadline |
| Retained / replay escrow | 8MiB retained; replay additionally reserves48MiB and prepays51380224 work |
| Reopen physical IO | Two scans; aggregate4MiB reads,4096read calls,16seeks,16metadata calls; no writes/sync |
| Append physical IO | 8276write bytes,1024write calls,4seeks,4metadata calls,one flush/sync; creation permits two flush/sync pairs |

The closed memory payload guard runs before general EYL decoding: at most256nodes, depth8, lists32, no maps. Digest-valid malformed extents/schemas still reject. All shapes/counts and checked sums precede costly work. G63 and ToolHost calls use a conservative prepaid facade allowance; their own physical IO is not claimed instrumented by the memory-journal tally. The actual G70 child uses the same Control without a reset. Caps are independent: individually valid maxima need not fit together. G74's separate ten-million-unit route ceiling is unchanged; no memory route is automatically enabled.

Counters describe logical admission, not RSS, throughput or measured hardware capacity. Caller-held stores/tools, allocator overhead and OS buffers are excluded. Native calls are checked before/after and are not hard real time. Journal integrity is not consent: opening the owned journal is a privileged trusted-host replay boundary, not approval of an untrusted imported journal. Private source text is untrusted evidence; the schema does not automatically certify arbitrary text free of secrets.

## Acceptance evidence and boundaries

The frozen synthetic corpus contains EN/TR ownership, two-hop site/equipment/checklist evidence, attributed conflicts, historical/current validity and prediction-only NoEvidence. Eight document-brief jobs cover36mode/time cases with the same deterministic renderer/verifier in both conditions. The ordinary local fixture matched36/36 memory briefs and0/36 flat-RAG briefs, with zero unauthorized citations in both. This is a measured outcome for those fixed evidence tasks, not generated-language or embedding-quality evidence or a general superiority claim.

Both conditions use the same corpus/representation and100M/64MiB/5-second ceilings. Flat RAG retains G70's existing read accounting; memory adds its approved conservative source-composition allowance. Work-counter differences therefore are not speed measurements. No expected baseline loss is required by the acceptance gate.

The mandatory native acceptance is separate: `cargo +1.99.0 test -p eylema-host --locked --offline --test memory_store_native native_restart_and_eight_briefs -- --ignored --exact --nocapture`. Its parent admits only one writer and one fresh reader, each at most10seconds/4096diagnostic bytes. Default CI ignores the parent/helper phases and cannot claim their success. The dedicated result must prove0700/0600 files, real full-sync/lock refusal, fresh capabilities, all eight jobs and cached/reopened withdrawal/delete/TTL exclusion. It does not prove power-loss recovery, Linux target execution or physical erasure.

The dedicated native acceptance passed on implementation `0bb4155dd4bd803dbdc6a23b4b1576e75984e30c` on16October2026: exactly one parent, one writer (1.30seconds) and one fresh reader (0.11seconds), parent1.43seconds. Both children exited0 within10seconds/4096bytes. Real full-sync, exclusive-lock refusal,0700/0600 ownership, fresh capabilities and cached/reopened withdrawal/delete/TTL exclusion passed. Memory matched36/36 fixed briefs, flat RAG0/36, both zero unauthorized citations. Peak logical reservation was9739856bytes; this is not RSS or a speed comparison.

The required local gates passed510Rust test executions and48Python tests, fmt, warnings-deniedClippy, native build and selected portable Wasm compilation;12ordinary tests were ignored (nine inherited plus the three explicit G88 native phases). The native parent/helpers above were separately executed successfully rather than inferred from those ignored results. Exact implementation [CI38940961458](https://github.com/eyleme-dev/eyleme/actions/runs/38940961458) passed. Genuine separate configuredgpt-6.1-sol/high review closed both graph-path conflict and replay cancellation/deadline findings;25focused memory tests independently passed. SingleValue path conflicts now include traversed links before final conflict admission, and read-only replay preserves control causes while uncertain writes remain uncertain. The maintainer approved three extra targeted passes(total6) without increasing original active/review/native limits. Final evidence-head review/current CI, source-main correlation and the separate guide delivery remain mandatory before coordinated acceptance. Configured contribution model is gpt-6.1-sol/high under the owner-approved G88-only structural profile; hidden serving identity is not exposed. No model/provider/training call, artifact acquisition, third-party engine, permission/account change or spending is included. [Approved design](g88-memory-design.md) and the G88 implementation plan supply the independent acceptance and delivery gates.
