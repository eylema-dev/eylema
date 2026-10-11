# Repository quality gates

Revision **0.1.0**. [Türkçe](../tr/quality-gates.md).

## Current executable gate

The `repository-checks` job in [CI](../../.github/workflows/ci.yml) runs on pull requests and pushes to `main`. It uses a GitHub-hosted runner, a read-only token, pinned action commits, no persisted checkout credentials, no secrets, no model API and no self-hosted hardware. PR descriptions are read as data from the event file. The workflow does not use a privileged PR trigger or perform publication.

| Check | What fails | What it does not prove |
|---|---|---|
| Repository files | Symlinks, invalid JSON/YAML, broken/escaping local Markdown targets, private machine paths | External links remain reachable forever; comprehensive secret detection |
| Schema examples | Invalid Draft 2020-12 schemas, remote references, invalid request example, schema regression tests | Identity, approval, referenced artifact existence or correct model execution |
| EN/TR contracts | Missing/stale translation fingerprints, API operation or inline identifier mismatch, roadmap ID mismatch | Semantic translation accuracy |
| GitHub configuration | Missing intake fields, malformed forms, missing PR sections, unpinned/unexpected actions, privileged workflow settings | GitHub server-side template rendering or branch-rule enforcement |
| Real PR body | Missing, duplicated or placeholder-only required sections | Truth of evidence, reviewer independence or authority |
| Source-stage guard | Rust/Cargo sources added while only documentation checks exist | Rust compilation or execution; EYL-003 must replace this guard with actual gates |
| Whitespace and checker tests | Git whitespace errors, regression tests failing | Application behavior |

The PR-body check is conditional on a PR event. On a push, there is no PR body to validate. CI prints actual PR head SHA and checked-out revision: a PR run may check GitHub's synthetic merge commit. A green run for an older head is not current evidence. Editing a PR body triggers a fresh check.

## Maintainer gates

Requirements, production dependency policy, independent review, license, API/oversight meaning and release authority remain maintainer decisions. `CODEOWNERS` is routing metadata. No required-check ruleset or branch protection is activated by this workflow file. A maintainer can configure `repository-checks` as required after observing it and deciding review requirements. Do not grant elevated workflow permissions to make a fork pass.

Checker/policy changes must be inspected against the accepted base policy; a PR running modified validators cannot independently approve their weakening. Separate review must identify the exact reviewed revision. Until that review exists, report it as pending.

## Future executable gates

EYL-003 adds the pinned Rust toolchain, root workspace/lockfile, formatter, clippy, behavioral tests, dependency inventory and Linux/Wasm compilation before accepting Rust sources. Native Linux execution must be reported separately from compilation. Browser/device GPU, performance, persistence, policy/world-model, language-model and training acceptance are added at their roadmap milestones on controlled hardware. Public PR code never gains access to private data or trusted deployment runners by changing a workflow.

## Updating the checkers

Add regression tests that demonstrate rejection of the failure being fixed. Run the standard [contribution commands](../../CONTRIBUTING.md). Refresh translation fingerprints only after reviewing both languages. Keep the checked-in [historical validation snapshot](../records/historical-validation.json) labeled as historical; current CI logs are the evidence for the current commit.

References: [GitHub workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax), [secure use](https://docs.github.com/en/actions/reference/security/secure-use), [issue-form syntax](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms).
