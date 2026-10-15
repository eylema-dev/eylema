# Eyleme product family

Revision **1.0.0**, 15 October 2026. English canonical; [Türkçe](../tr/product-family.md). This records the maintainer's accepted product and business model. Implementation maturity remains scoped by the [core evidence](../../README.md) and each product's acceptance records.

## One brand, three products

**Eyleme** is the single brand and the independently usable open-source community product. [eyleme.dev](https://eyleme.dev) is the common public entry point for getting started, documentation, contribution and the commercial product overview. The owner reports registering this domain; publication and ordinary DNS/HTTPS availability require separate live evidence.

| Product | Role | Source and business model |
|---|---|---|
| Eyleme | Run and inspect explicitly authorized tasks and compose supported agentic workflows | Public open-source product; first-party source and documentation remain Apache-2.0 |
| Eyleme Derleme | Collect, prepare and integrate data with explicit provenance and access boundaries | Separately developed private-source product offered through paid subscriptions |
| Eyleme Onama | Evaluate policy and authority and provide execution assurance for explicitly integrated workflows | Separately developed private-source product offered through paid subscriptions |

Present the combined workflow as **Derleme → Onama → Eyleme**: prepare data, evaluate authorization, execute permitted work. This is a product explanation, not a mandatory runtime dependency chain. Policy checks may apply before collection, planning, execution and disclosure. Shared branding, a subscription or an Onama decision never implicitly grants access, data rights or execution authority.

## Standalone open-source contract

Supported standalone Eyleme workflows must remain usable without a Derleme or Onama subscription, paid account, license server or commercial-service connection. Eyleme retains its basic access checks, bounded execution, explicit denial and local safety controls. Existing host-verified authorization, cancellation and recovery limitations remain visible; this decision does not claim that pending human-authentication or production gates have passed.

Derleme and Onama integrations are optional and must use explicit, versioned public interfaces. Entitlement to a paid product is distinct from permission to act on a source, device, workspace or project. Each product enforces its own authority and safety boundary; an unavailable paid integration must not silently bypass checks or block an unrelated supported standalone workflow.

## Community and commercial boundaries

The public ecosystem centers on useful examples, documentation, issue discussion, reviewed contributions and compatible extensions. Public API/interface documentation, schemas and community extension examples may remain open under their stated licenses; private implementation repositories, customer configurations, credentials and operational data are not community prerequisites. The [contribution workflow](https://github.com/eyleme-dev/devguide/blob/main/CONTRIBUTING.md) continues to govern changes and independent review.

Derleme and Onama's new commercial implementations are developed privately and offered by subscription under separately stated terms. Repository visibility alone does not relicense previously published code or revoke existing Apache-2.0 grants. Preserve notices and provenance for any already licensed components and distinguish them from new commercial code. Eyleme's Apache-2.0 license does not automatically apply to every product carrying its brand. See the canonical [license and contribution scope](https://github.com/eyleme-dev/devguide/blob/main/docs/en/licensing.md).

Commercial terms, prices and subscription availability are not yet published. Subscription packaging, billing, entitlements, service levels and commercial availability require their own implementation and publication evidence. This model does not create a checkout, payment integration or generally available subscription service. A subscription need not require hosted deployment; any supported local or hosted offering must state its actual installation and data boundaries.

## Naming and evidence

Use **Eyleme**, **Eyleme Derleme** and **Eyleme Onama** in customer-facing product descriptions. Keep the existing technical names, commands, repository ownership, protocol/schema identifiers, environment variables, stored data paths and license notices unless a separate migration explicitly changes them. The historical `eylema.dev` address is not the current community destination; its existing records are retained as history.

The commercial overview is public at the [website product section](https://eyleme.dev/#products); private source access is not required to understand the family. This document sets the accepted direction; repository privacy, website delivery, runtime execution and subscription availability are separate claims that must each be verified.
