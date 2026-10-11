# Owned Rust workspace bootstrap

Revision 0.1.0. English is canonical. [Türkçe](../tr/rust-workspace.md).

G07 introduces the first actual Rust workspace: one portable eylema-contracts library and one native eylema host consumer. Both packages are owned workspace-local code, unpublished and dependency-free outside the workspace. Cargo.lock contains exactly these two packages; there is no third-party production/development crate or build script. Distribution licensing remains undecided.

The contracts library currently exposes package build identity only. The native host accepts help/version and rejects unsupported, excess and non-Unicode arguments without a successful result or a panic. Four integration tests execute the compiled host as a real process. This proves bootstrap execution and dependency boundaries, not the task API/state machine, journal, HTTP service, lifecycle CLI or browser first proof. G08 adds typed task contracts; G14/G15 later add service/CLI behavior. The bootstrap package version does not change the existing API/NeedSpec/model-set schema versions.

The repository enters implementation / rust-v1 with runtime_implemented true as required by the accepted stage gate. That flag denotes implementation-stage source, not a completed task runtime or accepted product. The existing shared source/environment/checker/workflow policy is unchanged. A documentation checkout still cannot introduce Rust by changing its declared role. The portable Wasm package selection names only eylema-contracts; the native host is not built as Wasm.

The exact rust-toolchain.toml pins Rust 1.99.0, minimal profile, rustfmt/clippy and wasm32-unknown-unknown. The root workspace uses edition 2024 and resolver 3. The unchanged first-party gate performs locked/offline Cargo metadata, formatter, all-target/all-feature clippy with warnings denied, native tests/build, and the selected portable library Wasm build. All Cargo dependency entries must remain first-party workspace-local. Host/browser/SDK/driver boundaries remain explicit.

```sh
env -u RUST_LOG .venv/bin/python tools/check_rust.py
cargo run --locked --offline --package eylema -- --version
cargo run --locked --offline --package eylema -- --help
```

These commands require the official installed rustup/Cargo executables on PATH; G04 deliberately did not modify shell startup files. Add the standard cargo/bin location to the explicit invocation environment if needed. Clearing inherited RUST_LOG removes only the unrelated Codex host logging variable; the accepted environment guard is not relaxed. Run the six AGENTS tooling commands as well. Hosted ARM64 macos-26 CI executes actual Rust gates because the source profile is now implementation; hosted execution is separate from this local M4 acceptance.

Passed locally on Python 3.14.8, Rust 1.99.0 native ARM64, macOS 27.0.1 M4 Pro/24 GiB: formatter/clippy, four actual-process tests, native workspace build, owned dependency inventory and selected contracts-library Wasm compilation. Not run: Wasm/browser execution, task/service/oversight/journal behavior, GPU, model, training, performance or release acceptance. Current PR/main CI and independent exact-head review are separate delivery gates; local source compilation cannot replace them. The [task record](../../tools/work-plans/g07.json) binds scope, base, accepted dependencies and finite budgets.
