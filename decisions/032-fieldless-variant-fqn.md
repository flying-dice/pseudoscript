# ADR-032 — A fieldless union variant is referenced through its union

**Status:** Accepted
**Affects:** LANG.md §3.5, §8.1

## Context

ADR-030 requires every reference to a node, type, or union variant to be its FQN. §3.5 gives a *record* variant (`| Name { … }`) an FQN: it hoists to the module type namespace, addressed `module::Name`, the same as a top-level `data`. A *fieldless* variant (`| Name`) does not hoist, and §3.5 permits a fieldless variant's name to repeat across unions and to coincide with a node name — so no `module::Name` form can name it unambiguously. §3.5 and §8.1 left the fieldless variant with no spelled-out FQN, despite ADR-030 demanding one.

A daemon's fieldless error variant exposed it:

```pds
data DropzoneModsDirError =
  | DropzoneModsDirNotConfigured
  | DropzoneModsDirMissing
```

`Err(DropzoneModsDirNotConfigured)` is a bare reference, rejected by ADR-030. `Err(daemon::DropzoneModsDirNotConfigured)` reads as a module-level symbol the variant does not hoist into. Neither matched a defined form.

## Decision

A union variant's FQN form follows whether it hoists:

- A **record variant** hoists, and is referenced `module::Name` (§3.5, unchanged).
- A **fieldless variant** is referenced through its union: `module::Union::Variant`. The union is the variant's scope, so the union name qualifies it.

For the example, the reference is `daemon::DropzoneModsDirError::DropzoneModsDirNotConfigured`.

This is the only form consistent with §3.5: a fieldless variant has no module-level symbol (it does not hoist) and its name may repeat across unions, so the union is the sole namespace that names it without ambiguity.

## Consequences

- LANG.md §3.5 states the `module::Union::Variant` reference form; §8.1 lists it alongside the node/type/record-variant FQN forms.
- The checker resolves a value-position variant reference (the operand of `Ok`/`Err`/`from`, a `return` value) against the union: a fieldless `module::Union::Variant` MUST name a fieldless variant of that union; a record `module::Variant` resolves as the hoisted `data`. A reference naming no such variant, an unknown union, or the wrong module MUST be rejected. Previously such a reference was unchecked.
- The model indexes each union's fieldless variant names (they do not hoist to the symbol table), so the resolution above can run.
- Rejected alternative: hoist fieldless variants too, giving them `module::Variant`. It would let the daemon write `daemon::DropzoneModsDirNotConfigured`, but §3.5 lets a fieldless name repeat across unions and coincide with a node — hoisting forces those names to be unique in the module type namespace, narrowing the language to spell one reference more shortly. The union qualifier costs nothing the author cannot already see and keeps the repeat-across-unions allowance.
