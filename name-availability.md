# Eylema — Name and availability record

English is canonical. [Türkçe](name-availability.tr.md).

**Brand decision:** Eylema, accepted by the project owner on 11 October 2026. Public display name: **Eylema**. Proposed CLI name and package prefix: `eylema`; these are design conventions, not published packages or implemented commands.

**Checked:** 11 October 2026, approximately 15:49–15:56 Europe/Istanbul. The registry/API timestamp and browser observation window are recorded in [the machine-readable record](name-availability.json). Availability can change before registration.

**GitHub update — 11 October 2026:** The [eylema-dev organization](https://github.com/eylema-dev) and public [eylema-dev/eylema repository](https://github.com/eylema-dev/eylema) have now been created. The results below preserve the earlier, pre-creation availability check. The repository is the current documentation destination.

**Domain update — 11 October 2026:** The project owner reported purchasing **`eylema.dev` through GoDaddy**. It is the primary project domain. This update records the owner's report; the availability table below remains the historical pre-purchase observation. DNS, TLS and platform deployment acceptance were not checked by this logo work.

## Domain results

| Candidate | Registrar observation | Registry cross-check |
|---|---|---|
| `eylema.com` | Namecheap offered registration with an “Add to cart” control, USD 10.98/year displayed before optional promotions. | Verisign returned HTTP 404 without a parsed RDAP error object; retained as inconclusive registry evidence. |
| `eylema.dev` | Namecheap offered registration with an “Add to cart” control, USD 10.98/year displayed. | Google Registry RDAP returned HTTP 404 / `errorCode: 404`: no registration record found. |
| `eylema.ai` | Namecheap offered registration with an “Add to cart” control, USD 89.98/year displayed; minimum two-year registration and USD 114.98/year renewal shown. | Identity Digital RDAP returned HTTP 404 / `errorCode: 404`: no registration record found. |

Registrar evidence came from the [Namecheap search page](https://www.namecheap.com/domains/registration/results/?domain=eylema.com): the initial `eylema.com` query also listed `eylema.ai`, and a subsequent search-field query for `eylema.dev` returned its registration offer. Prices are observations of the search screen, not a final checkout quote. No domain was added to a cart, purchased or reserved.

Registry endpoints were selected from the [IANA RDAP bootstrap](https://data.iana.org/rdap/dns.json): [Verisign `.com`](https://rdap.verisign.com/com/v1/domain/eylema.com), [Google Registry `.dev`](https://pubapi.registry.google/rdap/domain/eylema.dev), [Identity Digital `.ai`](https://rdap.identitydigital.services/rdap/domain/eylema.ai). An absent RDAP record alone does not establish purchasability. The registrar offers above are the separate live availability signal.

## GitHub organization results

| Candidate | Public namespace | Organization-name form |
|---|---|---|
| `eylema` | **Occupied** by the existing [EylemA user account](https://github.com/EylemA); public API returned HTTP 200 and `type: User`. | Not a usable new organization name while the account holds it. |
| `eylema-dev` | Public users API returned HTTP 404 / `Not Found`. | The organization setup form's live name check returned success. No organization was created or reserved. |
| `eylema-platform` | Public users API returned HTTP 404 / `Not Found`. | The organization setup form's live name check returned success. No organization was created or reserved. |

Sources: GitHub public API for [eylema](https://api.github.com/users/eylema), [eylema-dev](https://api.github.com/users/eylema-dev), [eylema-platform](https://api.github.com/users/eylema-platform), and the [organization setup form](https://github.com/organizations/new?plan=free). The form was used only to validate the name field; contact details, terms acceptance and creation were not submitted. A successful name check does not reserve the name. GitHub also explains that apparent inactivity does not make a username available in its [username policy](https://docs.github.com/en/site-policy/other-site-policies/github-username-policy).

## Public identity proposed at the original check

- Brand: **Eylema** — accepted.
- Primary project and documentation domain: **`eylema.dev`** — recommended, unregistered by this task.
- GitHub organization: **`eylema-dev`** — recommended; name validation passed, creation pending a separate instruction.
- Future main repository: **`eylema-dev/eylema`** — proposed path, not an existing repository claim.
- `eylema.com` can serve as an additional brand address if later acquired. `eylema.ai` is optional.

The original availability check did not establish domain or organization ownership. Subsequent organization and repository creation and the owner-reported domain registration are recorded in the updates above. Existing protocol identifiers, API operation identifiers, file paths and contract versions were preserved. The illustrative schema example uses `example/eylema`, explicitly a placeholder. This check does not establish trademark clearance or package-registry name availability.
