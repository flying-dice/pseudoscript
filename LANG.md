# PseudoScript — Language Specification (LANG.md)

**Version:** 0.6 (Draft)
**Status:** Working draft

---

## 1. Overview

PseudoScript is an architecture-modeling language where the model *is* the source: a C4-level model written as high-level pseudocode, compiled to SVG diagrams.

Principles:
- Architecture is code — versionable, diffable, reviewable.
- **Flat structure**: containers and components declare their parent with `for`; nothing is physically nested except behavior inside its owner.
- **Behavior lives with its owner** and never changes ownership.
- **Progressive disclosure**: any `system`, `container`, `component`, `data` type, or callable MAY disclose internals with a block, or stay a black box with `;`. Sketch the architecture as signatures, then fill in only the flows worth tracing.
- **High-level**: bodies describe *flow and provenance*, not field-level computation.
- **Fallibility and absence live in the type** (`Result`, `Option`), handled explicitly with `if`.
- **Fully-qualified names everywhere**.
- **Tags** are additive visual labels (in docs); **macros** are active annotations (on declarations); **modifiers** are behavior- or styling-altering keywords.

---

## 2. Lexical Structure

### 2.1 Comments
Four comment forms; two are documentation.
```pds
// line comment        (discarded)
/* block comment */    (discarded)
/// doc comment        (attached to the following construct)
//! inner doc comment  (documents the enclosing module/file)
```
- `///` attaches to the construct that follows; consecutive lines concatenate.
- `//!` documents the enclosing module.
- A blank `///` line (marker, no text) splits the block: everything before is the summary (compact diagrams), everything after is the extended description (tooltips). No blank line means summary-only.
- Declaration order: doc block (prose + tags) → macros → modifiers → declaration.

### 2.2 Identifiers, Paths & Names
Every cross-reference is a **fully-qualified name (FQN)**, derived from the file system (§8).

- Identifiers: letter or `_`, then letters, digits, `_`. Case-sensitive (`Banking` ≠ `banking`); PascalCase nodes and lowercase locals are convention, not enforced.
- `::` separates an FQN's segments — the module path, then the node or type name (§8.1): `banking::core::Ledger` is the node `Ledger` in module `banking::core`.
- `.` invokes a method on, or reads a field of, a resolved node/value: `Repository.store(x)`, `r.value`. The member MUST exist on the receiver's type where it resolves (ADR-022). Chains freely: `Repo.fetch(id).value.owner` (§7).

### 2.3 Keywords
```
system  container  component  person
data    for        from
public  self
return  Ok    Err   Some  None
if      else  while  in
true    false
feature given when   then   and    but
```
Also reserved (MUST NOT be used as identifiers): the primitive type names (§3.1), `Result`, and `Option`.

### 2.4 Annotations: Tags, Macros, Modifiers
Three annotation kinds, distinct in syntax and effect.

| | Tag | Macro | Modifier |
|---|-----|-------|----------|
| Syntax | `#name` | `#[name]` / `#[name(args)]` / `#[name = val]` | keyword |
| Position | inside `///` docs | bare line on the declaration | before the construct keyword |
| Behavior | passive label | active: a trigger / entry point | behavior/styling-altering |
| Examples | `#critical`, `#deprecated` | `#[onevent(...)]` | `public` |

**Tags** — additive descriptive/visual labels, living in the doc block.
- `#name` (no `[`) on a `///` line is a tag; `#` elsewhere is literal prose.
- A tag-only block (`/// #critical`) is legal.
- Tags attach to the **declaration** their `///` block precedes. `//!` module docs carry no tags.
```pds
/// Durable store of account records.
/// #critical
public container AccountStore for banking::core::Banking;
```

**Macros** — Rust-style outer attributes on the declaration (a trimmed Rust `MetaItem`).
- Three forms: **word** `#[manual]`, **list** `#[onevent(Path)]`, **name = value** `#[schedule = "0 3 * * *"]`.
- They **stack** — one `#[..]` per line, order-independent.
- The namespace is **system-only and closed**: no user-defined macros; an unknown macro MUST be rejected.
```pds
#[http("POST /accounts")]
#[onevent(banking::core::OpenRequested)]
OpenAccount(req: OpenRequest): Result<BankingInfo, OpenError> { }
```

Each macro declares the declaration kind(s) it may attach to; a macro on a kind outside its target set MUST be rejected. The current built-in set is four **triggers** — each declares *how a callable is initiated* and marks it a sequence-diagram **entry point** with an inbound edge from its initiator, and each targets **callables**:

| Macro | Target | Initiated by | Wiring |
|-------|--------|--------------|--------|
| `#[onevent(Event)]` | callable | an event of `data` type `Event` | inbound edge from an event source |
| `#[schedule = "cron"]` | callable | a timer | renders a scheduler actor |
| `#[http("VERB /path")]` | callable | an HTTP request | endpoint + inbound edge from a client |
| `#[manual]` | callable | a person or external caller | inbound edge from the initiator |

- Each macro is an independent trigger; a callable with two triggers is reachable two ways.
- Repeats are allowed where they carry distinct data (e.g. two `#[http]` routes).
- `#[onevent(Event)]` requires the handler to have exactly one parameter, whose type MUST equal `Event`; two different event types on a one-parameter handler MUST be rejected.

**Modifiers** — structural keywords before the construct. `public` is the only one (§4.1, §8.2).

### 2.5 Terminators
- **Optional block** (`{ }` discloses internals, `;` is a black box): `system`, `container`, `component`, `person`, `data` (record form), and **callables**.
- **Trailing `;`**: the black-box form of any construct above.

---

## 3. Types

### 3.1 Primitives
```
number  string  bool  datetime  uuid  void
```

### 3.2 Built-in generics
```
Result<T, E>     // fallible result:  Ok(T) | Err(E)
Option<T>        // optional value:   Some(T) | None
```
`Result<T, E>` and `Option<T>` are the built-in generics.

### 3.3 Type expressions
A named type (`BankingInfo`), generic (`Result<BankingInfo, NotFound>`, `Option<Person>`), or array (`T[]`). `[]` is the only type suffix; an absent value is modeled with `Option<T>` (§6).

Every named type — a field, parameter, or return type, and each generic argument — MUST resolve to a primitive (§3.1), `Result`/`Option` (§3.2), or a declared type or node (§3.4, §3.5, §4); an unresolved type MUST be rejected (ADR-022). A reference to a declared type or node MUST be its FQN (§8.1), including one in the same module; only primitives, `Result`/`Option`, and `self` are bare.

### 3.4 Data declarations
A `data` type models any payload — DTOs, entities, messages alike. It MAY stay a **black box** with `;` (fields not yet disclosed).
```pds
/// Snapshot of an account's banking information.
data BankingInfo {
  id: number
  accountNo: string
  balance: number
  owner: Person
  tags: string[]
}

/// Shape to be detailed later.
data AccountSnapshot;
```

### 3.5 Unions (discriminated)
A variant is either a record (its own `data` type) or fieldless.
```pds
data BankAccCreated { accId: string }    // standalone
data AccountEvent =
  | BankAccCreated                       // binds an existing same-module data
  | BankAccClosed { accId: string, reason: string }   // declares + hoists to the module

data Severity =          // fieldless variants — an enum
  | Error
  | Warning
  | Info
```
- `| Name { ... }` (record variant) declares the variant and hoists it to the module's type namespace, addressed `module::Name` — the same as a top-level `data`. Its name MUST be unique among module-level type names (§8.1); a collision MUST be rejected.
- Bare `| Name` (no record): if a module-level `data Name` exists, the variant binds that same-module type; otherwise it declares a fieldless variant scoped to the union. A fieldless variant does not hoist.
- A variant's `data` MUST be in the same module as the union. The bare name is a declaration-site binding, not a use-site reference, so the FQN rule (§8.1, ADR-030) does not govern it (ADR-033); a qualified variant (`| other::Name`) MUST be rejected. A cross-module type is composed as a record field, not a variant.
- A fieldless variant's name MAY repeat across unions and MAY coincide with a node name (§8.1).
- A record variant is referenced `module::Name`, the same FQN as a top-level `data` (§8.1, ADR-030). A fieldless variant has no module-level form; it is referenced through its union, `module::Union::Variant` (ADR-032). A reference naming no such variant MUST be rejected.

---

## 4. Structural Constructs (flat)

`system` is top-level. `container` and `component` name their parent with `for` (an FQN, §8) — they are not physically nested. A `container`'s parent MUST be a `system`; a `component`'s parent MUST be a `container`; any other parent kind MUST be rejected. Each construct MAY **disclose** behavior with a block, or be a **black box** with `;`.

```
system Banking
  ← container Mainframe     for Banking
  ← container AccountStore  for Banking
      ← component AccountService for Mainframe
```

```pds
/// A retail banking customer.
public person Customer;                        // external actor, black box

public person Buyer {                          // a person MAY own behavior it initiates
  MakePurchase(item: Sku): Result<Receipt, PurchaseError>;
}

public system Banking;                         // black box

public container Mainframe for banking::core::Banking { }     // disclosed (behaviors per §5)
public container AccountStore for banking::core::Banking;     // black box

component AccountService for banking::core::Mainframe { }     // disclosed
component Repository for banking::core::AccountStore;         // black box
```

- A `person` is an external actor. It MAY own callables modeling actions it initiates (e.g. `MakePurchase`), or stay a black box (`;`).
- A container MAY hold behaviors directly or delegate to components; components are optional granularity.

### 4.1 Modifiers
`public` is the only modifier and precedes the construct keyword. **`public` means cross-module addressable** (§8.2); a node without it is reachable only within its own file.
```pds
public container Ledger for banking::core::Banking { ... }
```

---

## 5. Behavioral Members

Callables are declared **inside** the disclosing `system`/`container`/`component`/`person`. Ownership is positional and fixed.

### 5.1 Callables (implicit operations)
A function-shaped declaration is a callable.
- It MAY **disclose** its logic with a block, or be a **black box** with `;`.
- All calls are request/response. A call to a resolvable callable MUST pass one argument per declared parameter, and each inferable argument MUST match its parameter's type; a wrong arity or argument type MUST be rejected (ADR-022, ADR-023).
- Return type is optional; absence means `void`. A disclosed non-`void` callable MUST return a value on every path.
- A `return` operand whose type is determinable — a literal, an `Ok`/`Err`/`Some`/`None` marker (§6), a `from` (§7.2), a typed binding, or a call to a resolvable callable — MUST match the declared return type; a mismatch MUST be rejected. A union variant satisfies its union type (§3.5). A bare reference resolving to a `data` record or a node is not a value and MUST be rejected (§7.2).
- A bare name in a body MUST resolve to a parameter, a binding, or a `for` binding; it MUST NOT resolve to a node or union variant (ADR-030) — those are referenced by FQN (§8.1). An unresolved bare name MUST be rejected (ADR-022).
- A same-node callable is invoked via `self.Name(args)` (`self` = the enclosing node); this also covers recursion.
- A callable's name and its parameter names MUST NOT be reserved words (§2.3) — `container`, `component`, `data`, and `for` are reserved.
- A call statement MAY ignore its `Result` (the call still renders as a message).
- A black-box callable shows in C4 as a capability; a call to it in a sequence diagram is a single message with no expansion.
```pds
component AccountService for banking::core::Mainframe {

  // disclosed
  GetBankingInfo(id: number): Result<BankingInfo, NotFound> {
    r = Result<BankingInfo, NotFound> from banking::core::Repository.fetch(id)
    if (r.isErr) {
      return Err(r.error)
    }
    return Ok(r.value)
  }

  // black box — signature only
  Reconcile(): Result<void, ReconcileError>;
}
```

### 5.2 Features (BDD scenarios)
A `feature` is a top-level construct documenting one behavioral scenario of a node in given/when/then form.

- `feature Name for Path` names the node the scenario is about. `Path` is an FQN (§8) resolving to a node — `system`, `container`, `component`, or `person`. A `Path` resolving to a type or module MUST be rejected. A cross-module target MUST be `public` (§8.2).
- Each step is a step keyword followed by a string literal describing the step. The string is prose; it MUST NOT be resolved against the model.
- The flow is strict: zero or more `given` steps, then one or more `when` steps, then one or more `then` steps, in that order. A `then` before any `when`, or a `when` after any `then`, MUST be rejected.
- `and` and `but` continue the preceding step's kind. A leading `and` or `but` (no preceding step) MUST be rejected.
- A feature carries no behavior and takes no modifier; `public` MUST NOT precede it.
- A feature name occupies the module's feature namespace (§8.1); it MUST be unique among the module's features.
- A feature MAY carry a `///` doc block (summary + tags); macros MUST NOT attach to it.

```pds
/// A verified owner opens an account.
feature OpenAccount for banking::core::Mainframe {
  given "a verified owner"
  and   "no existing account for that owner"
  when  "the owner opens an account"
  then  "banking info is returned"
  and   "the account is durably stored"
}
```

---

## 6. Errors & Optional Values

Fallibility and absence live in the **type**, and every branch is explicit. The two built-in generics (§3.2) are inspected by their accessors and narrowed by `if`.

### 6.1 Result
- `Result<T, E>` — `Ok(v)` / `Err(e)`. Access: `r.isOk`, `r.isErr`, `r.value` (the `T`), `r.error` (the `E`).
- Accessing `.value` on an `Err`, or `.error` on an `Ok`, is a model error. The checker MUST report it; it tracks which branch you are in after an `if (r.isErr)` / `if (r.isOk)`.

```pds
OpenAccount(req: OpenRequest): Result<BankingInfo, OpenError> {
  check = Result<BankingInfo, OpenError> from banking::core::Verifier.check(req.owner)
  if (check.isErr) {
    return Err(check.error)
  }
  acc = Result<BankingInfo, OpenError> from banking::core::Repository.create(req)
  if (acc.isErr) {
    return Err(acc.error)
  }
  return Ok(acc.value)
}
```

### 6.2 Option
- `Option<T>` — `Some(v)` / `None`. Access: `o.isSome`, `o.isNone`, `o.value` (the `T`). `Option` has no `.error`.
- Accessing `.value` on a `None` is a model error. The checker MUST report it; it tracks which branch you are in after an `if (o.isNone)` / `if (o.isSome)`.

```pds
FindOwner(id: number): Option<Person> {
  o = Option<Person> from banking::core::Directory.lookup(id)
  if (o.isNone) {
    return None
  }
  return Some(o.value)
}
```

`Ok`, `Err`, `Some`, and `None` construct the built-in generics (§7.2): `Ok(v)` / `Some(v)` wrap a `T`, `Err(e)` wraps the error, `None` carries nothing.

---

## 7. Statements & Control Flow

Valid inside callable bodies. Each maps to a sequence-diagram element.

| Construct | Syntax | Sequence mapping |
|-----------|--------|------------------|
| Assignment | `x = Expr` | — (binds the name; single-assignment) |
| Call | `Target.method(args)` | solid request → return arrow |
| Composition | `x = Type from { a, b }` | — (local; provenance edge in data-flow view) |
| Return | `return Expr` | return arrow (`Err` labeled with `E`) |
| If | `if (C) { } else { }` | `alt` frame |
| For | `for (x in Expr) { }` | `loop` frame |
| While | `while (C) { }` | `loop` frame |

An `if`/`while` condition `C` MUST be `bool` where its type is inferable (ADR-023).

### 7.1 Assignment
`x = Expr` binds `x` once. A binding states its type through a `from` right-hand side (§7.2): `x = Type from Expr`. A binding whose right-hand side is not a `from` is `Unknown`-typed, and its later uses are not checked. Bindings are immutable: re-binding a name MUST be rejected, including by an inner `if`/`for`/`while` block (no shadowing).

### 7.2 Composition and conversion with `from`
`from` carries a type onto a value. It takes a brace **source set** or a single expression.
```pds
a = Thing from Foo.getThing()
b = Other from Bar.getOther()
c = BankingInfo from { a, b }
```
- `Type from { … }` composes a `data` record or union variant from a source set (bindings, field accesses, or calls). The target MUST be a `data` record or union variant. The sources are provenance and are not type-checked.
- `Type from Expr` carries the type `Type` onto the value `Expr`. Where `Expr`'s type is determinable — a literal, an `Ok`/`Err`/`Some`/`None` marker (§6), a `from`, a typed binding, or a call to a resolvable callable — it MUST satisfy `Type`; a mismatch MUST be rejected. A source whose type is not determinable is not checked. `Result`/`Option` match at the constructor; inner type arguments are not compared.
- A `from` target MAY be any type except a node and `void`: a `data` record, a union variant, `Result<…>`, `Option<…>`, a primitive, or an array `T[]`. A node target MUST be rejected.
- `Type from …` produces a single value; `Type[] from …` produces an array `T[]`. A singular `from` MUST NOT satisfy an array type, and an array `from` MUST NOT satisfy a singular type.
- A value-position reference resolving to a `data` record or a node is not a value; `from` produces a `data` value. A fieldless union variant (`module::Union::Variant`, §3.5) is a value.
- The produced value is usable wherever a value of `Type` is expected: a call argument, a `return` operand, or a binding.
- Model/C4: records a derivation relationship (data-flow edge).
- Sequence diagram: the calls producing the sources are the messages; the composition itself is local.

### 7.3 Loops
`for (x in Expr)` iterates an array: `Expr` MUST be an array type `T[]`, and `x` is bound to `T` per iteration. Iterating a non-array MUST be rejected.

### 7.4 Brace disambiguation
A `{` opens a **source set** only when it directly follows `from`. Everywhere else `{` opens a **block**. Because control-flow conditions are paren-delimited, the `{` after an `if`/`while`/`for` header always begins that statement's block.

---

## 8. Modules, Names & Visibility

### 8.1 Project root & module paths
A `pds.toml` at the project root anchors the file system: it is the single root from which every name is addressable. Every `.pds` file is a module, addressed by its path relative to `pds.toml`: separators become `::`, and the **filename is a path segment**. §9.3 defines `pds doc` and the `[doc]` table.
```
pds.toml
banking/
  core.pds      → module  banking::core
  events.pds    → module  banking::events
platforms/
  legacy.pds    → module  platforms::legacy
web-ide/
  file-tree.pds → module  web_ide::file_tree
```
A node declared in `banking/core.pds` is addressed `banking::core::NodeName`. A module's identity is its file path alone; a `//!` inner doc documents the module (§2.1) but MUST NOT determine its name.

Each path segment becomes an FQN segment, which MUST be an identifier (§2.2). A hyphen in a directory or filename normalises to `_` so a kebab-case path yields a valid root (ADR-031): `web-ide/file-tree.pds` is the module `web_ide::file_tree`. A dependency name carries no such normalisation — it MUST already be a valid identifier (§8.3).

A module has three distinct namespaces: **type names** (`data` declarations and hoisted record variants, §3.5), **node names** (`system`/`container`/`component`/`person`), and **feature names** (§5.2). A name MUST be unique within its namespace; the three do not collide — a `data`, a `container`, and a `feature` MAY share a name. Callable and parameter names are scoped to their owner, not the module.

Every reference to a node, type, or union variant MUST be its FQN, including a reference to one declared in the same module (ADR-030). A bare leaf name MUST NOT resolve to a node, type, or variant; it resolves only to a parameter, a binding, or a `for` binding (§7). `self` and member access (§7.1) are unaffected, as are the primitives (§3.1) and `Result`/`Option` (§3.2). Within `banking/core.pds`, a sibling node is addressed `banking::core::Other`, never `Other`. An FQN names a node by its module path and name only; the system→container→component nesting (§4) is carried by `for`, not the name. A structural drill — a node addressed through its C4 ancestry, `Container::Component` or `module::System::Container::Component` — is not an FQN and MUST NOT resolve (ADR-036).

An FQN's first segment is a **root**. The file-derived module paths above are the local roots; a `[dependencies]` entry (§8.3) adds one root per declared dependency.

### 8.2 Visibility
All declarations are module-private by default. **`public` means cross-module addressable**; a private node is reachable only within its own file, even by FQN. Applies to `data`, `person`, `system`, `container`, `component`, and callables.
```pds
public container Mainframe for banking::core::Banking {
  public GetBankingInfo(id: number): Result<BankingInfo, NotFound>;
  internalReconcile(): void;   // private — same-file only
}
```

### 8.3 Dependencies
A `pds.toml` `[dependencies]` table declares other workspaces. Each entry is one dependency with one **source**, selected by the presence of `git`: a **git source** when `git` is set, a **local source** otherwise.
- A **git source** carries a git URL, at most one **revision selector** (`tag`, `rev`, or `branch`; default = the remote's default-branch HEAD), and an optional **`path`** — the dependency workspace's directory within its repository (default = repo root).
- A **local source** carries a **`path`** and no `git` — a filesystem path to a sibling workspace, resolved relative to the declaring `pds.toml`. A local dependency is read live from disk; it is not version-pinned and records no `pds.lock` entry (§8.4).
- A local source MUST NOT be the resolved source of a git dependency: a consumer fetching a git dependency cannot follow its local entries (ADR-026).
- An entry with neither `git` nor `path` declares no source and MUST be rejected.
- Each declared name is an **FQN root** (§8.1), scoped to the declaring workspace: `dep::module::Node` addresses the node at module path `module` within dependency `dep`. The same name MAY denote different dependencies in different workspaces. The name MUST be an identifier (§2.2); unlike a filename (§8.1), it is not normalised — a hyphenated name MUST be rejected (ADR-031).
- A cross-workspace target MUST be `public`; a private or missing target MUST be rejected (extends §8.2).
- Only **direct** dependencies are addressable. A dependency's own dependencies are resolved so it is internally well-formed, but MUST NOT be nameable from a workspace that does not declare them.
```toml
[dependencies]
banking = { git = "https://example.com/acme/banking.git", tag = "v2.1.0", path = "model" }
```

### 8.4 Resolution & lockfile
A dependency's **identity** is `(source, revision, path)`.
- Entries resolving to one identity are the same package. Entries differing in revision or path are distinct packages and MAY coexist; there is no version unification.
- `pds.lock` pins the resolved graph: one entry per package — source, resolved commit, path, and dependency edges — making resolution reproducible.
- A **local** dependency (§8.3) has no commit and no `pds.lock` entry; the resolver reads it live from disk. Its identity is its resolved path.

---

## 9. Diagram Generation

### 9.1 C4 diagrams (structure + relationships)
- **Context:** `person`, `system`, inter-system arrows.
- **Container:** one system's containers (resolved via `for`).
- **Component:** one container's components (resolved via `for`).
- Arrows from cross-boundary body calls.
- Relationships of one kind between the same ordered pair of nodes MUST collapse to a single arrow; its label lists each relationship name. Opposite-direction relationships (A→B, B→A) render as separate arrows.
- **Trigger** macros add an inbound edge from the initiator: `#[onevent]` from an event source, `#[schedule]` from a scheduler actor, `#[http]` from a client, `#[manual]` from a person/caller. Tags drive styling/filtering; `///` summaries become descriptions.
- `from` composition can render as data-flow/provenance edges in a dedicated view.

### 9.2 Sequence diagrams (bodies)
From disclosed callables per §7. A **triggered** callable (one bearing a trigger macro) is an entry point. Its trigger initiator (§9.1) is the first lifeline: it calls the entry and receives the entry's `return`. A non-triggered callable projected directly takes a `caller` initiator.

A call to a **disclosed** callee expands inline: the callee becomes the active lifeline, its body traces in place, and each of its `return`s is a return message to its caller's lifeline. A call to a **black-box** callable renders as a single message with no expansion (§5.1). A callee already in flight on the call path (direct or mutual recursion) MUST NOT re-expand; it renders as a single message.

In a chained expression, each call is its own message, emitted left-to-right; field accesses between calls are local. A `self.` call renders as a self-message.

Each lifeline head card shows the participant's C4 kind and name. A `container` or `component` participant SHOULD also show its `for` ancestry (enclosing node names, outermost first) dimmed beneath the name. Every declared participant SHOULD show its `///` summary, as on a C4 card (§9.1). A synthesised initiator carries neither.

### 9.3 Documentation site (`pds doc`)
`pds doc` generates a static documentation site from the workspace rooted at `pds.toml` (§8.1), analogous to `cargo doc`: every module and node is documented automatically, with diagrams (§9.1, §9.2) embedded on the relevant pages.

The site MUST contain:
- An **index** page: the workspace name and the C4 context diagram (persons, systems, inter-system edges).
- One page **per module** (§8.1), listing its nodes with their `///` summaries (§2.1) and tags.
- One section **per node** with its `///` description, tags, visibility, and relationships (its `for` parent, inbound and outbound edges). A `system` section embeds that system's container diagram; a `container` section embeds its component diagram.
- A **sequence** diagram for each triggered callable (§9.2), on its owning node.
- A **scenario** card for each `feature` (§5.2), rendered as its given/when/then steps on the target node's section.
- **Cross-links**: every FQN reference links to the referenced node.

The site MAY also carry **authored documentation pages**: Markdown files declared in `[[doc.sidebar]]` groups (below). Each page renders as its own page; its sidebar group sits **above** the auto-generated module tree. A page whose file cannot be read MUST be skipped, not abort generation.

`[doc]` in `pds.toml` configures the site; all keys are optional:
- `name` — site title. Defaults to the root directory name.
- `out` — output directory, relative to `pds.toml`. Defaults to `target/doc`.
- `logo` — path to a logo image, relative to `pds.toml`.
- `theme` — `light` or `dark`. Defaults to `light`.
- `format` — `html` or `md`. Defaults to `html`. `md` writes one Markdown file per page, each diagram inlined as a self-contained SVG. A `--format` flag on `pds doc` overrides this key.

Each `[[doc.sidebar]]` table is one sidebar group: a `title` and an ordered `items` array of `{ title, path }` entries. Each `path` names a Markdown file relative to `pds.toml`.

```toml
[doc]
name = "Banking Architecture"
out  = "target/doc"
logo = "media/pds-logo.svg"

[[doc.sidebar]]
title = "Getting Started"
items = [
  { title = "Introduction", path = "docs/introduction.md" },
  { title = "Installation", path = "docs/installation.md" },
]
```

### 9.4 Data entity diagrams (shapes)
A `data` symbol projects an **entity view**: a card for the focal type plus the data types its fields reference, one hop out. The focal type's card is emphasised.

A **record** renders one row per field, `name: type`. A **union** renders one row per variant. A **black box** (§3.5) renders no rows.

A field (or variant) whose rendered type resolves to another `data` type in the workspace MUST draw a reference arrow from that row to the referenced type's card. The referenced type MUST render as a peer card. Type resolution strips a trailing `[]` and any generic arguments, then matches an exact FQN, a name qualified by the declaring module, or any `data` node of that simple name. A built-in type (`string`, `number`, …) resolves to no type and draws no arrow.

### 9.5 Feature flow diagrams
A `feature` (§5.2) projects a **flow view**: its steps as connected nodes, top to bottom, in source order. The view names the target node the feature describes.

Each step node shows its keyword (`given`/`when`/`then`/`and`/`but`) and its prose. Consecutive steps are joined by a directed connector.

---

## 10. Grammar Sketch (EBNF, informal)

```ebnf
Program     = { InnerDoc } { Decl | Feature } ;

Decl        = DocBlock { Macro } { Modifier } Structural ;
Modifier    = "public" ;
Structural  = Person | System | Container | Component | Data ;

Person      = "person" Ident Body ;                 // block discloses, ';' = black box
System      = "system" Ident Body ;
Container   = "container" Ident "for" Path Body ;   // parent MUST be a system
Component   = "component" Ident "for" Path Body ;   // parent MUST be a container
Body        = "{" { BodyMember } "}" | ";" ;        // block discloses, ';' = black box

BodyMember  = DocBlock { Macro } [ "public" ] Callable ;
Callable    = Ident "(" [ Params ] ")" [ ":" Type ] ( Block | ";" ) ;
Params      = Param { "," Param } ;
Param       = Ident ":" Type ;

Feature     = DocBlock "feature" Ident "for" Path FeatureBody ; // Path MUST resolve to a node
FeatureBody = "{" { Given } When { When } Then { Then } "}" ;   // strict given* when+ then+
Given       = "given" String { Cont } ;
When        = "when"  String { Cont } ;
Then        = "then"  String { Cont } ;
Cont        = ( "and" | "but" ) String ;                        // continues the preceding step kind

Data        = "data" Ident ( Record | "=" Union | ";" ) ;   // block discloses, ';' = black box
Record      = "{" { Field } "}" ;
Union       = Variant { "|" Variant } ;
Variant     = Ident [ Record ] ;
Field       = Ident ":" Type [ "," ] ;

Type        = Named [ "[]" ] ;
Named       = Path [ "<" Type { "," Type } ">" ] | Primitive ;
Primitive   = "number" | "string" | "bool" | "datetime" | "uuid" | "void" ;

Block       = "{" { Stmt } "}" ;
Stmt        = Assign | Return | If | For | While | Postfix ;
Assign      = Ident "=" Expr ;                     // binds once (single-assignment); type via `from`
Return      = "return" [ Expr ] ;
If          = "if" "(" Expr ")" Block [ "else" Block ] ;
For         = "for" "(" Ident "in" Expr ")" Block ; // Expr MUST be an array type
While       = "while" "(" Expr ")" Block ;

Expr        = Marker | FromExpr | Postfix | Literal | Unary ;
Postfix     = Primary { "." Ident [ "(" [ Args ] ")" ] } ;   // field access / call, chained
Primary     = Ref | "(" Expr ")" ;
Marker      = ( "Ok" | "Err" | "Some" ) [ "(" Expr ")" ] | "None" ;   // built-in generic constructors
FromExpr    = Type "from" ( "{" [ Expr { "," Expr } ] "}" | Expr ) ;   // brace source set, or a single value; "[]" target composes an array
Unary       = "!" Expr ;
Args        = Expr { "," Expr } ;
Ref         = "self" | Ident | Path ;               // self or an FQN
Path        = Ident { "::" Ident } ;

Literal     = String | Number | Bool ;
Bool        = "true" | "false" ;

DocBlock    = { DocLine } ;
DocLine     = "///" { DocAtom } NEWLINE ;
DocAtom     = Tag | text ;
Tag         = "#" Ident ;
Macro       = "#[" Meta "]" ;                       // outer attribute, may stack
Meta        = Path [ "(" MetaArgs ")" ] | Path "=" Literal ;
MetaArgs    = MetaArg { "," MetaArg } ;
MetaArg     = Literal | Path | Meta ;               // nested meta allowed
InnerDoc    = "//!" text NEWLINE ;
```

---

## 11. Worked Example

```pds
//! banking::core — core account systems.

/// A retail banking customer.
public person Customer;

public data BankingInfo { id: number, balance: number }
public data OpenRequest { owner: string }
public data NotFound    { id: number }
public data OpenError   { reason: string }

public system Banking;

/// Core transaction processor.
public container Mainframe for banking::core::Banking {

  /// Fetches current banking info for an account.
  public GetBankingInfo(id: number): Result<banking::core::BankingInfo, banking::core::NotFound> {
    r = Result<banking::core::BankingInfo, banking::core::NotFound> from banking::core::Repository.fetch(id)
    if (r.isErr) {
      return Err(r.error)
    }
    return Ok(r.value)
  }

  public OpenAccount(req: banking::core::OpenRequest): Result<banking::core::BankingInfo, banking::core::OpenError>;
}

/// Durable store of account records.
/// #critical
public container AccountStore for banking::core::Banking;

component Repository for banking::core::AccountStore {
  fetch(id: number): Result<banking::core::BankingInfo, banking::core::NotFound>;
  create(req: banking::core::OpenRequest): Result<banking::core::BankingInfo, banking::core::OpenError>;
}

/// A customer opens an account through the mainframe.
feature OpenAccount for banking::core::Mainframe {
  given "a verified owner"
  when  "the owner opens an account"
  then  "banking info is returned"
}
```

---

## 12. Open Questions

1. **Macro extensibility** — fixed built-in set, or user-definable macros?
2. **Generics** — only the built-in `Result`/`Option`, or user-defined generic `data` later?
3. **Expression grammar** — conditions admit only `Ref`/call/`!Expr`; no comparison/boolean operators (`==`, `&&`). Add them for `if`/`while`?
4. **Branch-aware typing** — how far does the checker track `isOk`/`isErr` narrowing (nested ifs, early returns)?
5. **`person` parenting** — can a person belong to anything, or always top-level?
