# G72 private local report job design

Status: written specification for owner review. No G72 product code, artifact access or model execution has started. [Türkçe](../tr/g72-local-report-design.md). [Issue79](https://github.com/eyleme-dev/eyleme/issues/79), proposed G72/r4.

## 1. Outcome, evidence and scope

Complete an actual project-private bilingual source-bound report through owned Rust embeddings, planning and decision, typed operations, factual/citation verification, one bounded pure-step repair, effective human stop and durable reopen. Models must affect the admitted evidence/selection path; neither a counter demonstration nor a renderer that ignores model outputs is acceptance. Report facts remain attributed document claims, not calibrated model truth.

Source base is `cbad05aa256541858c595ba7f33fc1068709fb55`; guide base is `0870a4dbe437fad84034ce16b5b62cd86cc46a45`. All nine unchanged dependencies G30/G57/G68/G70/G54/G50/G63/G73/G88 have accepted receipts. Their proofs remain separate: G30 has short unverified planning proposals, G57 has uncalibrated relevance choices, G68 has numerical embeddings, G70/G88 have bounded retrieval/memory, G54 has two counter tools and G73 has explicit device-owner purposes. None already proves this report job.

The owner approved application-owned composition and preparation of this specification/qualification assessment, including proposed first-party Cargo.lock and G72 guide scope. That does not yet approve this written design, its additional report-purpose path below, the implementation plan or any model/artifact trial. Original120active minutes/3targeted repairs/900review seconds includes preparation; no reset. Existing inline execution and conditional merge method persist.

## 2. Architecture and responsibility

An application-owned `report` module in the native CLI composes the existing libraries. A fixed same-binary worker executes models; it has no project owner/session or effect authority. The controller owns ProjectStore, MemoryStore, ToolHost, the actual human controller, verified report permits and state transitions. No listener, browser, external provider, training engine, new crate or third-party dependency is added.

| Component | Responsibility and boundary |
|---|---|
| ReportNeed | Closed project/job/generation, two required EN/TR site/field sections plus one optional NoEvidence probe, authorized source pins, assigned model/profile identities, source-review acknowledgement, output ID and budgets; no arbitrary tool/process/provider fields |
| Source tools | Read current authorized structured documents; preserve source digests/revisions and local Execute/Collect ancestry; no automatic permission or review promotion |
| Model worker | Actual G68 embeddings, G30 Planning proposal and G57 decision; fixed typed protocol, exact artifact identities, no tool execution |
| Report tools | Closed application Rust operations PrepareReport/VerifyReport/CommitVerifiedReport/ReopenReport; runtime requests cannot install callbacks or invent handlers |
| G54 checkpoint | Actual source-bound AddTaskValue/ReadTaskValue admission, human approval/intervention and observed evidence; still only the existing two handlers |
| ReportPermit | Non-serializable, non-cloneable application handle binding verified bytes/digest, Need/generation/source pins, actual completed checkpoint and human acknowledgement; never constructed from model or CLI JSON |

The report operations are new application consumers of G63/G88 and the existing oversight APIs, **not new operations in G54's ToolRegistry**. The checkpoint value records verified fact count, not the report's factual answer. A completed checkpoint alone cannot produce ReportPermit or JobCompleted. No contracts/runtime handler or generic callback extension is proposed.

Lifecycle: Validated -> SourceReviewed -> Computing -> EvidenceSelected -> Prepared -> AwaitingApproval -> CheckpointCompleted -> ReportCommitted -> Completed. NoEvidence, Abstained, Denied, Unsupported, Failed, Stopped, Partial and OutcomeUnknown remain separate terminal results. A failure after a confirmed checkpoint preserves that effect and cannot masquerade as rollback or complete report.

## 3. Actual model and RAG flow

Use only the accepted G30 Qwen3-0.6B CPU profile (`abc9669adb66de6ca55e86f5a6793e6b6f2a694eee260f63dcc7120794b7fcc5`), G57 reranker CPU profile (`c47cc3f8008e4735ab43acfde124b884cab99f9206913cbecfc79ab06e2d0cdc`) and G68 embedding profile (`ca3cde459437692c9ae877b66cf4d15bab58c0852ec2f0fc7dc08e7003fdc71f`). Existing complete artifact/tokenizer/preprocessing/layout/rights identities must match; this spec admits no alternate profile or new download.

1. Read bounded EN/TR structured source documents through current G63 authority. Product parsing accepts language/site/owner/checklist fields and validates UTF8, closed shape and sizes. Test names/answers are not product constants. Explicit source-review acknowledgement is bound into Need and real-person start; authentication alone does not declare claims verified. Without review, keep Proposed and return NeedsSourceReview rather than promote reusable knowledge.
2. Build G88 entities and source-linked reviewed owner/checklist claims. A read-only host projection-preview method exposes exactly the bounded projection bytes that G88 will store. It grants no authority. Embed those exact bytes with G68 document preprocessing; after actual collection compare stored bytes/digest to the preview and reject mismatch. Never reuse a document vector as if it embedded different projection text.
3. Embed the actual query with the accepted G68 query prefix. G88/G70 uses those vectors and actual source-backed projections. Run separate bounded per-language memory/retrieval operations, then reauthorize their combined pins at one common trusted-time observation before commit/release. Rank over the authorized corpus; use a fixed bounded result set defined before candidate execution. Wrong-subject or missing-field candidates cannot satisfy report verification. A bounded predefined missing-field requery may consume the single repair allowance; no synthetic vector fallback or silently expanded top-k is allowed.
4. Run actual G30 Planning with the short request. Its valid short PlanProposal is included in the actual G57 question together with Need and retrieved evidence; the receipt/input digest retains that dependency. It is not executable action syntax. The decision selects only enumerated retrieved candidate bundles or abstains. The existing floor750000/margin100000, tie/missing/unsupported rules and uncalibrated status are unchanged. No choice is forced to meet the fixture.
5. Prepare report bytes from the selected actual source values. Verify required subject/field coverage, attributed conflicts, exact current citations/pins and source rights independently of the model. Only verified bytes and a fresh explicit user approval can proceed to checkpoint/commit.

G30 context includes prompt plus generation: at most64total tokens, with8generation tokens reserved, so the rendered prompt must fit56. No truncation or change to G30/G57/G68 numerical code is permitted. If the fixed job cannot complete under accepted profiles, report the precise profile/quality/resource blocker; do not lower factual criteria or silently enlarge context. Broad language quality, arbitrary prose extraction and general autonomous planning are not inferred from fixed structured EN/TR jobs.

## 4. Real human control and stop

A newly proposed `HumanPurpose::LocalReport` says that the person starts/reviews, approves and stops one private report on `state:report`, for at most five minutes. This is an additional **proposed** source scope: `crates/eylema-platform/src/human.rs`, limited to the purpose/resource/reason and purpose-bound regression cases. Existing LocalCli/AcceptanceProof purposes, native ABI/selectors and authentication implementation stay unchanged. The written-design decision must explicitly approve this path before implementation.

Use task/Need-bound host wrappers, not `enroll_verified_human` fixtures:

- request_report_controller(owner,grant,task,need_digest,ttl) returns a sealed ReportControlRequest. The authenticated challenge includes exact host/grant/nonce/task/Need digest and expiry. Generic unbound enrollment cannot accept LocalReport.
- enroll_report_controller consumes the actual matching DeviceOwnerProof into a sealed ReportController. Replayed, wrong-purpose/task/Need/instance and expired proofs reject. No proof, credential or human session is serialized.
- check_report_controller validates live instance/task/Need, immutable scoped rights, policy and remaining lease. Cancellation checks do not read source bodies, so withdrawal cannot disable stop.
- approve/cancel/revoke report wrappers preserve the existing ToolHost registry and actual pending-cancellation bookkeeping; expose no raw mutable DurableHost.

The controller retains terminal stdin; worker stdin/stdout are private bounded pipes. Model/source text cannot become an approve/stop command. Before commit the user sees escaped verified facts/citations and approves the currently displayed prepared digest/generation. A model flag or saved request cannot create that acknowledgement. Actual G54 approval covers its concrete checkpoint effect; the separate application approval also binds the actual prepared report. Both are required.

Stop cancels all registered CpuControls, stops future dispatch and releases no final report. Parent monotonic deadline remains authoritative across IPC; a child's relative deadline cannot reset it. Lease loss, source revocation, deadline and denial also stop future work. Cooperative cancellation is not hard real-time preemption. On grace expiry, terminate/wait only the owned worker and record uncertainty; do not claim cooperative success from a kill.

## 5. Privacy, commit and reopen

Sources, projections, embeddings, the short typed proposal, decision inputs, prepared/final reports, cost and review/control receipts remain project-private derivatives. Retain input/receipt digests and necessary typed proposals, not raw conversations, private reasoning traces, token dumps, credentials or worker stderr. Arbitrary valid text is not certified secret-free. No Train/ExternalDisclosure/EcosystemExport rights are created.

Every model dispatch checks current Execute rights; every derived write checks Collect; selection, effect admission, commit and final release recheck full current ancestry, actor/human lease, source digest/revision and TTL. Parent poll checks may cancel an in-flight worker, but already copied bytes cannot be recalled. All influences fit G63's existing16direct-parent/32depth and G88's8influence/32source limits; otherwise reject before write, never truncate lineage.

Before a checkpoint, persist the verified prepared report and a canonical unsigned fact-count input derived from its full ancestry. The G54 action binds that actual source. Obtain a genuine observed completed ToolReport and G88 observed episode; caller reports cannot substitute. Match the exact task/execution/action, canonical count input put pin, applied count and full prepared-report ancestry; a valid witness from another execution is InvalidWitness. Pin knowledge generation at selection; only the explicitly expected observed-episode append may advance it, after verifying original fact revisions/projection hashes unchanged. Any other change is RevisionConflict, not silent rebinding. Only then form ReportPermit and collect the final report with its exact prepared bytes, approval/control, model receipts and observed episode lineage. Recheck after synchronization before returning a receipt. Unknown write/receipt authority or a gap between checkpoint and final collect yields OutcomeUnknown/Partial, preserving effects and orphan facts.

Durable state/receipts use bounded immutable G63 records, not a new shared journal format. A fresh process opens actual private files with fresh owners/sessions, verifies pinned reports/receipts and reauthorizes all sources. Completed reports may be inspected; revoked/expired outputs cannot be read. G54 cannot resume recovered typed tasks: never reconstruct a ToolHost over old command history or automatically replay effects. Interrupted jobs remain Stopped/Partial/OutcomeUnknown until separately reconciled. Reopen is not an exactly-once or physical-erasure claim.

## 6. Fixed acceptance and resource envelope

Freeze exact document strings, prompts, candidate construction, per-stage token/byte/work predictions and expected citations in the implementation plan before model execution. The factual oracle uses independent literal values: EN Site North/owner Mira/checklist C7; TR Güney/sorumlu Ece/liste C9; unrelated and cross-project decoys. Product code reads values from sources. The fixed native Need includes two required bilingual sections and one optional unknown-site probe. NoEvidence is explicit for that optional section; an all-missing mandatory request yields root NoEvidence and no completed report. No expected answer is supplied in a model query or copied from a test constant by product code.

| Required case | Evidence |
|---|---|
| Actual bilingual job | Actual owned embeddings/planning/decision materially feed selection; two correct source-bound report sections/four facts and complete current citations; zero unauthorized citations; optional unknown-site probe returns actual NoEvidence with no facts/citations |
| Denied/unsupported | Foreign project/purpose, missing review, stale identities and unsupported model reject before unauthorized body/model/effect work; no fallback |
| No evidence/conflict | Actual retrieval path has no eligible requested facts or has contradictory claims; explicit NoEvidence/Conflict, never invented settled output |
| Bounded repair | One predefined pure Prepare/Verify or missing-field repair; second failure stops. Never retry uncertain writes/effects or reset counters. Synthetic injection and actual model evidence are labelled separately |
| Human stop | A fresh real person stops after an actual planning receipt and before final effects; no later decision/commit/output, prior control writes retained. Fake, expired or foreign controller cannot stop/approve another job |
| Fresh reopen | New process, current rights, same final bytes/digests/lineage; stale handles, withdrawn/expired citations and uncertain work reject without automatic resumption |
| Dependency checks | Removing planner text from decision input, swapping actual embedding/projection input, fabricated ToolReport or bypassing verifier/stop must fail owning regressions |

Proposed ceilings, to be instantiated without expansion in the later plan: one bilingual positive model job and one stopped job, at most one worker each and one fresh reader child (three children total), no automatic retry or new oracle. Positive root at most2.5e12CPU work; stop root at most1e11; suite2.6e12. Count artifact admission and failed/repair work; reserve a failed worker's full allocation if its consumption is unknown. Positive model calls at most9embeddings,2planning calls and2decision batches (each<=2questions×2options); the stop job adds at most1planning call. Independent maxima do not imply combined fit; reject before an unfunded stage. Actual success must not depend on spending all maxima. No calls are authorized by this design alone.

Human enrollment retains the existing<=60second authentication and<=300second lease including authentication time, consumed within existing120second enrollment age/15second proof freshness. Root<=300seconds, compute worker<=180seconds and always within remaining lease/root budget; no renewal/reset. If that cannot fit actual hardware, stop for a concrete decision. IPC<=256closed frames,<=8192bytes/frame,<=1MiB total; no maps, depth<=8, list<=32, checked extents before allocation. Worker diagnostics<=16KiB sanitized fixed categories. Native reader<=10seconds/4096diagnostic bytes. Actual-person participation and exact process/model trial allocation are requested only after runnable artifacts and the written implementation plan exist.

Load only one model's weight/tokenizer/workspace at a time; drop it before the next profile. Worker logical resident<=2.5GiB; controller separately reserves<=64MiB plus bounded IPC, total<=3GiB. These are admission limits, not measured RSS/fit/throughput. Memory/retrieval operations retain their independent100M/64MiB/5second cap; G74's10M route budget and dispatch are unchanged. The root separately caps facade/control work at4e9units and32memory operations; every operation retains its smaller accepted cap. Track CPU scalar work separately from facade work rather than pretend different units are interchangeable. Report actual token/work/timing/peak/RSS samples when measured; energy and monetary hardware cost stay unknown without measurement. Zero provider calls means no external API charge, not zero local cost; Codex development usage stays in its separate private ledger.

## 7. File map, qualification and delivery

Preparation changes only this EN/TR design and reviewed translation fingerprints. Proposed production paths:

- `apps/eylema/src/{main.rs,cli.rs,report.rs}`, new `report/{types,source,models,worker,control,verify,storage,wire}.rs`, app Cargo.toml and report unit/native tests.
- `crates/eylema-host/src/{tool.rs,human.rs}`, new `tool/report_control.rs` and host tests for task-bound real controllers; memory projection-preview method/tests under existing memory_store modules.
- Only `crates/eylema-platform/src/human.rs` for the new purpose and tests, subject to explicit scope approval. No native ABI or model/kernel change.
- Root Cargo.lock for the existing first-party model dependency; no root Cargo.toml, new crate, contracts/runtime handlers, G40/provider boundaries, workflow/checker or policy change.
- G72-only guide plan/card/profile/EN/TR action-plan/core-pin/fingerprint paths presented in the approved readiness proposal. Other67cards, original dependencies/order/budgets/history/prior profiles and non-core pins remain unchanged.

The proposed configuredgpt-6.1-sol/high binding covers only G72 structural application/model-SDK composition, bounded validation/IPC/source lineage and the minimal existing human-purpose bridge plus genuine separate review. It does not extend numerical/inference/training/performance/global-SOTA qualification or expose hidden serving identity. Actual owner admission is still pending; G88 qualification does not transfer automatically.

After written spec/profile/scope review, prepare the exact implementation plan with paths, frozen references, budgets, no-download cached-artifact identities and process/physical acceptance conditions. No artifact access, production code or runtime calls before that plan is authorized. Then require meaningful regressions, actual M4 acceptance, exact local Python/Rust/fmt/Clippy/build/Wasm/document checks, genuine separate final-head source/guide review, current CI and authorized merges with equal main trees/ancestry and exact-main CI. Source/static checks are not model/person/RSS evidence. Rollback is a scoped reviewed revert preserving private records and confirmed effects.
