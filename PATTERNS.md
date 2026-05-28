# PATTERNS.md

Implementation patterns for the PseudoScript compiler, mapped to the components
defined in [`pseudoscript.pds`](./pseudoscript.pds). This is a research and
decision document: for each crate it records the pattern we intend to use, the
established prior art it borrows from, and the alternatives we weighed.

## Guiding constraints

Four constraints shape every choice below; a pattern that violates one is out.

1. **WASM-safe core.** `Syntax`, `Model`, and `Emit` ship inside the browser
   build (`Wasm` container), so they must avoid native-only dependencies:
   no threads, no filesystem, no `std::time`, no C-linked crates. I/O lives at
   the edges (`Cli`, `Lsp`) behind traits.
2. **One core, many frontends.** `Cli`, `Lsp`, and `Wasm` are thin shells over
   the same `Syntax → Model → Emit` pipeline. Anything frontend-specific
   (argv, JSON-RPC, JS interop) stays out of the core.
3. **Errors are values.** Mirrors the language itself (LANG.md §6): the core
   returns `Result`, never panics on user input. Diagnostics carry spans.
4. **Incremental-friendly.** The `Lsp` re-runs the pipeline on every keystroke.
   Data structures must support cheap re-computation and partial results
   (error recovery), not just batch compilation.

The reference architecture throughout is **rust-analyzer**, which solves exactly
this shape of problem (shared core, LSP frontend, incremental, error-tolerant).

---

## Cross-cutting patterns

These apply across crates and are worth fixing before any single component.

### Index/handle types over pointers (`NodeId`, `Span`, `Symbol`)

Represent the AST and the graph as flat arenas (`Vec<T>`) addressed by newtype
indices (`struct NodeId(u32)`), not `Box`/`Rc<RefCell<…>>` trees. This is the
data-oriented pattern petgraph and rust-analyzer both use: compact, `Copy`,
cache-friendly, trivially serialisable, and — crucially — it sidesteps the
borrow-checker gymnastics that pointer-graphs cause and is WASM-friendly.

- petgraph's `Graph` is itself `Vec<Node>` + `Vec<Edge>` keyed by `NodeIndex`.
  Use `StableGraph` if we ever delete nodes (indices stay valid).

### String interning (`Symbol`)

Identifiers and FQNs repeat constantly. Intern them once into a `Symbol(u32)`
so comparison and hashing are integer ops. Use a dedicated interner crate or a
tiny `FxHashMap<&str, Symbol>` + `Vec<String>`. FQN resolution (§8) becomes
symbol-keyed map lookups instead of string compares.

### Spans everywhere

Every token, AST node, and diagnostic carries a `Span { start: u32, end: u32 }`
(byte offsets into the source). Line/column is derived lazily from a
`LineIndex` (precomputed newline offsets) only when rendering — never stored.
This keeps the hot path offset-based and makes LSP range mapping a lookup.

### Diagnostics as data, rendered at the edge

The core emits structured `Diagnostic { severity, span, code, message }`
values; it does **not** render them. Rendering is a frontend concern:

- **`Cli`** renders to the terminal with **ariadne** (or **codespan-reporting**)
  — fancy carets, multi-line labels.
- **`Lsp`** maps the same structs to LSP `Diagnostic` (range + severity).
- **`Wasm`** serialises them to JS.

This is the codespan/ariadne split: the *report model* is shared, the *renderer*
varies. (We avoid baking **miette** into the core — it's a great app-level error
framework, but it's a heavier protocol than the core needs and pulls rendering
concerns inward.)

### Incremental computation (longer-term): salsa

rust-analyzer's core trick is **salsa** — a memoising query system where
`parse(file)`, `resolve(module)`, `graph(workspace)` are queries whose results
are cached and invalidated only when their inputs change. The load-bearing
invariant is *"editing inside one function body never invalidates global derived
data."* We don't need salsa for the CLI, but the `Model` API should be shaped as
**pure functions of inputs** (`fn build(ast) -> Graph`) so we can drop salsa in
under the `Lsp` later without rewriting the core. Design for it now; adopt it
when the LSP needs sub-100ms responses.

---

## `Syntax` — lexer + parser (`pseudoscript-syntax`)

The most consequential and most WASM-sensitive crate (it's tagged `#wasm`,
`#critical`). Maps to the `Lexer` and `Parser` components.

### Lexer: `logos`

Use **logos** for the `Lexer.tokenize` step. It's a derive-macro lexer that
generates a fast DFA, is `no_std`/WASM-clean, and emits a flat token stream with
spans — exactly the shape the `lexical/` conformance layer asserts
(`KIND@line:col`). The token enum maps 1:1 to the kinds defined in
`CONFORMANCE/lexical/README.md` (`KW_SYSTEM`, `COLONCOLON`, `DOC`, `TAG`,
`HASH_LBRACKET`, …). The tricky `#`-disambiguation (tag vs. macro vs. literal,
LANG.md §2.4) is handled with logos callbacks / a tiny mode flag for doc lines.

### Parser: hand-written recursive descent producing a lossless tree

**Recommendation: hand-written recursive-descent + a lossless syntax tree
(`rowan`), with Pratt parsing for expressions.** Rationale below.

- **Why lossless (CST, not just AST)?** The `Lsp` needs to map cursor offsets to
  nodes, preserve trivia (comments, the `///` docs that LANG.md attaches to
  constructs), and survive incomplete input. rust-analyzer's **rowan** (red-green
  trees) is built for this: immutable, position-free *green* nodes that share
  structure, with *red* nodes adding absolute offsets on demand. A typed AST
  layer is then a thin accessor view over the CST. `cstree` is a viable
  alternative (interns tokens, slightly different ergonomics).
- **Why hand-written?** Error *recovery* and progressive disclosure (LANG.md
  §1 — a half-written model must still parse into something) are far easier to
  steer in recursive descent than in a generated parser, and the grammar (§10)
  is small. rust-analyzer is hand-written for the same reasons.
- **Pratt parsing** for the expression grammar (§7 / §10 `Expr`). It cleanly
  absorbs the comparison/boolean operators flagged as needed in LANG.md §13 #3
  (`==`, `&&`) when we add them, via a precedence table — no grammar surgery.

### Alternatives weighed

| Option | Verdict |
| --- | --- |
| **chumsky** (parser combinator) | Strong: built-in **error recovery**, partial ASTs, `no_std`, ~hand-written speed, pairs with logos. Best pick if we don't want to hand-roll. Tradeoff: combinator types get heavy, and steering recovery/lossless-trivia is less direct than RD. **Solid plan B.** |
| **tree-sitter** | The browser-runnable WASM build exists, gives incremental reparse for free. But its terminals are *anonymous* nodes with no mapping to our named token classes — the exact reason `CONFORMANCE/lexical/README.md` cites for not asserting against it. Would mean maintaining a grammar in a second language (JS DSL) and a CST→AST mapping. Reserve for a future "incremental reparse" optimisation, not the primary parser. |
| LALR generators (lalrpop) | Poor error recovery, generated-code WASM bloat, awkward for the doc/tag lexical quirks. Rejected. |

---

## `Model` — resolver, builder, views (`pseudoscript-model`)

Turns the AST into the one resolved `Graph` and projects views from it. Maps to
`Resolver`, `Builder`, `Views`.

### Graph representation: petgraph (or a thin index arena)

The architecture graph is a directed multigraph: nodes (`system`/`container`/
`component`/`person`/`data`/callable) and typed edges (`for`-parent, derived
call, `from`-provenance). Use **petgraph**
(`StableGraph<Node, EdgeKind>`) or a hand-rolled `Vec<Node>` + `Vec<Edge>`. Node
payloads are small structs referencing interned `Symbol`s and `Span`s.

### Name resolution: two-phase, FQN-keyed symbol table

This is the classic compiler pattern and matches LANG.md §8:

1. **Collect.** Walk all modules, derive each declaration's FQN from the file
   path (filename is a path segment, §8.1), and insert into a `FxHashMap<Fqn,
   NodeId>`. Record visibility (`public` = cross-module addressable, §8.2).
2. **Resolve.** Walk references (`::` paths, `.` access, `alias` targets, macro
   args). `alias` binds to a *node* not a module (§8.3) — a dangling or
   module-targeted alias is the diagnostic asserted in
   `CONFORMANCE/static/8-*`. Cross-module access to a non-`public` node is the
   visibility error.

Keeping these phases separate is what lets recursion/forward-references resolve
regardless of declaration order.

### Relationship derivation: the visitor pattern

`Builder` walks each disclosed callable body (the AST visitor pattern) and emits
edges: a `Target.method()` call → a derived relationship; `Type from { … }` →
provenance edges. **Trigger** macros add inbound edges from a synthesised
initiator (event source / scheduler / client / person) and mark the callable as
a sequence entry point — `#[onevent(Event)]` also resolves `Event` and checks the
handler's parameter type matches. This is where "full coverage of relationships"
lives — every C4 edge originates here so any view can be projected.

### View extraction: Strategy + graph traversal

`Views.extract(graph, view)` is a **Strategy**: each view kind is a projection.

- **C1 context / C2 container / C3 component** — filter the graph to the nodes
  at that level (resolving children via `for`) plus their boundary edges.
- **Sequence trace** — a DFS from a chosen triggered (entry-point) callable
  following call edges, emitting an ordered message list; `if`/`else` → `alt` frames,
  `while`/`for` → `loop` frames (LANG.md §7 mapping). This is the "C1 all the
  way down to a sequence diagram" requirement, and it's just a graph walk over
  the edges `Builder` already produced.

### Branch-aware accessor checking (§6)

The `r.value`-on-`Err` rule (asserted in `CONFORMANCE/static/6-*`) is
flow-sensitive. Implement as a small **typestate dataflow**: thread an
environment mapping each `Result`/`Option` binding to `Unknown | Ok | Err`, and
narrow it on entering an `if (r.isErr)` / `if (r.isOk)` branch. Cloning the
environment per branch is fine at this scale.

---

## `Emit` — pluggable backends over the graph (`pseudoscript-emit`)

The resolved (sub-)graph is the stable seam; emission is a **pluggable backend**
chosen at runtime by `--format` (`Target`). Maps to `Transpiler` (dispatch), the
custom `Svg` path (`Layout` + `SvgRenderer`), the text exporters
`Dot`/`Mermaid`/`PlantUml`, and the `#future` `SvgWorkspace`.

### Backend trait: Strategy over the graph

```rust
trait Backend { fn render(&self, view: &SubGraph) -> Result<Diagram, EmitError>; }
```

`Transpiler` holds a `Box<dyn Backend>` selected from the `Target`; every backend
consumes the **same resolved sub-graph**. Two families:

- **Text exporters (`Dot`, `Mermaid`, `PlantUml`)** — graph → notation text in a
  single pass; layout is delegated to the external tool (Graphviz `dot`,
  mermaid, plantuml). Cheap to add, and the output is deterministic text.
- **Custom `Svg` (headline)** — graph → our own layout → SVG. We own the
  geometry, so the result is self-contained (no external renderer) and runs
  client-side in WASM.

Backends are additive: a new notation is one `render(&SubGraph)` impl. `Target`
is orthogonal to `View` (which C4 level / sequence) — any view renders through
any backend.

### The custom SVG path: layout, then render

The hard part of "SVG from the graph" is **layout** — turning a node/edge set
into coordinates. Keep it separate from rendering:

1. **`Layout.solve(graph, view) -> Scene`** computes geometry. The view kind
   selects the algorithm: C4 box diagrams want a layered/hierarchical layout
   (Sugiyama-style — `rust-sugiyama` or `dagre-rs`, both built on petgraph; or
   `layout-rs` for GraphViz-style records), while the sequence view is a far
   simpler timeline (lifelines across the x-axis, messages ordered down the
   y-axis) worth hand-rolling.
2. **`SvgRenderer.render(scene) -> Diagram`** walks the positioned `Scene` and
   emits SVG — uniform across views; it just draws the shapes, edges, and labels
   the layout placed.

`Scene` is the SVG path's **notation-neutral geometry IR** (positioned boxes,
routed edges, frames, text). The interactive `SvgWorkspace` consumes the same
`Scene` the static renderer does.

### Generation: programmatic, WASM-safe

Every backend is a visitor that string-builds its output with `std::fmt::Write`
(the **Builder** pattern) — SVG (or the `svg` crate), DOT, Mermaid, and PlantUML
are all just text. No template engine, no headless browser, no native deps, so
the whole of `Emit` compiles to `wasm32` and renders client-side
(`wasm_svg_graphics` is an option for the interactive workspace specifically).

### Goldening (see `generation/README.md`)

- Text backends (`Dot`/`Mermaid`/`PlantUml`) emit **deterministic text** →
  byte-for-byte goldens are fine.
- The custom SVG path is brittle (float coords) → assert on the `Scene` IR, not
  the rendered pixels.

> Conformance note: raw SVG is brittle to golden (floating-point coordinates,
> attribute order). The `generation/` layer should assert on the **`Scene` IR**
> (deterministic — which nodes/edges/lifelines, not pixel positions); pixel-level
> SVG snapshots belong in implementation tests, not the spec contract.

---

## `Cli` — the `pds` binary (`pds`)

Maps to `Args`, `Generate`, `Check`, `Serve`. A thin frontend.

- **Arg parsing: `clap`** (derive API). Subcommands (`generate`, `check`,
  `serve`) map to the **Command pattern** — each is a handler that reads input,
  calls the core, and renders the result. This is the `Args.parse → Request`
  shape in the model.
- **I/O at the edge.** The CLI owns file reading and writing; the core receives
  a `Source { path, text }` and returns `Diagram`/`Diagnostic`s. Diagnostics
  render via **ariadne** to stderr; exit non-zero on any error diagnostic
  (matches the conformance "exit status" rule).

---

## `Lsp` — language server (`pseudoscript-lsp`)

Maps to `Server`, `Diagnostics`, `Hover`, `Definition`. Reuses `Syntax` +
`Model` wholesale — it adds protocol, not language logic.

### Framework: `tower-lsp` (or `lsp-server` for full control)

- **tower-lsp** (and the maintained **tower-lsp-server** fork) gives an async,
  Tower-based `LanguageServer` trait — fastest path to a working server,
  handles JSON-RPC framing and lifecycle. Default plan.
- **lsp-server** (rust-analyzer's own, synchronous, no async runtime) is the
  alternative when we want a hand-driven main loop and tighter control over
  scheduling — and it composes more naturally with salsa. Choose this if/when
  incremental scheduling becomes the bottleneck.

### Server patterns

- **VFS + document store.** Keep open documents in memory keyed by URI; the
  filesystem is just the initial load. `didChange` updates the in-memory text.
- **Debounce + recompute.** On `didChange`, debounce, then run
  `parse → build` and `publishDiagnostics` — exactly `Server.onChange` in the
  model. Error recovery (from the parser) is what makes diagnostics useful while
  typing.
- **Span ↔ range mapping** via the shared `LineIndex`. `Hover` and `Definition`
  resolve the offset under the cursor to a `NodeId` through `Model`'s resolver,
  then read its `///` doc / declaration span.
- **Incremental** is the long-term reason to adopt salsa here: typing in one
  module shouldn't reparse the workspace.

---

## `Wasm` — browser bindings (`pseudoscript-wasm`)

Maps to `Bindings`. The reason the core crates carry the `#wasm` constraint.

- **`wasm-bindgen` + `wasm-pack`** to compile and package the core for the
  browser. `Bindings.generate(text, view)` becomes an exported JS function.
- **`serde-wasm-bindgen`** for crossing the boundary: derive
  `Serialize`/`Deserialize` on `View`, `Diagram`, and `Diagnostic`, and convert
  with `to_value`/`from_value`. It's the officially preferred path — smaller and
  faster than the JSON bridge for structured data.
- **Hygiene:** install `console_error_panic_hook` (turn panics into readable JS
  errors), keep the core panic-free regardless, and gate any native-only code
  out of the WASM build with `#[cfg(not(target_arch = "wasm32"))]`. This is the
  enforcement point for constraint #1 — if it doesn't compile to `wasm32`, it
  doesn't belong in `Syntax`/`Model`/`Emit`.

---

## Dependency cheat-sheet

| Crate | Role | WASM-safe | Notes / alt |
| --- | --- | --- | --- |
| `logos` | lexer DFA | ✅ | matches `lexical/` token kinds |
| `rowan` | lossless syntax tree | ✅ | alt: `cstree` |
| (hand-written RD) | parser | ✅ | alt: `chumsky` |
| `petgraph` | architecture graph | ✅ | `StableGraph`; or hand arena |
| `rust-sugiyama` / `dagre-rs` | C4 layered layout | ✅ | petgraph-based; `layout-rs` alt |
| `svg` (or `fmt::Write`) | SVG generation | ✅ | `wasm_svg_graphics` for interactive |
| `rustc-hash` (`FxHashMap`) | interning, symbol tables | ✅ | fast non-crypto hash |
| `ariadne` | CLI diagnostic rendering | n/a (CLI) | alt: `codespan-reporting` |
| `clap` | CLI args | n/a (CLI) | derive API |
| `tower-lsp` | LSP framework | n/a (LSP) | alt: `lsp-server` (+ salsa) |
| `wasm-bindgen` + `wasm-pack` | JS interop / packaging | ✅ | the browser build |
| `serde-wasm-bindgen` | struct marshalling | ✅ | preferred over JSON bridge |
| `salsa` | incremental queries | ✅ | adopt when LSP needs it |

---

## Alignment with LANG.md open questions

Two patterns above are shaped by gaps the spec itself flags (LANG.md §13):

- **Expression operators (#3).** Pratt parsing in `Syntax` is chosen so that
  adding `==`/`&&` later is a precedence-table edit, not a rewrite.
- **Union narrowing / no `match` (#4 and the missing `match`).** `Model` does
  `Result`/`Option` narrowing by hand, and the view-kind dispatch is pushed
  inside the black-box `Layout.solve` rather than branched in a disclosed body —
  both because the language can't yet match on a `View` variant. When it lands,
  the layout dispatch could surface explicitly.

These are deliberate accommodations, not accidents — revisit them when the
corresponding language features are resolved.

---

Sources:
- [rust-analyzer architecture (book)](https://rust-analyzer.github.io/book/contributing/architecture.html)
- [rust-analyzer architecture.md (repo)](https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md)
- [Red-Green Syntax Trees — an Overview](https://willspeak.me/2021/11/24/red-green-syntax-trees-an-overview.html)
- [cstree crate](https://crates.io/crates/cstree)
- [chumsky (docs.rs)](https://docs.rs/chumsky/latest/chumsky/)
- [chumsky + logos example](https://github.com/zesterer/chumsky/blob/main/examples/logos.rs)
- [tower-lsp](https://github.com/ebkalderon/tower-lsp)
- [tower-lsp-server (fork)](https://github.com/tower-lsp-community/tower-lsp-server)
- [ariadne](https://docs.rs/ariadne/latest/ariadne/)
- [codespan / codespan-reporting](https://github.com/brendanzab/codespan)
- [miette](https://lib.rs/crates/miette)
- [serde-wasm-bindgen](https://docs.rs/serde-wasm-bindgen/latest/serde_wasm_bindgen/)
- [wasm-bindgen Guide — arbitrary data with Serde](https://rustwasm.github.io/docs/wasm-bindgen/reference/arbitrary-data-with-serde.html)
- [petgraph](https://docs.rs/petgraph/)
