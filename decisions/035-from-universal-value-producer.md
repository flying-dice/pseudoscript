# ADR-035 — `from` is the universal typed value-producer

**Status:** Accepted
**Supersedes:** ADR-020 (return-type & `from`-target checking), ADR-027 (bindings state their type)
**Amends:** ADR-003 (no value construction), ADR-021 (array `from`), ADR-022 (inference-based checks), ADR-032 (fieldless-variant FQN)
**Affects:** LANG.md §5.1, §7.1, §7.2, §10

## Context

`return mod::Dog` against a `: mod::Cat` declaration passed clean. The cause is structural. A value's type was inferred only for literals, markers, and `from` (ADR-020, ADR-022); a call, a binding fed by a call, and any `::` path stayed `Unknown`, and an `Unknown` is never flagged. A binding bridged the gap by stating its type in an annotation (ADR-027) — but the annotation duplicated the type a `from` already names, and `from` itself produced only a `data` record or variant (§7.2), so a binding holding a `Result`, an `Option`, or a primitive could be typed only by the annotation, never by `from`. Two ways to type a value, each covering what the other could not, and neither reaching a returned `::` path.

## Decision

`from` is the single typed value-producer. It carries a type onto a value, and it is the only way a binding states one.

- A `from` target MAY be any type — a `data` record, a union variant, `Result<…>`, `Option<…>`, a primitive, or an array `T[]` — except a node and `void`. A node target MUST be rejected.
- `from` takes a brace source set or a single expression:
  - `T from { a, b }` composes a `data` record or union variant from a source set; the target MUST be a `data` record or variant. The sources are provenance and are not type-checked.
  - `T from expr` carries the type `T` onto the value `expr`. Where `expr`'s type is determinable it MUST satisfy `T`; a mismatch MUST be rejected.
- A determinable source includes a **call to a resolvable callable** — its declared return type. `Result`/`Option` match at the constructor (`Result` satisfies `Result`); inner type arguments are not compared. A source whose type is not determinable is not checked.
- A binding states its type only through `from`: `x = T from expr`. The `x: Type = Expr` annotation form is removed. A binding whose right-hand side is not a `from` is `Unknown`-typed; its downstream uses are not checked.
- A value-position reference resolving to a `data` record or a node is not a value and MUST be rejected. A fieldless union variant (`module::Union::Variant`) stays a value.

## Consequences

- §7.1: a binding is `x = Expr`; its type comes from a `from` right-hand side.
- §7.2: the target rule widens to any non-node type; the `T from expr` form and its checked conversion are added; the brace form stays `data`-only composition.
- §5.1: the return-type clause reads through `from` and determinable sources; a bare `data`/node FQN in value position is rejected.
- §10: `Assign = Ident "=" Expr ;` and `FromExpr = Path [ "[]" ] "from" ( "{" [ Expr { "," Expr } ] "}" | Expr ) ;`. `ExprKind::From.ty` widens from a `Path` to a `Type` to carry `Result<…>` generics.
- ADR-021 stands: `T[] from { … }` composes an array; `T[] from expr` is its single-source form.
- ADR-003 stands amended: `from` records provenance for every type, not only `data`; it still does not construct.
- ADR-022's `infer` now reads a resolvable call's declared return type; the condition and argument-type checks (ADR-023) ride on the same extension.
- ADR-032 stands amended: a record-variant or `data` FQN is no longer a bare value — `from` produces it; a fieldless variant referenced through its union stays a value.
- The annotated bindings across the worked model, the samples, and conformance migrate `x: T = e` → `x = T from e`.
- Rejected alternative: keep annotations and bless `from` as preferred — two ways to type one value, the redundancy ADR-027 already carried.
- Rejected alternative: permissive `T from expr` (provenance only, no source check) — `Cat from MakeDog()` would pass, the mismatch this resolves.
- Rejected alternative: check a `from` source structurally against the target's fields — beyond the shape-hints stance (§1); only the source's type, where determinable, is compared.
