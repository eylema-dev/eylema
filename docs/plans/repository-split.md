# Organization repository split

Task `repository-split`, revision 2, 11 October 2026. Authorization: the maintainer's explicit chat request to create `eylema-docs-en`, `eylema-docs-tr` and `devguide` in `eylema-dev` and move the relevant documents. This record is traceability, not an authorization mechanism.

## Base and scope

Core base: `214a2c3d5313a251f1aae720b2ac1565f273d76b` (`main`). Existing public contribution draft `406d1ab639cf70149f89832a1b03bd06d2d72633` supplies the proposed bilingual document organization and tooling; it is not merged policy. Preserve the original dirty checkout and open PRs #2 and #3.

Allowed paths: the core document entry points, relocated Markdown/JSON records, `docs/plans/`, `.github/`, contributor adapters, `tools/`, `.gitignore`, `.gitattributes`; all initial documentation and repository-tooling files in the eight new repositories. The second explicit maintainer message extends scope to .github, eylema-examples, eylema-evals, eylema-models and eylema-training; these receive honest scaffolds only. Keep core schemas, brand assets and artwork source in `eylema`. No runtime, website modification, license selection, permissions, merge, deployment or release.

## Design and acceptance

| Requirement | Implementation and check |
|---|---|
| Eight named repositories exist in the organization | GitHub repository metadata and explicit public visibility matching the source docs |
| One owner for each maintained document | Migration inventory with source revision and hashes, destination paths, no remaining core prose duplicates |
| English canonical and Turkish synchronized | Local reviewed fingerprints plus pinned repository-set verification, API identifier parity and roadmap ID parity |
| Repeatable contribution setup | Central contract in devguide, thin per-repository adapters, pinned CI-only tooling, issue/PR templates and negative validator tests |
| Portable navigation | Local Markdown target checks and cross-repository target checks over the pinned repository set |
| Reviewable delivery | One migration branch/PR per repository, exact local commands, actual hosted CI, dependency/rollback instructions |

## Roles, risks and budget

Codex (GPT-6) performs architect, implementer, verifier and self-reviewer roles using Python, shell, Git, GitHub CLI and Codex worktree/PR tools. No subagent or separate reviewer is claimed. Independent maintainer review remains pending.

Risks: split-repository translations can drift, default-branch links require coordinated merges, and older PRs can restore removed documents. Use immutable repository-set pins, explicit merge order and inventory checks; preserve Git source history through immutable provenance links. Shared tooling is maintained in devguide with explicitly inventoried copies so local checks do not silently execute fetched code. These are documentation checks, not runtime/GPU/model evidence.

Budget: at most 150 minutes, 240 tool calls, three repair cycles per failing gate, no paid services, no model-provider calls beyond this Codex session and no automatic merge. Stop and report any remaining failure or exhausted budget.
