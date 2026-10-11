# Contributing to Eylema

English is canonical. [Türkçe](CONTRIBUTING.tr.md). Contract revision: **0.1.0**.

Eylema currently publishes design contracts and repository tooling. The Rust web platform, GPU runtime and autonomous AI team remain roadmap work. The same contribution process applies to people, Claude, ChatGPT/Codex, MiniMax and other tools; supported provider/model combinations are verified per task, not promised by name.

## Start with a concrete need

Read [AGENTS.md](AGENTS.md), [architecture](architecture-v0.3.en.md) and [ROADMAP.md](ROADMAP.md). Use the feature, bug or architecture issue form. English or Turkish issue descriptions are welcome; committed identifiers and canonical documents remain English. For small corrections, an explicit maintainer request linked in the PR is sufficient.

Describe the initial state, action, expected result, affected targets, allowed paths and exclusions. Attach measurable acceptance criteria. A maintainer decides priority and authority; a new issue does not authorize a model to run commands or obtain secrets. Large interface, dependency, data/oversight or platform changes need a reviewed architecture proposal before implementation.

The [NeedSpec schema](ai-team-protocol/request.schema.json) is the machine-readable intake contract. GitHub forms collect inputs for it; no automatic issue-to-NeedSpec converter or AI service has been deployed. The current example remains synthetic.

## One task, one reviewable change

1. Fork the public repository or use an authorized branch. Start from current `main`; use `feat/<task>`, `fix/<task>` or `docs/<task>` where practical.
2. Write a short plan with task/revision, base SHA, affected paths, acceptance checks and finite time/model-call/repair budgets. Clarify genuine missing intent, not routine steps already authorized.
3. Implement a bounded change; keep production code in owned Rust components. Record any proposed dependency in the [tooling inventory](docs/en/tooling.md) and architecture proposal. Do not introduce dependencies through generated code or copied bundles.
4. Add behavioral tests where necessary. Update both document languages, review their meaning and refresh `tools/translation-pairs.json` using the command in AGENTS.md.
5. Run the [quality gates](docs/en/quality-gates.md). Capture exact commands, environment, outcomes and limits. A performance change needs a reproducible before/after comparison on named hardware or a justified inapplicability decision.
6. Open a draft PR with the supplied template. Prefer `Refs #123` while work is incomplete; use `Closes #123` only when the acceptance criteria are satisfied. Reuse the existing branch/PR for the same task revision.
7. Resolve CI failures and reviewer findings. Request maintainer review when evidence is current. New commits invalidate affected checks; a maintainer owns merge/release decisions.

An author may use multiple models but must state who actually performed each role. An independent review requires a separate reviewer/context and a report bound to the reviewed commit. Self-review must be labeled self-review. Lack of an independent review keeps that gate pending; do not claim it passed.

## Development commands

```sh
python3 -m venv .venv
.venv/bin/python -m pip install --require-hashes -r tools/requirements-ci.txt
.venv/bin/python -m unittest discover -s tools/tests -v
.venv/bin/python tools/check_repository.py
.venv/bin/python tools/check_pr.py --body-file .github/PULL_REQUEST_TEMPLATE.md --allow-template
git diff --check
```

Validate an actual PR description with `python tools/check_pr.py --body-file <path>` inside the virtual environment. The CI job reads the PR event payload without injecting its text into shell commands. It validates section structure and filled values, not truth or authorization.

## Evidence, language and review

Use `passed`, `failed`, `skipped`, `not_run`, or `not_applicable`; explain the latter three. Include base/head revisions and acceptance-to-test mapping in the PR. The workflow records actual head and checked-out merge revision so the PR body need not recursively update its own commit hash. Never supply private reasoning transcripts, credentials, production logs, personal data or model weights as proof.

English/Turkish fingerprints detect unrecorded edits; API identifier parity detects contract drift. Human or separately reviewed translation quality remains necessary. Historical architecture and availability snapshots are labeled and do not supersede current README or roadmap.

`CODEOWNERS` names the initial maintainer. It does not by itself require approval. This change supplies CI and a review policy; branch rulesets are separately administered and must not be described as enforced unless verified. Changes to CI, policy or schemas need maintainer attention; a PR cannot establish its own approval merely by relaxing checks.

The repository has no selected distribution license yet. License selection and contribution terms are an explicit release gate in the roadmap; do not add a license, CLA or DCO claim on somebody else's behalf. For suspected vulnerabilities, do not put exploitable details or secrets in a public issue; use GitHub's private reporting route if enabled, otherwise request a private channel from the maintainer without publishing the details.

## What is not automated yet

Issue triage, authority validation, model access, independent review, task orchestration, hardware benchmarking, model promotion and merge decisions are not automated by the templates. No workflow sends contributor content to a model provider or trains a model. See the [team design](ai-team-protocols-v0.1.en.md) for the planned protocol.
