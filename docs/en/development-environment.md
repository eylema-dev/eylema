# Native development environment — G04

Revision 0.1.0. [Türkçe](../tr/development-environment.md). This is development-tool evidence for one physical Mac, not an implemented Eylema application. The documentation source guard remains active; G07 supplies the root workspace, toolchain file and actual implementation CI together.

## Observed environment and reproducible probes

[Observed JSON](../../tools/doctor/observed-macos.json) binds exact versions, commands, doctor/fixture source hashes, outputs and artifact hashes. Temporary artifacts are deleted after checks; rebuilding may produce different binary hashes. Source/tool/environment identities and behavior must be rechecked, not inferred from the prior report's passed labels.

| Input | Observation |
|---|---|
| Physical device | One Apple M4 Pro, 25,769,803,776 bytes unified memory (24 GiB) |
| macOS | 27.0.1, ARM64 |
| Rust | 1.99.0, aarch64-apple-darwin, commit b940084d7eb6a299eb4bfeb8e34901bc051e7ac4 |
| Developer tools | Rustup 1.29.1, Cargo 1.99.0, rustfmt 1.10.0-stable, clippy 0.1.99 |
| Rust targets | aarch64-apple-darwin and wasm32-unknown-unknown |
| Apple SDK/tools | Xcode 27.0 / 27A266a, macOS SDK 27.0, Metal 32023.921 |
| Browser inventory | Chrome 154.0.8037.98; no browser execution or WebGPU acceptance here |

The [owned doctor](../../tools/doctor/doctor.py) uses Python 3.14 solely as development tooling and generates three explicit temporary first-party smoke fixtures: a Rust executable with a known message, a portable Rust function compiled to Wasm, and a Metal 4.0 kernel compiled/linked without GPU submission. It adds no production crate or inference engine. The native binary's ARM64 Mach-O type and actual returned message are checked. Wasm version/header, nonempty AIR and metallib magic are checked after successful tool invocation. Wrong/missing tools, unsupported host/device, unexpected Rust commit/release, missing targets, failed/timed-out commands and invalid outputs fail the doctor.

```sh
env -u RUST_LOG .venv/bin/python tools/doctor/doctor.py
env -u RUST_LOG .venv/bin/python tools/doctor/doctor.py --output /tmp/eylema-doctor.json
```

RUST_LOG was inherited from the Codex host in the initial run, and the accepted compiler-environment guard rejected it. Clearing that unrelated logging setting keeps the guard unchanged. Never clear a compiler override merely to hide its effects; run with the reviewed environment. Each command is bounded to 120 seconds. Failure exits nonzero, prints a failed result and preserves an existing output file. Home/probe paths and transient Metal mount directories are redacted; device serials, credentials and private training data are not collected.

## Installation and boundaries

[Installation record](../../tools/doctor/installation.json) fixes the official rustup installer version/URL/SHA-256 and requested components. Verify downloaded bytes against the official SHA before execution. [Rustup installation](https://rust-lang.github.io/rustup/installation/index.html) and [versioned toolchains](https://rust-lang.github.io/rustup/concepts/toolchains.html) describe the standard tools and exact release selection. The local installation used no shell-profile edits; enable Cargo's standard bin directory in the current shell when using Rust commands.

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy --target wasm32-unknown-unknown
xcodebuild -downloadComponent MetalToolchain
```

The Metal compiler stub was discoverable before installation but could not execute; metallib was missing. Installing the official component made compiler/linker invocation succeed. Apple's [additional-component instructions](https://developer.apple.com/documentation/xcode/downloading-and-installing-additional-xcode-components) describe this tool boundary. No sudo, repository permission, deployment or paid service was used.

Passed: actual native Rust compile/run, Wasm compile, Metal shader compile/link and local environment inventory. Not run: Eylema runtime/workspace, Wasm/browser execution, GPU compute/readback, model inference, training, performance or release acceptance. Six failure-oriented unit tests use mocked commands and synthetic artifacts; their success is validator evidence, separate from the real local probes. Output replacement is atomic and storage failure preserves the prior report. Hosted repository CI executes those tests and keeps the documentation Rust gate skipped; it does not rerun this Mac's environment acceptance. G07 and G19 must provide their own actual implementation/runtime/GPU evidence.
