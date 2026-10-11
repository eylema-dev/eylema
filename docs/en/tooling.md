# Tooling and dependency inventory

Revision **0.1.0**. [Türkçe](../tr/tooling.md).

The production target is owned Rust components with no third-party Cargo packages by default. This repository currently has no production code. CI tools do not become application dependencies. Linux, the browser, drivers, GPU interfaces and model artifacts are explicit platform inputs, not magically removed by using Rust.

| Boundary | Tools | Purpose / pinning |
|---|---|---|
| Repository validation only | CPython 3.14, `jsonschema` 4.26.0, PyYAML 6.0.3 | Official standards parsing/validation; exact package versions and all artifact hashes in [requirements-ci.txt](../../tools/requirements-ci.txt) |
| Transitive CI-only packages | attrs 26.1.0, jsonschema-specifications 2025.9.1, referencing 0.37.0, rpds-py 2026.9.1 | Locked with hashes alongside direct packages; none ships in the application |
| GitHub checkout | actions/checkout v7.0.1, `3d3c42e5aac5ba805825da76410c181273ba90b1` | Official GitHub action; credentials not persisted |
| Python provisioning | actions/setup-python v7.0.0, `5fda3b95a4ea91299a34e894583c3862153e4b97` | Official GitHub action; Python minor fixed to 3.14, patch version printed in CI |
| Hosted environment | GitHub `ubuntu-24.04`, Git | Hosted image revisions may change; record runner/runtime versions when reproducing results |
| Planned production toolchain | Rust/Cargo, Wasm target tools | Exact version, target inventory and Rust checks selected in EYL-003; no Rust test is claimed today |
| Planned training environment | Unsloth and its Python/accelerator dependencies | Separate pinned training image/recipe at M6; not installed or used by repository CI |

Reproduce installation with `python -m pip install --require-hashes -r tools/requirements-ci.txt` in an isolated Python 3.14 environment. Updating a tool requires reviewing its release/provenance, resolving the complete dependency set, recording artifact SHA-256 values from PyPI, running tests with the new lock, and updating this inventory. GitHub action updates require an official release tag resolved to its full commit SHA. No automatic dependency updater is installed.

The first-party policy does not justify writing unreviewed cryptography, replacing standard schema validation for this docs-only phase, or claiming a complete Rust-only operating system. Changes to the production dependency boundary require an architecture proposal and maintainer decision. Minimal browser host bindings and shader sources must be explicitly inventoried at M3.
