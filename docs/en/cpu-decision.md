# G57 bounded CPU decision acceptance

Status: one authorized local artifact/reference acceptance invocation passed. This record is evidence for the exact pinned artifact and fixed inputs only. The G57 delivery remains pending independent final-head reviews, current exact-head CI and coordinated merge.

## Inputs and oracle

The pinned `Qwen/Qwen3-Reranker-0.6B` source revision was `e61197ed45024b0ed8a2d74b80b4d909f1255473`. The accepted BF16 artifact was 1,191,588,280 bytes with SHA-256 `27cd75a405b9c1b46b59abfd88aaa209e6fed2a1972cde9b70e7659537c5e65b`. Its 310-tensor layout and exact whole-file identity passed the owned loader. The bounded tokenizer input was the previously verified pinned `tokenizer.json`; its contents and token rows are intentionally not reproduced here.

The development-only F64 oracle was independently implemented in Rust and checked against the frozen Decimal80 fixtures. Its source was built with Rust 1.99.0 `-C opt-level=3`; the oracle's two synthetic tests and `--self-test` passed. An earlier attempt with the unoptimized build exceeded the 600-second cap and was recorded as failed; source and inputs were unchanged before the authorized optimized rerun. The measured reference digest was `147e416f1dca6006b03176db99f00c1553f7815da3ba6fc60ff31c11bb1f67d5`.

## Native results

Exactly one authorized acceptance command ran offline, locked and in release mode. It passed one ignored real-asset test in 487.46 seconds. Four frozen token rows were compared at context limit 128. Maximum per-layer hidden-state absolute difference was `0.0005589494`; maximum yes/no head-logit absolute difference was `0.0000113805`. Both were within the frozen per-value bound `0.001 + 0.0002 * abs(reference)`.

The two-question/four-pair batch returned the predeclared positive option for each fixed English and Turkish smoke question. Reversed question/option order reproduced the matching pair results. The cancellation job stopped during computation, reported no completed batch result and did not publish a partial proposal. The exact-context boundary succeeded; the one-below preflight rejected before forward work. Malformed and bounded-resource negative cases passed.

Aggregate charged work was `956151402665` units. Peak logical reservation was `1361720300` bytes; separately sampled process RSS was `905183232` bytes. These measurements describe this invocation on its execution host, not an isolation guarantee or universal capacity/performance claim.

## Limits and delivery gates

This tiny fixed smoke set does not establish calibrated relevance quality, broad Turkish quality, statistical task performance, general context capacity, throughput, GPU/Metal execution, training, production authorization, or permission to collect, disclose or export project data. Scores remain uncalibrated proposal data and cannot authorize effects. The first timed-out reference attempt remains in the private ledger. Model weights, tokenizer content, prompts, token IDs and private receipts are excluded from this repository.

Independent review of the final source, catalog and guide heads, current exact-head CI, dependency-ordered merges and post-merge main verification remain required before G57 can be marked accepted.
