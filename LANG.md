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
- **Errors are values** (`Result`), handled explicitly with `if`.
- **Fully-qualified names everywhere**; `alias` provides local shorthand.
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
- `::` walks the module/node path: `banking::core::Ledger`.
- `.` invokes a method on, or reads a field of, a resolved node/value: `Repository.store(x)`, `r.value`. Chains freely: `Repo.fetch(id).value.owner` (§7).
- After an `alias` name, only `.` MUST follow; `::` MUST NOT.

### 2.3 Keywords
```
system  container  component  person
data    for        alias      from
public  self
return  Ok    Err
if      else  while  in
true    false
```
Also reserved (MUST NOT be used as identifiers): the primitive type names (§3.1) and `Result`.

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
public container AccountStore for Banking;
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
- **Trailing `;`**: `alias`, and the black-box form of any construct above.

---

## 3. Types

### 3.1 Primitives
```
number  string  bool  datetime  uuid  void
```

### 3.2 Built-in generics
```
Result<T, E>     // fallible result: Ok(T) | Err(E)
```
`Result<T, E>` is the only built-in generic.

### 3.3 Type expressions
A named type (`BankingInfo`), generic (`Result<BankingInfo, NotFound>`), or array (`T[]`). There is no optionality marker.

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
Each variant is a usable `data` type.
```pds
data BankAccCreated { accId: string }    // standalone
data AccountEvent =
  | BankAccCreated                       // reference to an existing data
  | BankAccClosed { accId: string, reason: string }   // declares + hoists to the module
```
- `| Name { ... }` declares the variant and hoists it to the enclosing module namespace, addressed `module::Name` — the same as a top-level `data`.
- Bare `| Name` references an existing module-level `data Name`; a missing target MUST be rejected.
- A declared variant whose name collides with another module-level `data` MUST be rejected.

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

public container Mainframe for Banking { }     // disclosed (behaviors per §5)
public container AccountStore for Banking;     // black box

component AccountService for Mainframe { }     // disclosed
component Repository for AccountStore;         // black box
```

- A `person` is an external actor. It MAY own callables modeling actions it initiates (e.g. `MakePurchase`), or stay a black box (`;`).
- A container MAY hold behaviors directly or delegate to components; components are optional granularity.

### 4.1 Modifiers
`public` is the only modifier and precedes the construct keyword. **`public` means cross-module addressable** (§8.2); a node without it is reachable only within its own file.
```pds
public container Ledger for Banking { ... }
```

---

## 5. Behavioral Members

Callables are declared **inside** the disclosing `system`/`container`/`component`/`person`. Ownership is positional and fixed.

### 5.1 Callables (implicit operations)
A function-shaped declaration is a callable.
- It MAY **disclose** its logic with a block, or be a **black box** with `;`.
- All calls are request/response.
- Return type is optional; absence means `void`. A disclosed non-`void` callable MUST return a value on every path.
- A same-node callable is invoked via `self.Name(args)` (`self` = the enclosing node); this also covers recursion.
- A call statement MAY ignore its `Result` (the call still renders as a message).
- A black-box callable shows in C4 as a capability; a call to it in a sequence diagram is a single message with no expansion.
```pds
component AccountService for Mainframe {

  // disclosed
  GetBankingInfo(id: number): Result<BankingInfo, NotFound> {
    r = AccountStore::Repository.fetch(id)
    if (r.isErr) {
      return Err(r.error)
    }
    return Ok(r.value)
  }

  // black box — signature only
  Reconcile(): Result<void, ReconcileError>;
}
```

---

## 6. Errors

Fallibility lives in the **type**, and every error path is explicit.

- `Result<T, E>` — `Ok(v)` / `Err(e)`. Access: `r.isOk`, `r.isErr`, `r.value` (the `T`), `r.error` (the `E`).
- `Ok` and `Err` are branch markers, not constructors; nothing is instantiated.
- Accessing `.value` on an `Err`, or `.error` on an `Ok`, is a model error. The checker MUST report it; it tracks which branch you are in after an `if (r.isErr)` / `if (r.isOk)`.

```pds
OpenAccount(req: OpenRequest): Result<BankingInfo, OpenError> {
  check = KYC::Verifier.check(req.owner)
  if (check.isErr) {
    return Err(check.error)
  }
  acc = AccountStore::Repository.create(req)
  if (acc.isErr) {
    return Err(acc.error)
  }
  return Ok(acc.value)
}
```

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

### 7.1 Assignment
`x = Expr` binds `x` once. Type is inferred from the right-hand side. Bindings are immutable: re-binding a name MUST be rejected, including by an inner `if`/`for`/`while` block (no shadowing).

### 7.2 Composition with `from`
`x` of type `Type` is composed *from* a set of sources. The braces enclose a **source set** (bindings, field accesses, or calls).
```pds
a = Foo.getThing()
b = Bar.getOther()
c = BankingInfo from { a, b }
```
- Model/C4: records a derivation relationship (data-flow edge).
- Sequence diagram: the calls producing the sources are the messages; the composition itself is local.

### 7.3 Loops
`for (x in Expr)` iterates an array: `Expr` MUST be an array type `T[]`, and `x` is bound to `T` per iteration. Iterating a non-array MUST be rejected.

### 7.4 Brace disambiguation
A `{` opens a **source set** only when it directly follows `from`. Everywhere else `{` opens a **block**. Because control-flow conditions are paren-delimited, the `{` after an `if`/`while`/`for` header always begins that statement's block.

---

## 8. Modules, Names & Visibility

### 8.1 Workspace & module paths
A `workspace.toml` at the root anchors the file system. Every `.pds` file is a module, addressed by its path relative to the root: separators become `::`, and the **filename is a path segment**.
```
workspace.toml
banking/
  core.pds      → module  banking::core
  events.pds    → module  banking::events
platforms/
  legacy.pds    → module  platforms::legacy
```
A node declared in `banking/core.pds` is addressed `banking::core::NodeName`.

### 8.2 Visibility
All declarations are module-private by default. **`public` means cross-module addressable**; a private node is reachable only within its own file, even by FQN. Applies to `data`, `person`, `system`, `container`, `component`, and callables.
```pds
public container Mainframe for Banking {
  public GetBankingInfo(id: number): Result<BankingInfo, NotFound>;
  internalReconcile(): void;   // private — same-file only
}
```

### 8.3 alias
`alias` binds a local name to a **node** FQN, not a module/namespace.
- File-scoped, not exported; nothing MAY follow it via `::`.
- The target is a node addressable by `::`; a callable (reached via `.`) cannot be aliased.
- A dangling alias (target missing, or not `public` when cross-module) MUST be rejected.
```pds
alias Store = banking::core::AccountStore;       // ✓ a container node
alias Created = banking::core::BankAccCreated;   // ✓ a data node
alias Core = banking::core;                      // ✗ that's a module, not a node

Store.fetch(id)   // alias then invoke
```

---

## 9. Diagram Generation

### 9.1 C4 diagrams (structure + relationships)
- **Context:** `person`, `system`, inter-system arrows.
- **Container:** one system's containers (resolved via `for`).
- **Component:** one container's components (resolved via `for`).
- Arrows from cross-boundary body calls.
- **Trigger** macros add an inbound edge from the initiator: `#[onevent]` from an event source, `#[schedule]` from a scheduler actor, `#[http]` from a client, `#[manual]` from a person/caller. Tags drive styling/filtering; `///` summaries become descriptions.
- `from` composition can render as data-flow/provenance edges in a dedicated view.

### 9.2 Sequence diagrams (bodies)
From disclosed callables per §7. A **triggered** callable (one bearing a trigger macro) is an entry point. Black-box callables render as single messages with no expansion (§5.1). In a chained expression, each call is its own message, emitted left-to-right; field accesses between calls are local. A `self.` call renders as a self-message.

---

## 10. Grammar Sketch (EBNF, informal)

```ebnf
Program     = { InnerDoc } { Alias | Decl } ;

Alias       = "alias" Ident "=" Path ";" ;          // Path must resolve to a node

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
Assign      = Ident "=" Expr ;                      // binds once (single-assignment)
Return      = "return" [ Expr ] ;
If          = "if" "(" Expr ")" Block [ "else" Block ] ;
For         = "for" "(" Ident "in" Expr ")" Block ; // Expr MUST be an array type
While       = "while" "(" Expr ")" Block ;

Expr        = ResultMarker | FromExpr | Postfix | Literal | Unary ;
Postfix     = Primary { "." Ident [ "(" [ Args ] ")" ] } ;   // field access / call, chained
Primary     = Ref | "(" Expr ")" ;
ResultMarker= ( "Ok" | "Err" ) [ "(" Expr ")" ] ;
FromExpr    = Path "from" "{" Expr { "," Expr } "}" ;
Unary       = "!" Expr ;
Args        = Expr { "," Expr } ;
Ref         = "self" | Ident | Path ;               // self, alias name, or FQN
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
public container Mainframe for Banking {

  /// Fetches current banking info for an account.
  public GetBankingInfo(id: number): Result<BankingInfo, NotFound> {
    r = AccountStore::Repository.fetch(id)
    if (r.isErr) {
      return Err(r.error)
    }
    return Ok(r.value)
  }

  public OpenAccount(req: OpenRequest): Result<BankingInfo, OpenError>;
}

/// Durable store of account records.
/// #critical
public container AccountStore for Banking {
  component Repository for AccountStore {
    fetch(id: number): Result<BankingInfo, NotFound>;
    create(req: OpenRequest): Result<BankingInfo, OpenError>;
  }
}
```

---

## 12. Open Questions

1. **Macro extensibility** — fixed built-in set, or user-definable macros?
2. **Generics** — only built-in `Result`, or user-defined generic `data` later?
3. **Expression grammar** — conditions admit only `Ref`/call/`!Expr`; no comparison/boolean operators (`==`, `&&`). Add them for `if`/`while`?
4. **Branch-aware typing** — how far does the checker track `isOk`/`isErr` narrowing (nested ifs, early returns)?
5. **`person` parenting** — can a person belong to anything, or always top-level?
