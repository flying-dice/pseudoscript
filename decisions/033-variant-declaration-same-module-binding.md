# ADR-033 — A union variant binds a same-module `data` (a declaration-site binding, not an ADR-030 reference)

**Status:** Accepted
**Affects:** LANG.md §3.5, §10

## Context

§3.5 described a bare `| Name` variant as *referencing* a same-module `data Name`. ADR-030 requires every reference to a node, type, or variant to be its FQN. Read together, a variant declaration looked like a reference that must be qualified — yet the grammar `Variant = Ident [ Record ]` (§10) admits only a bare identifier, so `| other::Name` is a parse error. The two readings cannot both hold:

```pds
public data ModNotFound { id: string }

public data Errors =
  | ModNotFound        // §3.5 calls this a "reference" — ADR-030 then wants `webapp::ModNotFound`
  | Timeout            // but the grammar admits only a bare `Ident`
```

A reviewer fed the bare form, the qualified form, and a cross-module variant; only the bare same-module form parses. The spec prose oversold the variant declaration as an ADR-030 reference.

## Decision

A union variant names a `data` in the **same module** as the union. The bare name at the variant's declaration is a **declaration-site binding** — it introduces or binds a variant within the union's scope — not a use-site reference across scopes. ADR-030 governs use-site references (§8.1); it does not reach the variant declaration. A variant therefore stays bare and same-module; a qualified variant declaration (`| other::Name`) MUST be rejected.

A cross-module type an author wants in a union is composed as a **record field** of a same-module variant, not pulled in as a variant directly.

## Consequences

- §3.5 states the binding is same-module and bare, and scopes the FQN rule (ADR-030) to use-site references — the variant declaration is excluded.
- The grammar `Variant = Ident [ Record ]` (§10) is unchanged: a variant is bare by construction.
- Rejected alternative: extend `Variant = Path [ Record ]` to admit `| other::Name`, composing a cross-module `data` as a variant. It would broaden union composition across module boundaries, but force cross-module variant resolution, hoisting, and visibility rules — a foreign record variant would have to hoist into *this* module's type namespace (§3.5) or carry the other module's visibility (§8.2). The same-module rule keeps a union's variants resolvable within one file, and the record-field path already expresses cross-module composition without those questions.
