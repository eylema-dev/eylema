# Why Eylema — Brochure copy and production record

[Türkçe](../tr/brochure.md) · 11 October 2026

[PDF](../../assets/eylema-brochure/why-eylema.en.pdf) · [Editable copy](../../assets/eylema-brochure/copy.en.json)

## From idea to action.

**You're in control.**

A web platform vision for applications built by AI teams, operated by local agents and directed by people.

A clear purpose. AI with direction.

Continuous development and operations

EARLY DEVELOPMENT  /  PRODUCT VISION

## More possibilities. Clearer control.

Turning a need into working software takes a shared goal, clear authority and results you can inspect. Eylema is being designed to bring them together.

### One goal. A shared language.

A shared contract for people and AI: define what the application should do, which actions it may take and what success means. Keep development and operations connected to the same purpose.

### Set the boundaries.

Goals, permissions and budgets stay under human direction. The design includes approval where required, visible outcomes and the ability to intervene. A model cannot grant itself more authority.

### Put your device to work.

Local agents and models are planned around the task and available resources. The aim is to choose suitable capabilities using measured quality, latency and memory. Development providers and GitHub remain explicitly online.

### Make progress measurable.

The learning design connects permitted experience and human corrections to evaluated model candidates. Every improvement must earn its place through independent evaluation tasks and acceptance checks.

## Develop. Operate. Verify. Improve.

One connected cycle, with defined human authority and acceptance criteria at every stage. The full cycle remains a development goal.

### DEFINE THE GOAL

Describe the need, allowed actions, resource budget and expected result.

### BUILD AND REVIEW

AI teams prepare changes for testing, review and acceptance before a release.

### OPERATE AND OBSERVE

Local agents execute within granted authority. Outcomes and intervention results stay visible.

### EVALUATE AND IMPROVE

Permitted records inform development and training. Candidates need fresh evaluation before acceptance.

### A small core. Room to grow.

Owned Rust components form the foundation. 3D, local language models and world models are optional planned capabilities. The OS, browser, drivers and training tools remain explicit dependencies.

## Help shape what comes next.

Eylema is at an early stage. A clear need, a reproducible experiment or a thoughtful contribution can help shape its direction.

### Build

Contribute Rust components, tools and meaningful tests.

### Investigate

Develop evaluation tasks, hardware measurements and permitted learning recipes.

### Make it clearer

Improve documentation, examples and English–Turkish explanations.

### WHERE WE ARE TODAY

An initial Rust workspace, bootstrap commands, validated task types and bilingual design documents exist. The full platform remains in development.

Demonstrate creating, running, stopping, saving and reopening a task in the browser. GPU, models, learning and autonomous development have separate future acceptance gates.

**Start with your first contribution.**

Explore the project. Describe a need. Shape a contribution.

[github.com/eylema-dev/eylema](https://github.com/eylema-dev/eylema)

## Production and verification

Four A4 pages using the existing Eylema logo, orange, ink and paper colors. The PDF is for digital sharing and ordinary A4 printing, with no press bleed or CMYK profile. The Markdown transcript supports accessible reading and copy comparison; the PDF is not a tagged accessibility document.

The corresponding JSON file is the copy source. The builder generates the PDF and this Markdown transcript from the same input. Refresh translation fingerprints after reviewing English and Turkish meanings. Fingerprints do not prove semantic equivalence.

```sh
python tools/render-brochure.py --font-dir <licensed-arial-directory> --verify
.venv/bin/python tools/check_repository.py --write-translation-hashes
```

Authoring tools are not added to the application runtime or CI. Versions, source and output hashes are recorded in [tooling.json](../../assets/eylema-brochure/tooling.json).

The tool uses existing installations, with no network downloads or installs. Font files are not distributed; only used glyph subsets are embedded in the PDF. Licensed Arial Regular, Bold and Black are required. Visual-check outputs are kept under `.cache/brochure/`.

## Claim sources

Current status follows the core revision below. Older status sentences in design documents do not override that progress. Development, local operation, human oversight and the learning cycle are not presented as completed product capabilities.

- [Core status](https://github.com/eylema-dev/eylema/blob/7e64118196845814d9e8de531fb817c9f14fde5a/README.md)
- [Typed task contracts](https://github.com/eylema-dev/eylema/blob/7e64118196845814d9e8de531fb817c9f14fde5a/docs/en/typed-contracts.md)
- [Architecture](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/architecture.md)
- [API and oversight](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/api-reference.md)
- [Roadmap](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/roadmap.md)

This brochure creates no new runtime, GPU, model, training or performance evidence. Review in this Codex context is self-review; independent review remains pending.
