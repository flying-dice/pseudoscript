# The Pattern

Relay is a clean, modern CRM. The ERP it has to read from is not. **Anti-Corruption Layer** shows how Relay borrows the ERP's data without inheriting its vocabulary — so a `Customer` stays a `Customer`, and the ERP's bizarre "trade party" never leaks past the door.

## The problem

A `SalesRep` opens Relay and looks up a customer. Relay thinks in clean domain terms: a `Customer` has an `id` and a `name`. But the customer data physically lives in a legacy `Erp`, whose model is from another era — it speaks of a `TradeParty` with a `partyNum`, a `dba` ("doing business as"), and a `statusFlags` string that encodes half a dozen booleans in one field.

The lazy move is to let Relay call the ERP directly and pass `TradeParty` around. It works on Tuesday. By next quarter, `partyNum` and `statusFlags` have crept into Relay's UI, its reports, its database. The ERP's accidental complexity is now Relay's permanent complexity, and the two models are welded together — you can't change one without breaking the other.

## The pattern

The anti-corruption layer is a translator at the boundary. Only one place in the whole system is allowed to know what a `TradeParty` is, and that place is the `Acl` container.

Follow the call:

1. `SalesRep.lookUp(id)` calls `Relay::Accounts.customer(id)`. `Accounts` is Relay's own clean view — it speaks only `Customer`.
2. `Accounts.customer` delegates to `Acl.customer(id)`.
3. `Acl.customer` fetches the foreign shape with `Erp.party(id)` — returning a `TradeParty` — and immediately hands it to `self.toCustomer(party)`.
4. `toCustomer(party: TradeParty): Customer` is the translation. It maps `partyNum` to `id`, derives a clean `name` from `dba`, and unpacks `statusFlags` into whatever Relay actually needs. It returns a clean `Customer`, and the `TradeParty` dies right there.

That is the whole pattern: a one-way valve. The `TranslateAtTheBoundary` feature pins the guarantee — the ACL fetches the party and maps it, *but no ERP concept leaks past the ACL into the CRM*. `Accounts`, the rest of Relay, and the `SalesRep` never see a `TradeParty`. The corruption is quarantined to a single container.

## When to use it

Reach for an ACL whenever your clean model must integrate with a system whose model you don't control and don't want — legacy systems, third-party APIs, another team's service with a different ubiquitous language. It is essential when the foreign model is genuinely awkward (encoded flags, cryptic IDs, missing concepts) and you want your own domain to stay coherent. It also pairs naturally with a strangler migration, shielding the new core from the old one's shape.

## When to avoid it

Skip it when the two models already agree — translating `Customer` to an identical `Customer` is pure ceremony. Skip it for a throwaway script or a one-off import where the foreign model never touches your core. And be wary of building an ACL around a system you actually own and could just fix at the source; sometimes the right move is to change the upstream model, not wrap it.

## Trade-offs

The ACL buys model integrity at the cost of an extra hop and a mapping you must maintain. Every field the ERP adds or renames is a change to `toCustomer`. The translation can lose information, and the boundary adds latency. But the alternative — letting `TradeParty` and `statusFlags` colonise Relay — is a debt that compounds forever. The ACL keeps the blast radius the size of one container.
