# G46 loop, role and model-binding contracts

The portable `eylema_contracts::platform` module validates declared harness metadata. Platform metadata revision is 0.1.0; existing task contract 0.1.0 and EYL/EYLJ v1 are unchanged. See the [current action plan](https://github.com/eyleme-dev/devguide/blob/main/docs/en/platform-action-plan.md) and [nine roles](https://github.com/eyleme-dev/eylema-docs-en/blob/main/docs/architecture.md#rust-harness-model-roles-and-domain-selection).

`RoleKind` separates perception, reasoning, planning, decision, policy, world_model, specialist, critic and representation. A loop needs at least one role, not all nine. Multiple profiles can use one model; several bindings can use a compatible profile. STT/TTS/vision/text subprofiles are declared separately in the platform plan, not implemented as inference by this module.

`RoleInterface` binds positive semantic revision, input/output schema and units, preprocessing, action semantics and state reset by exact profile identity. A model profile includes model/profile IDs, declared 32-byte artifact identity, positive artifact revision and deployment intent. The zero digest is rejected as a placeholder. No file/hash/schema/model is read or verified here; profile ID equality does not prove physical behavior, artifact integrity, rights or support.

`ModelRegistry` rejects empty, duplicate or excessive profile sets. `LoopDefinition` resolves exact profile interfaces and takes immutable copies of the selected profiles in binding order. Replacing a registry cannot alter an already constructed definition. Edges require existing endpoints and matching source output/destination input schema and units. Feedback cycles are allowed under declared finite positive step/iteration bounds; enforcement during execution belongs to the future controller.

| Structural ceiling | Limit |
|---|---|
| Model profiles | 1–128 |
| Role bindings | 1–32, unique IDs |
| Edges | 0–128, unique directed pairs |
| Steps | 1–4096 |
| Iterations | 1–1024 |

These are input limits, not measured throughput/capacity. IDs wrap the existing bounded opaque ASCII ResourceId contract. `LocalOnly` rejects `ExternalConfigured` deployment. `ExplicitExternal` admits metadata only; it cannot authorize access, disclosure, paid calls, execution or a fallback. ProjectId is a logical association, not authentication or filesystem isolation. All model outputs remain proposals; actual tool/permission/data boundaries are G54/G63/G73 work.

## Executed evidence

At this PR head, on ARM64 macOS with Rust 1.99.0 and Python 3.15.0, the new integration tests exercise two different loop definitions through the same contracts, each optional role, every interface mismatch, registry/role/edge duplicates and bounds, missing references, zero artifact identity, edge schema/unit mismatch, local-only rejection and replacement snapshot identity. The test-first run failed on the missing platform module; the implemented tests then passed.

Passed: 5 new contract integration tests, 176 complete workspace Rust tests, pinned fmt/clippy/build and selected portable-library Wasm compilation; 47 Python tests, repository/PR/diff checks and refreshed EN/TR fingerprints. Task-only verification PATH selects the already installed toolchain; inherited RUST_LOG is removed because the reviewed runner rejects that override. CI runs its own reviewed environment and records exact SHAs. Fingerprints do not establish semantic equivalence; a separate final-head review is required.

Not run: Wasm execution of the new module, model inference, training, GPU performance, real human enrollment and complete harness jobs. A constructor test does not prove any of those. No external crate, provider call, model download, training record, production engine or G40 trial is included.

Rollback removes this module/export and its scoped notes through a reviewed PR. Existing task/wire data needs no migration because this card introduces no serialization, durable records or runtime effect.
