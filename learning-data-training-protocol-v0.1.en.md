Revision: **0.1.0** · Canonical language: **EN** · Secondary language: **TR** · [Turkish companion](ogrenme-verisi-ve-egitim-protokolu-v0.1.md)

# Eylema — Learning data and training protocol — v0.1

Date: 11 October 2026. Status: **design proposal**. This document does not enable collection, training, or deployment. No training, export, performance measurement, or model acceptance test has run.

Related designs: [architecture](architecture-v0.3.en.md), [AI team protocols](ai-team-protocols-v0.1.en.md), and [behavioral API and human oversight](api/en/reference-v0.1.md). Identifiers remain English across language editions.

## Purpose and architectural boundary

The goal is to accumulate attributable, verifiable examples that improve local models as the platform operates. Success means better completed tasks within the same resource budget. Log volume and training loss alone do not establish improvement. Frontier models and humans may teach, review, and correct; their judgments remain distinct from observed outcomes.

Our production Rust components will manage collection policy, task execution, model assignment, and accepted model packages. Unsloth is an explicitly selected external development tool within the training and packaging environment. Python, training libraries, and converters enter that environment's dependency inventory, without becoming hidden Rust runtime dependencies. Training-server failure must not stop accepted applications.

Flow: permitted events → episode/feedback ledger → selection and verification → immutable dataset release → pinned training recipe → candidate model → independent evaluation → release PR → authorized approval → canary → activation or rollback. Skill/workflow teaching is versioned separately from weight training; training is optional when a skill change solves the measured need.

## Records and permission

An `Episode` spans a task from start through completion, cancellation, or timeout. It links `episode_id`, task/tenant identity, timestamps, environment version, model/skill hashes, observations, proposed and executed actions, outcomes, errors, and resource measurements. Large images/videos use authorized object references instead of inline event content. Missing or unrecorded events carry explicit gap markers.

`FeedbackRecord` attributes human corrections, teacher suggestions, verifier results, and disputes. A new judgment explicitly supersedes its predecessor rather than silently replacing it. Explanations, tool calls, and observable outcomes suffice; collecting private model chain-of-thought is unnecessary. `EvidenceRecord` identifies an executed check, its version, environment, and result. Traceability does not prove label correctness.

Operational telemetry permission is not training permission. Before collecting content, obtain explicit opt-in and establish `training_scope`: `none`, `tenant_private`, `organization_shared`, or `public_contribution`. Purpose, data categories, retention, authorization source, and withdrawal method remain visible. Sending data to an external teacher API requires its own scope. Service access must not depend on accepting training sharing.

Remove secrets, authentication material, and unnecessary personal content at the collection boundary. Tenant separation, access controls, and retention also cover raw objects. Immutable release identity does not require permanent raw-data retention: deleted or withdrawn content becomes inaccessible, and affected packages are reassessed. Do not assume withdrawal automatically removes information from trained weights.

## Different data for different objectives

| Package | Core records | Use |
|---|---|---|
| Language/tool use | Input, visible context, tool schema, verified output/correction | SFT, instructions, tool selection |
| Preference | Two candidates for one input, preference, evaluator, rationale | Preference training such as DPO |
| Action trajectory | Ordered observations, executed actions, rewards/costs, termination, cancellation, action constraints | Imitation and supported policy training |
| World transition | Previous state/observation, action, elapsed time, next observation, environment/sensor versions | Action-conditioned dynamics prediction |
| Hardware performance | Model hash, workload, context length, concurrency, CPU/GPU, driver, memory, latency | Selection, assignment, capacity profiles |

Serializing movement into chat does not automatically make it suitable world-model training data. Partial observability, coordinates, units, and time steps belong to the contract. Distinguish cancellation from natural termination and retain reward versions. Outcomes learned later must not leak into decision-time inputs. Compare hardware measurements only with compatible workload and environment definitions.

Unsloth's dataset guide distinguishes corpus, instruction, conversation, and preference/feedback formats, and calls for checking synthetic examples' quality and relevance. Our episode contract is broader; each adapter produces the selected method's required format. [Unsloth dataset guide](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide/datasets-guide)

## Valuable selection and label reliability

Selection signals include human intervention, failure, unexpected outcomes, model disagreement, new task coverage, and performance regression. None guarantees a correct label or valuable training example. Reduce repetitive easy examples; queue difficult cases, counterexamples, and corrections for review. Do not imitate wrong actions as good behavior: retain them as failures or corrected trajectories.

Maintain a representative sampling quota alongside difficult examples to avoid distorting actual usage. Report task, tenant-scope, language, device, and failure distributions. Preserve selection reasons, sampling probabilities where applicable, and example weights. Avoid retaining only successes; make observation bias caused by action selection visible.

`label_source` distinguishes at least `observed_real`, `simulated`, `teacher_generated`, and `human_reviewed`; one example may have multiple sources. Simulator version, teacher model/prompt version, and human correction have separate references. A teacher rating its own answer highly is not acceptance evidence. Use executable verifiers, independent sample audits, and a disagreement queue. Generated predictions never replace the observed next state.

## Evaluation separation and dataset releases

Clean and group exact/near duplicates before train/development/test splitting. Keep episode fragments, document derivatives, and variants of one synthetic seed in the same partition. Generalization to other tenants and future use are separate evaluation objectives; reserve tenant groups and forward time windows accordingly. Justify coverage and sample counts instead of applying one universal percentage.

Keep the final holdout unavailable to teacher generation, selection, and hyperparameter search. A repeatedly consulted candidate-selection set is a development set. Once final test results guide selection, a new locked evaluation is required for subsequent independent acceptance. Retire an evaluation version when corrections from its test examples enter training.

`DatasetRelease` carries content manifests/hashes, schema, source rights, scope permissions, cleaning recipe, duplicate groups, split procedure, and label-acceptance evidence. Raw-record-to-example transformations should be reproducible. Dataset, training, evaluation, and model versions have separate identities; new releases do not mutate earlier contents. Flag packages that can no longer be reproduced because content was deleted or access revoked.

## Unsloth training adapter

The official guide documents LoRA/QLoRA, full fine-tuning, pretraining, SFT, preference optimization, and GRPO/GSPO examples. Select by task, supported architecture, and hardware. A narrow first task may need only an SFT/LoRA or QLoRA comparison. Training loss is not an acceptance criterion by itself. [Unsloth training guide](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide)

The current main repository lists diffusion, decision, embedding, and audio models alongside language models. That list does not prove automatic support for our RSSM/Dreamer, MuZero, or another custom network. Verify the loader, forward computation, loss/reward, data format, training method, and export path for each architecture. Reject unsupported paths explicitly; do not silently substitute another engine. [Official Unsloth repository](https://github.com/unslothai/unsloth)

`TrainingRecipe` pins the Unsloth commit/version, dependency lock, base-model hash/license, tokenizer/template, dataset release, method, precision, seeds, parameters, reward/verifier version, budget, and output contract. Execution produces a `TrainingRun`; planned, completed, and early-stopped steps remain distinguishable. Identical recipes do not imply bit-identical results across hardware.

## Export and Rust acceptance

Unsloth provides GGUF saving/quantization and merged 16-bit export paths; its documented manual GGUF conversion uses llama.cpp tools. Record any converter version in the training/packaging inventory. A GGUF or safetensors file does not establish that our Rust engine supports its architecture and every required operator. [GGUF export](https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf)

Acceptance sequence: training reference → unquantized export at its declared precision → CPU reference → each intended precision/quantization and target backend. Quantization is optional; record it as not applicable when unused. A 16-bit export is not automatically a full-precision reference. Check tokenizer, chat template, BOS/EOS, masking, and preprocessing/postprocessing. Unsloth documents degraded external results caused by template and special-token differences. Define numerical tolerances beforehand; additionally measure task success, stopping behavior, multistep prediction, and closed-loop action outcomes. [Inference troubleshooting](https://unsloth.ai/docs/basics/inference-and-deployment/troubleshooting-inference.md)

## Releases, ecosystem, and first delivery

Preserve the AI team protocol's `NeedSpec`, `WorkPlan`, `ChangePackage`, and evidence chain. Sufficient new data, verified distribution drift, or regression may create a training-need candidate. Recipes specify minimum new coverage/examples, cooldown, concurrent-job limit, GPU-hour budget, and stopping conditions. Every log event must not trigger training; one regression must not cause unlimited retries.

Automation may prepare candidate training, comparisons, and a release PR. The PR binds dataset/training/evaluation IDs, model hashes, code SHA, hardware evidence, and differences from the current release. After authorized human or previously approved policy approval, `ModelSetRelease` enters canary. Threshold violations restore the previous compatible release. Promoting new weights remains distinct from runtime assignment within an approved set.

The open ecosystem accepts dataset, reward, evaluation, model, and hardware-profile packs. Each records provenance, contributor credit, license, permission scope, schema version, compatibility, and hashes. Review community reward code before activation and execute it in a restricted environment. Development evaluations may be shared; locked holdout examples, private tenant content, and reconstructive details must remain private. Open availability does not establish trust.

The first delivery should cover one task: permitted episode capture, human correction, representative selection, locked evaluation, one Unsloth recipe, and a Rust acceptance report. Measure end-to-end success, incorrect actions, human intervention, separately authorized escalation to frontier models, p95 latency, memory, and cost/measurable energy per successful task. A local-only workload must not silently use a cloud model; absent authorization, escalation remains unavailable. Log volume and training loss are diagnostics. Do not release candidates without demonstrated benefit; retain the reason and findings.
