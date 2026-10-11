# Public contribution and first application plan

Task: **EYL-009**, revision **2**, 11 October 2026.

## Accepted request and baseline

The maintainer requested an international public repository and asked to clarify the project structure together, then explicitly retained bilingual documentation. English remains canonical; Turkish documentation stays synchronized. Code, identifiers, file names and contribution templates use English. In the same conversation, the maintainer selected the first working flow: create, run, stop, save and reopen a task through a web interface. These instructions do not select a distribution license or approve a runtime implementation.

Base: `214a2c3d5313a251f1aae720b2ac1565f273d76b`, the fetched `main` revision. Branch: `codex/public-contribution-foundation`.

Reuse the contribution and CI foundation from [PR #2](https://github.com/eylema-dev/eylema/pull/2), revision `47552f85b2a52904bf45d73c5f8b76de130fca9f`, as an explicit dependency. Its existing branch, PR and unrelated working copies remain intact. This PR contains that foundation with English contribution surfaces and bilingual documentation; maintainers must reconcile the overlapping PRs before merging. Neither PR is merged by this task.

## Scope and sequence

1. Import the committed foundation into an isolated branch from the base above.
2. Group current English and Turkish documentation under `docs/en/` and `docs/tr/` with matching English file names. Keep bilingual root entry points and preserve original historical drafts under `docs/archive/`.
3. Use English contributor instructions, issue forms and PR headings. Preserve translation fingerprints, API identifier parity and all other gates, and test the changed PR heading contract.
4. Define the selected first application, proposed Rust component boundaries, dependency direction, acceptance scenarios and remaining decisions. Clearly distinguish the accepted goal from the proposed implementation.
5. Run the documentation gates, self-review the diff, and open a draft PR with current local and CI evidence.

Allowed paths: root Markdown documents and their documentation relocations; `AGENTS.md`, `CLAUDE.md`, `.gitattributes`, `.gitignore`, `.github/**`, `docs/**`, `api/**`, `tools/**`, `validation-report.json`, and `name-availability.json`. Existing JSON schema contracts and brand assets remain unchanged. No production source, dependencies, releases, account settings or deployment changes are in scope.

## Acceptance and verification

| Criterion | Check |
|---|---|
| Code, contribution templates and file names use English; documentation remains bilingual | Manual review, paired-document inventory, fingerprints and PR-heading regression fixtures |
| Current links resolve after relocation | Repository local-link checks and representative document inspection |
| Existing API operation IDs, methods and paths survive the migration | Compare to the base API catalogue, then run bilingual operation/identifier parity and drift regression tests |
| Contribution checks remain effective | Failure-oriented unit tests; hashed dependency installation; template and populated PR-body validation |
| First application and planned component structure are concrete | Scenario-to-milestone mapping and a proposal explicitly identifying undecided implementation details |
| Publication is reviewable | Whitespace, private-path/secret/diff inspection; current PR CI; independent review recorded as pending |

## Roles, resources and risks

Codex (GPT-6) performs architect, implementer, verifier and self-reviewer roles in one context. Tools: patch editor, shell, Git, Python, GitHub CLI and Codex worktree/PR attachment tools. No independent reviewer is claimed and no sub-agent is assigned.

Budget: one scoped PR, at most 90 minutes of work, three local repair cycles and two CI repair cycles. No paid service purchases, additional model-provider API calls, training or hardware workloads. Stop and report any required check still failing after its repair budget.

Risks: incoming links to relocated documents need the documented migration map; policy changes need independent maintainer review; PR #2 overlaps this dependency import; matching translation fingerprints do not prove semantic equivalence. Old history and historical evidence are retained without being presented as current validation. Reverting the scoped commits restores the prior documentation layout; no runtime data migration exists.

## Local evidence

Environment: macOS 27.0.1, arm64, CPython 3.14.8. Revision: this PR's source tree; GitHub CI records the actual head and checked-out merge SHAs. These results describe repository tooling, not application execution.

| Check | Result | Evidence or limit |
|---|---|---|
| `python3 -m venv .venv` | `passed` | Isolated local validation environment created |
| `.venv/bin/python -m pip install --require-hashes -r tools/requirements-ci.txt` | `passed` | Existing pinned dependency inventory installed unchanged |
| `.venv/bin/python -m unittest discover -s tools/tests -v` | `passed` | 16 tests, including rejection of obsolete PR heading suffixes; the imported baseline passed its original 15 tests |
| `.venv/bin/python tools/check_repository.py` | `passed` | 70 files, 2 schemas, 12 translation pairs, 20 bilingual API operations and 3 issue forms |
| `.venv/bin/python tools/check_pr.py --body-file .github/PULL_REQUEST_TEMPLATE.md --allow-template` | `passed` | Template structure only; actual PR body is validated separately before publication |
| `git diff --check` and `git diff --cached --check` | `passed` | Working and staged whitespace checks |
| Baseline preservation inspection | `passed` | Both API catalogues and inline identifiers match the base; schemas, synthetic request and all brand assets are byte-identical to the base |
| Documentation review | `passed` | All eight current guides in each language are recorded in the manifest; seven first-application scenarios match; changed translations self-reviewed before fingerprints were refreshed |
| Publication inspection | `passed` | Scoped diff and path/credential/model-file scan; scanner matches in Python rejection expressions and synthetic URI fixtures were inspected as test data, without weakening repository gates |
| Independent architecture/policy review | `not_run` | Separate reviewer not assigned; draft PR remains pending maintainer review |
| Rust/browser application, GPU/model and performance tests | `not_applicable` | This change implements documentation and repository tooling only; the runtime does not exist yet |

Current PR CI is recorded on the PR, separately from this local evidence. CI success does not establish independent review, merge, runtime support or deployment.
