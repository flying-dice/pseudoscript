# PATTERNS.md

Two kinds of idiom, one document:

1. **Modeling idioms** — how to write `.pds` models (the language as used).
2. **Implementation patterns** — how the compiler is built, crate by crate, mapped to the components in the [`model/`](./model/) workspace (one module per crate).

The implementation half records the pattern **in use**, the prior art it draws on, and the alternatives weighed. Where the build chose differently from an obvious default (hand-written over `logos`, an index arena over `petgraph`), the rejected option is kept as an alternative, not presented as current.

---

## Modeling idioms (writing `.pds`)

The model expresses **architecture and flow**; a body may also state a static business rule over primitives and constants (LANG.md §7.5), type-checked but never run. Reach for these when authoring a model.

### Express meaning, not the implementation

A `data` field is a shape hint, not a faithful port of the host type: a count is `number` whether the code uses `u32` or `usize`. Model a genuinely optional value with `Option<T>` (LANG.md §6) and a fallible one with `Result<T, E>`; reach for the type that conveys intent, not the host language's exact one. Bodies describe *flow and provenance* (LANG.md §1, §7); a static business rule over primitives and constants is also welcome (§7.5), but a body is never executed, so don't lean on it for computation a reader would expect to be live.

### Produce values with `from`

`from` carries a type onto a value (LANG.md §7.2): `Type from { … }` composes a record `data` or union variant from a source set; `Type from expr` carries a type onto a single value. It is how a binding states its type, and the result is usable as a call argument, a `return` operand, or a binding:

```pds
view = Sequence from { entry }                       // compose a View variant
scene = Scene from emit::Projector.project(graph, view)   // carry the type onto a call result
```

`Ok`/`Err`/`Some`/`None` construct the built-in generics (LANG.md §6): `Ok(v)`/`Some(v)` wrap a `T`, `Err(e)` wraps the error, `None` is empty. A bare `data`-record or node reference is not a value — `from` produces it.

### State a business rule with a constant threshold

```pds
public constant WITHDRAWAL_LIMIT = 10000

WithinLimit(amount: number): bool {
  return amount > 0 && amount <= banking::core::WITHDRAWAL_LIMIT
}
```

A `constant` (LANG.md §3.6) names the value the business gives a threshold; operators (LANG.md §7.5) state the rule over primitives and constants. Both are type-checked, never evaluated — the rule documents intent and renders in the condition label, it does not run. Reference a constant by its FQN, like any cross-module name (§8.1).

### Model fallibility by the operation's nature

| Operation | Return | Idiom |
| --- | --- | --- |
| genuinely fallible (I/O, projection) | `Result<T, E>` | handle with `if (x.isErr) { return Err(x.error) }` |
| optional / may be absent | `Option<T>` | handle with `if (x.isNone) { return None }`, then read `x.value` |
| total | the type directly | `parse(text): Parsed` |
| validation pass | `Diagnostic[]` | collect, don't fail — empty means well-formed |

Parsing and checking are total: they always produce a tree/graph and collect `Diagnostic`s. Don't wrap them in `Result`.

### Triggers mark entry points

A callable bearing a trigger macro (`#[manual]`, `#[http]`, `#[onevent]`, `#[schedule]`, LANG.md §2.4) is a sequence-diagram entry point and gets its own diagram on its owner's page (LANG.md §9.3). A callable with no trigger is reached only when something calls it.

### Disclose only flows worth tracing

Black-box every callable with `;` by default. Disclose a body `{ }` (LANG.md §5.1) only where the sequence is worth showing — the headline path, a cross-crate flow, the one place an error is handled. Progressive disclosure (LANG.md §1) is the point: a sketch of signatures is a valid model.

### Document scenarios with `feature`

```pds
/// A verified owner opens an account.
feature OpenAccount for banking::core::Mainframe {
  given "a verified owner"
  when  "the owner opens an account"
  then  "banking info is returned"
}
```

A `feature` records one expected behavior of a node as a prose given/when/then flow (LANG.md §5.2), surfaced as a scenario card on the node's page (LANG.md §9.3). Steps are prose — they don't resolve against the model, so reach for `feature` to state intent, not to trace a flow (disclose a body for that). The flow is strict: `given*` then `when+` then `then+`, with `and`/`but` continuing the preceding step.

### Names

Three namespaces are distinct (LANG.md §8.1): **type names** (`data` + hoisted record variants), **node names** (`system`/`container`/`component`/`person`), and **feature names** (LANG.md §5.2). Don't reuse a name across two nodes (a container and a component both `Doc` collides), or across two record variants. Fieldless variants are free — they don't hoist (LANG.md §3.5). Don't name a callable or parameter with a reserved word — `container`/`component`/`data`/`for` are reserved (LANG.md §2.3).

### Validate against the toolchain

`pds check <file>` is ground truth for well-formedness; `pds fmt <file>` is ground truth for layout. Run both — they settle questions (is a bare nullary variant legal? yes) faster than reading the spec.

---

## Implementation patterns

The reference architecture throughout is **rust-analyzer** — shared core, LSP frontend, incremental, error-tolerant.

### Guiding constraints

1. **Pure, I/O-free core.** `Syntax`, `Model`, `Emit`, and `Doc` are pure functions of their inputs and touch no filesystem, clock, or network. `Doc`'s renderer returns in-memory files; the CLI writes them. I/O lives at the edges (`Cli`, `Lsp`).
2. **One core, two frontends.** `Cli` and `Lsp` are thin shells over the same `Syntax → Model → Emit`/`Doc` pipeline. Anything frontend-specific (argv, JSON-RPC, HTTP serving) stays out of the core.
3. **Diagnostics as data; `Result` only where it fails.** The core never panics on user input. Parsing and checking always return a value plus a `Vec<Diagnostic>` (LANG.md §6 is the *language's* error model; the compiler's own is diagnostics). Only projection (`EmitError`) and formatting (`FormatError`) return `Result`.
4. **Incremental-friendly.** `Lsp` reruns the pipeline on every keystroke, so the core is shaped as pure functions (`fn graph(modules) -> Graph`) that a memoiser could wrap later.

### Cross-cutting

#### Index/handle types over pointers

The AST and graph are flat arenas (`Vec<T>`) addressed by indices, not `Box`/`Rc<RefCell<…>>` trees — `Copy`, cache-friendly, serialisable, and it sidesteps borrow-checker gymnastics. `Graph` is `Vec<GraphNode>` + `Vec<Edge>` + an FQN→index `FxHashMap` (graph.rs), the data-oriented shape rust-analyzer uses. **petgraph** is the obvious alternative; the hand arena was chosen to keep the node payloads and edge kinds domain-specific and dependency-free.

#### String keys via `rustc-hash`

FQN resolution (LANG.md §8) is map lookups keyed on owned `String`s in `FxHashMap` (`rustc-hash`), the fast non-crypto hash. Interning to a `Symbol(u32)` is the next step if profiling shows FQN compares hot; not yet needed.

#### Spans everywhere, line/col on demand

Every token, AST node, and diagnostic carries `Span { start: u32, end: u32 }` (byte offsets, span.rs). Line/column is derived lazily from `LineIndex` (precomputed newline offsets) only when rendering — the hot path stays offset-based and LSP range mapping is a lookup.

#### Diagnostics as data, rendered at the edge

The core emits structured `Diagnostic { severity, span, code, message }` — the one type every crate produces (diagnostic.rs) — and never renders. Rendering is a frontend concern:

- **`Cli`** prints `path:line:col: severity: message` to stderr and exits non-zero on any error.
- **`Lsp`** maps the same struct to an LSP `Diagnostic` (range + severity) via `convert`.

A richer CLI renderer (**ariadne**, **codespan-reporting**) is a drop-in upgrade at the `Cli` edge — the report model is already shared.

#### Incremental computation: salsa (not adopted)

rust-analyzer's core trick is **salsa** — memoised queries (`parse`, `graph`) invalidated only when inputs change. Not adopted: the CLI is batch, and the LSP rechecks whole documents fast enough. The `Model` API is kept as pure functions so salsa can slot under the `Lsp` later without a core rewrite.

---

### `Syntax` — lexer + parser (`pseudoscript-syntax`)

The most `#critical` crate. Maps to `Lexer` and `Parser`.

- **Hand-written lexer** (lexer.rs) → `Vec<Token>`, each with a `Span` and lexeme; `lex` also returns trivia (comments, blank-line runs) for the formatter. `render_tokens` emits the `KIND@line:col "lexeme"` form the `lexical/` conformance layer asserts. The `#`-disambiguation (tag vs macro vs literal, LANG.md §2.4) is a mode flag on doc lines.
- **Hand-written recursive descent** (parser.rs) → `Parsed { ast: Module, diagnostics }`. **Total and recovering**: it always yields a tree (partial on error) and never throws, because progressive disclosure (LANG.md §1) means a half-written model must still parse. The typed AST (ast.rs) is the parser's own structs, not a view over a lossless tree.

#### Alternatives weighed

| Option | Verdict |
| --- | --- |
| **logos** (DFA lexer) | Fast, `no_std`, span-carrying. Not used — the hand-written lexer keeps the `#`/doc-line modes explicit and adds no dependency. Viable swap. |
| **rowan / cstree** (lossless CST) | The rust-analyzer pattern; a typed AST is a view over position-free green nodes. Not used — the typed AST + trivia list covers the LSP and formatter needs without the red-green machinery. Reserve for if incremental reparse is needed. |
| **chumsky** (combinator) | Built-in recovery, `no_std`. Solid plan B; recovery is less direct to steer than recursive descent. |
| **tree-sitter** | Browser-runnable, incremental reparse for free, but anonymous terminals don't map to our named token classes (the reason `lexical/` cites for not asserting against it). Reserve for a future incremental optimisation. |

---

### `Model` — resolution, checks, graph (`pseudoscript-model`)

Turns parsed modules into the one resolved `Graph`, and runs the static checks. Maps to `Checks` and `Builder`. **View extraction is not here** — projection lives in `Emit`; `Model` produces the complete graph any view draws from.

#### Graph: hand-rolled index arena

`Graph` (graph.rs) is `Vec<GraphNode>` + `Vec<Edge>` + a per-callable body trace, keyed by an FQN→index map. Node kinds: `person`/`system`/`container`/`component`/`data`/`callable`. Edge kinds: `ForParent`, `Call`, `Trigger`, `Provenance`. **petgraph** (`StableGraph<Node, EdgeKind>`) is the weighed alternative.

#### Name resolution: two-phase, FQN-keyed

Classic compiler shape, matching LANG.md §8:

1. **Collect** — walk modules, derive each declaration's FQN from the file path (filename is a segment, §8.1), record visibility into an FQN→node map.
2. **Resolve** — walk references (`::` paths, `.` access, macro args). Separating the phases is what lets forward references resolve regardless of order.

#### Edge derivation: AST visitor

`Builder` walks each disclosed body and emits edges: a `Target.method()` call → `Call`; `Type from { … }` → `Provenance`; a trigger macro → a `Trigger` edge from a synthesised initiator (`caller`/`client`/`scheduler`/`event:<FQN>`) and marks the callable a sequence entry. Every C4 edge originates here, so any view projects from one graph.

#### Checks (`check/`)

- **`cross_module`** — LANG.md §8.2: a cross-module reference resolves only to a `public` node; private access and dangling FQNs are diagnostics.
- **`result_flow`** — LANG.md §6: a typestate dataflow threading each `Result` binding as `Unknown | Ok | Err`, narrowed on entering `if (r.isErr)` / `if (r.isOk)`, flagging wrong-branch `.value`/`.error` reads.

Both return `Vec<Diagnostic>`. `check`/`analyze`/`check_workspace` are the entry points; `graph` is the diagnostic-free projection `Emit` and `Doc` consume.

---

### `Emit` — view projection + SVG (`pseudoscript-emit`)

Projects a `View` out of the graph into a `Scene` and renders it to SVG. **SVG is the only backend (ADR-017).** Maps to `Projector`, `Layout`, `SvgRenderer`.

#### Project, then render

```
project(graph, view) -> Result<Scene, EmitError>      // select nodes/edges + lay out
render_svg(scene)     -> String                       // draw the positioned scene
```

`project` (project.rs) dispatches on the `View` value the caller built with `from`: `Context`/`Container`/`Component` → a `C4Scene`, `Sequence` → a `SequenceScene`. It fails with `EmitError` (`UnknownNode`, `WrongKind`, `NoBody`) when the target is missing or wrong-kind.

`Scene` (scene.rs) is the **notation-neutral geometry IR** — placed nodes and routed edges, or lifelines and ordered messages. It is the `generation/` conformance surface (ADR-017): deterministic structure, no pixel coordinates in the golden form.

#### Layout

- **C4 views** — the **`layout-rs`** Sugiyama placer assigns box positions and routes edges (c4_render.rs), with a panic-safe fallback.
- **Sequence view** — a hand-rolled timeline: lifelines across x, messages down y, `if`/`else` → `alt` frames, `for`/`while` → `loop` frames (LANG.md §7).

#### Generation: programmatic SVG

`render_svg` string-builds with `std::fmt::Write` — no template engine, no headless browser. Notation exporters (DOT/Mermaid/PlantUML) were weighed and **rejected** (ADR-017): the stable seam is the `Scene` IR, not a `Box<dyn Backend>`. A new notation would consume `Scene`, not re-walk the graph.

---

### `Doc` — Svelte-rendered documentation site (`pseudoscript-doc`)

The headline subsystem (ADR-017, ADR-025). `try_render_site_with(graph, config, engine) -> Result<Site, RenderError>` projects the resolved graph into per-page props, server-renders each through an SSR engine, and wraps it in the document shell. Maps to `SiteBuilder`, `Pages`, `Ssr`, `Shell`, `Diagrams`, `Urls`, `Assets`.

- **Pure, no I/O.** Returns a `Site` of in-memory `SiteFile { path, contents }`; the CLI writes them. The page model is precomputed data (`PageProps`) — deterministic, byte-identical across runs.
- **Presentation in Svelte, embedded prebuilt.** `web/` builds `ssr.js`/`client.js`/`style.css`, committed and `include_str!`'d into the binary, so `pds doc` needs no JS toolchain; `build.rs` only checks the bundles exist.
- **SSR for first paint.** `Ssr` is the JSON-in/JSON-out seam: the native `QuickJsEngine` (`rquickjs`) evaluates `ssr.js` in-process; a wasm host implements the same trait against its own JS engine, so QuickJS never enters a wasm build. `Shell` owns the Rust-side document — `data-theme`, asset links, and the `window.__DATA__` hydration payload.
- **Diagrams as client islands.** `Diagrams` projects an `emit::View` and carries the laid-out `Scene` geometry (not server-embedded SVG); the client renders C4 as a Svelte Flow graph and sequences as an animated timeline. A failed projection degrades to an `Empty` placeholder — the cargo-doc stance that a partial model still documents.
- **`Urls`** maps every FQN to a page path + anchor for cross-links; **`Assets`** ships `style.css` + `client.js` once at the root and feeds `ssr.js` to the engine; **`config`** carries the `[doc]` table (`name`/`out`/`logo`/`theme`).

---

### `Format` — canonical formatter (`pseudoscript-format`)

`format(src) -> Result<String, FormatError>` (lib.rs). Maps to `Formatter`, `Printer`.

- Parse with `syntax::Parser.parse`; if any error-severity diagnostic, return `FormatError::Parse(messages)`; otherwise pretty-print.
- **Trivia-preserving** (printer.rs): two-space indent, comments and blank-line runs kept, one declaration per stanza. Idempotent — formatting the output is a no-op.

---

### `Cli` — the `pds` binary (`pseudoscript`)

A thin frontend over the libraries. Maps to `Args`, `Workspace`, the command components, `HttpServer`, `Watcher`.

- **`clap`** derive subcommands (`doc`/`check`/`fmt`/`tokens`/`lsp`) — the **Command pattern**: each handler reads input, calls the core, renders the result.
- **`Workspace`** resolves the project root by walking up to the nearest `pds.toml` (`find_root`) and loads the `[doc]` config + every `.pds` module (`load`, via `walkdir` + `toml`/`serde`).
- **`pds doc`** is the headline: `find_root` → `load` → `check_workspace` (reported, non-fatal, like `cargo doc`) → `graph` → `try_render_site` → write files.
- **`--serve`** hosts the site over **`tiny_http`** on `127.0.0.1`; **`--watch`** adds a **`notify`** filesystem watcher that rebuilds and bumps a live-reload version the browser polls.
- **I/O at the edge.** The CLI owns reading and writing; `anyhow` carries I/O errors to the exit code.

---

### `Lsp` — language server (`pseudoscript-lsp`)

Protocol, not language logic — reuses `Syntax` + `Model` + `Format`. Maps to `Server`, `Analysis`, `Convert`.

- **`tower-lsp`** over stdio, on a **`tokio`** runtime: an async `LanguageServer` `Backend` that handles JSON-RPC framing and lifecycle.
- **Document store.** Open documents live in memory keyed by URI; `did_change` (full sync) updates the text and republishes diagnostics — `Server.onChange` in the model.
- **`Analysis`** computes fresh per request, delegating to the core: `diagnostics` → `Model::check`, `format_edit` → `Format::format`, `hover`/`definition` → `parse` + symbol lookup.
- **`Convert`** maps byte-offset `Span`s to LSP positions (0-based line, UTF-16 column) via `LineIndex`.
- **lsp-server** (rust-analyzer's synchronous loop) is the alternative if hand-driven scheduling or salsa integration becomes the bottleneck.

---

## Dependency cheat-sheet (as built)

| Crate | Role | Used by |
| --- | --- | --- |
| `serde` | derive `Serialize`/`Deserialize` (Scene IR, tokens, diagnostics) | syntax, model, emit |
| `rustc-hash` (`FxHashMap`) | FQN-keyed symbol tables, graph index | model |
| `layout-rs` | C4 Sugiyama layout | emit |
| `tower-lsp` | LSP framework | lsp |
| `tokio` | async runtime (LSP; CLI runtime) | lsp, cli |
| `serde_json` | LSP payloads | lsp |
| `clap` | CLI args (derive) | cli |
| `anyhow` | edge error chaining | cli |
| `toml` + `serde` | parse `pds.toml` | cli |
| `walkdir` | discover `.pds` modules | cli |
| `tiny_http` | serve the doc site (`--serve`) | cli |
| `notify` | watch + live-reload (`--watch`) | cli |

**Weighed, not adopted:** `logos`, `rowan`/`cstree`, `chumsky`, `tree-sitter` (Syntax); `petgraph` (Model); DOT/Mermaid/PlantUML exporters (Emit, per ADR-017); `salsa` (incremental); `ariadne`/`codespan-reporting` (CLI diagnostics); `wasm-bindgen` (no browser build exists).

---

## Alignment with LANG.md open questions

- **Expression operators (resolved, ADR-038).** Arithmetic, comparison, equality, and boolean operators are in the grammar via a precedence cascade (LANG.md §7.5); they are type-checked, never evaluated. See "State a business rule with a constant threshold" above.
- **`View` dispatch without `match` (§12 #3).** The language can't `match` on a `View` variant, but a caller **constructs** a `View` with `from` (LANG.md §7.2) and passes it to `emit::Projector.project`, which dispatches internally. The dispatch is real; it just isn't surfaced in a disclosed body — the projection callable is a black box over the graph.

---

Sources:
- [rust-analyzer architecture (book)](https://rust-analyzer.github.io/book/contributing/architecture.html)
- [rust-analyzer architecture.md (repo)](https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md)
- [layout-rs](https://crates.io/crates/layout-rs)
- [tower-lsp](https://github.com/ebkalderon/tower-lsp)
- [rustc-hash (FxHashMap)](https://crates.io/crates/rustc-hash)
- [Red-Green Syntax Trees — an Overview](https://willspeak.me/2021/11/24/red-green-syntax-trees-an-overview.html)
- [chumsky (docs.rs)](https://docs.rs/chumsky/latest/chumsky/)
- [petgraph](https://docs.rs/petgraph/)
- [ariadne](https://docs.rs/ariadne/latest/ariadne/)
