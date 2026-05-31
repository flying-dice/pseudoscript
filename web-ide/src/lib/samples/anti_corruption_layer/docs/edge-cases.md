# Edge Cases & Resilience

The `Acl` model captures the clean read path: fetch a `TradeParty`, return a `Customer`. A reviewer's job is to press on the translation itself — because every mapping is a place where information, sync, and ownership quietly go wrong.

## Edge cases & failure modes

**Translation that loses information.** `toCustomer` flattens a rich `TradeParty` into a slim `Customer`. That `statusFlags` string might encode "credit hold," "VIP," and "pending KYC" — and if `Customer` has no field for them, the ACL silently drops them. Lossy by design is fine; lossy by accident means Relay makes decisions on data it doesn't know it threw away. Every dropped field should be a deliberate choice, not an oversight.

**Leaky mappings.** The whole point is that no ERP concept escapes. But a leak is easy: a `Customer.name` that is really `dba` verbatim, a status code passed through as a magic string, an `id` that is just `partyNum` with the ERP's formatting quirks intact. The concept escaped even though the type didn't. Review mappings for *semantic* leaks, not just structural ones.

**Keeping the ACL in sync as the legacy model changes.** `Erp.party` returns whatever the ERP currently returns. When the ERP team renames `dba` or repurposes a `statusFlags` bit, `toCustomer` is now silently wrong — it still compiles, still returns a `Customer`, just with garbage in it. The ACL needs contract tests against the real ERP shape so drift fails loudly.

**The cost of the extra hop.** Every `customer(id)` is now two calls — `Accounts` to `Acl` to `Erp` — plus a translation. For a single lookup that's invisible; for a list of 500 customers it's 500 ERP round-trips. The ACL is where N+1 problems are born, and where batching or caching has to be designed in.

**Where the ACL lives and who owns it.** `Acl` is a container *inside* Relay, which means Relay's team owns the translation and bears the maintenance when the ERP shifts. That is usually right — the consumer protects itself. But it's a real cost to budget, not a free wrapper, and the ownership must be explicit.

**Bidirectional translation.** This model is read-only. The moment Relay must *write* back — create or update a party — the ACL needs the inverse map, `Customer` to `TradeParty`, and now it must invent the `partyNum`, the `dba`, the `statusFlags` bits the clean model never carried. Round-tripping a value through both directions and getting the original back is a genuinely hard property to preserve.

## Resilience

The model shows the mapping but not its guards. Harden it by making `toCustomer` total and explicit — every `TradeParty` field is either mapped or consciously dropped, with no silent defaults; by contract-testing `Erp.party`'s shape so legacy drift breaks the build, not production; by caching or batching at the `Acl` boundary to absorb the extra hop; and by logging translations that hit unmapped or unexpected values so leaks and losses are observable rather than invisible.

## Pairs well with

- **Strangler Fig** — during ACME-style migrations, the new core reaches back into the legacy system through exactly this kind of ACL, so the old model never infects the new one.
- **Adapter / Gateway** — the ACL is a domain-aware adapter; a thinner protocol adapter can sit beneath it to handle transport while `toCustomer` handles meaning.
- **Published Language** — agree a shared schema with the ERP team and the ACL shrinks; the cleaner the upstream contract, the less translation you carry.
- **Repository** — hiding `Erp` behind a repository-shaped `Acl` keeps Relay unaware that the data is even foreign.
