# Decisions

Architecture decision records for PseudoScript. Each pins a resolved fork out of `LANG.md` and into history. Format: `00N-name.md`.

## [001 — Result is the only fallible type (no Option)](001-result-only-no-option.md)

`Result<T, E>` is the only built-in generic; `Option`, `Some`, and `None` do not exist. The `?` clause here is superseded by ADR-008. **Superseded by ADR-019** — `Option` is reinstated.

Read in full for the rationale that a static model gains no power from a second absence type, and for the full list of `LANG.md` edits (§3.2, §6, §10).

## [002 — Bindings are immutable (single-assignment)](002-immutable-bindings.md)

A name is bound exactly once; reassignment and shadowing MUST be rejected.

Read in full when implementing the rebind/shadowing checks, or for the `static/7-rebind-rejected` conformance case it enables.

## [003 — No value construction; Ok/Err are result markers](003-no-construction-result-markers.md)

No constructor syntax: `Ok`/`Err` are result markers that tag a return branch, not constructors; `data` describes shape only. `from` is the sole value-producing form (it composes a value and records provenance) — see LANG.md §7.2. **Superseded by ADR-019** for the built-in generics: `Ok`/`Err`/`Some`/`None` now construct `Result`/`Option`; `data` stays `from`-only.

Read in full to see why `CtorExpr` became `ResultMarker` and how this reinforces ADR-001.

## [004 — Self/sibling calls via `self.`](004-self-calls.md)

`self` refers to the enclosing node; same-node callables are invoked as `self.Name(args)`. Bare `Name(args)` does not resolve. **Superseded by ADR-041** — `self.` is dropped; a same-node call is bare `Name(args)`.

Read in full for the sequence-diagram rendering (self-message on the lifeline) and the `self`-only-in-body scoping rule.

## [005 — Unhandled Result is allowed](005-dropped-result-allowed.md)

A call statement MAY drop its `Result` with no diagnostic; explicit `if` handling is the idiom, not a requirement.

Read in full when deciding whether to warn on unconsumed results — this ADR rejects that, and explains the progressive-disclosure reasoning.

## [006 — Union variants: inline declares and hoists, bare references](006-union-variants.md)

Record variant `| Name { ... }` declares and hoists to the module's type namespace; bare `| Name` references an existing `data Name`, or declares a fieldless variant (no hoist) when none exists. Record-variant name collisions MUST be rejected.

Read in full for the declare-vs-reference worked example, the fieldless-variant refinement (enum-style unions), and the namespace-scoping alternative that was rejected.

## [007 — Full `.` chaining](007-full-chaining.md)

`.` access and call chaining are unrestricted (`a.b.c`, `Repo.fetch(id).value`, `a.f().g()`), evaluated left-to-right.

Read in full for the left-recursive `Postfix` grammar form and the per-call sequence-diagram mapping.

## [008 — No optionality marker (`?` removed)](008-no-optionality-marker.md)

`?` is removed entirely; `[]` is the only type suffix. Amends ADR-001, which had kept `?`.

Read in full for why the optionality concept was judged too thin to keep, and the resulting `Type = Named [ "[]" ]` grammar.

## [009 — Doc-comment summary/body split on a blank `///` line](009-doc-paragraph-delimiter.md)

A `///` line with no text ends the summary; everything before it is the summary, everything after is the extended description.

Read in full for the delimiter example and the first-line/first-sentence alternatives that were rejected.

## [010 — `for` parent: FQN addressing and kind rules](010-for-parent.md)

A `for` parent is a `Path` resolved as an FQN; a cross-module parent MUST be `public`. A container's parent MUST be a `system`, a component's MUST be a `container`. **Amended by ADR-042** — `for` is optional on a container; the kind rule applies only when a parent is named.

Read in full when implementing parent resolution or the kind checks, and for the cross-module parenting example.

## [011 — Block contents: person behavior, callables-only blocks](011-block-contents.md)

A `person` MAY own callables (overturning the earlier no-behavior rule). A disclosed block holds callables only; containers and components never nest inside a block.

Read in full for the impact on sequence diagrams (persons as participants) and the spread of edits across §1, §4, §5, §9, §10.

## [012 — Reserved words and case sensitivity](012-reserved-words-case.md)

Keywords, primitive type names, `Result`, `Ok`, and `Err` are reserved. Identifiers are case-sensitive; the PascalCase/lowercase convention MUST NOT be enforced.

Read in full for the exact reserved set and the rejected enforced-casing alternative.

## [013 — Literal forms and placement](013-literals.md)

`Literal` is a string, number, or bool; `true`/`false` are reserved. Literals MAY appear as macro and call arguments. Extends ADR-012.

Read in full for the `Literal` grammar and the string-only / macro-args-only alternatives that were rejected.

## [014 — `for` iterates arrays only](014-for-iterables.md)

`for (x in Expr)` requires `Expr` to be an array `T[]`; `x` binds to `T`. Iterating a non-array MUST be rejected.

Read in full for the binding-type rule and the rejected iterate-`Result` alternative.

## [015 — Per-macro target constraints](015-macro-targeting.md)

Each built-in macro declares which declaration kinds it may attach to; the grammar permits a macro anywhere and the checker enforces targeting. All four current macros target callables.

Read in full when adding a macro or its target check, and for why targeting is a static rule rather than a grammar rule.

## [016 — Non-void callables must return on all paths](016-return-coverage.md)

A non-`void` callable MUST return on every path; a fall-through branch MUST be rejected. A `void` callable needs no return.

Read in full for the all-paths example and the `static/5-missing-return` conformance case it enables.

## [017 — `pds.toml` is the project root; `pds doc` generates the doc site](017-pds-toml-root-and-build.md)

`pds.toml` replaces `workspace.toml` as the sole root FQNs derive from. `pds doc` auto-documents the workspace as a static site (cargo-doc style) with embedded C4 and sequence diagrams; a `[doc]` table tunes presentation. SVG is the only backend; the `Scene` IR is the conformance surface.

Read in full when implementing the workspace loader, the doc-site generator, or the generation conformance layer; and for the rejected descriptor/backend alternatives.

## [018 — `feature` BDD scenarios: prose given/when/then for a node](018-feature-bdd-scenarios.md)

`feature Name for <node>` documents one behavioral scenario as a strict given/when/then flow of prose steps. Steps are not resolved against the model; `and`/`but` continue the preceding kind; the flow is grammar-enforced.

Read in full for the node-target rule, the feature namespace, and the rejected structured-step / multi-scenario / loose-flow alternatives.

## [019 — Option reinstated; the built-in generics are constructed by their markers](019-option-and-built-in-construction.md)

`Option<T>` joins `Result<T, E>` as a built-in generic; `Some(v)` / `None` construct it as `Ok(v)` / `Err(e)` construct a `Result`. Accessors `isSome` / `isNone` / `value` mirror `Result`; the checker narrows on `if (o.isNone)` / `if (o.isSome)`. Supersedes ADR-001 (no-`Option`) and ADR-003 (no-construction, for the built-in generics); ADR-008 (no `?`) stands.

Read in full for the construction rule, the accessor-only surface, and the rejected combinator-method alternative.

## [020 — Return-type and `from` checking for determinable forms](020-return-type-and-from-checking.md)

A `return` of a literal, an `Ok`/`Err`/`Some`/`None` marker, or a `Type from { … }` composition MUST match the declared return type (a union variant satisfies its union); a `from` target MUST be a `data` record or union variant. Bindings/calls/field accesses stay uninferred (shape hints, §1). **Superseded by ADR-035** — `from` targets any non-node type and checks a single-expression source.

Read in full for the determinable-forms scope, the variant-satisfies-union rule, and the rejected full-static-typing alternative.

## [021 — `from` can compose an array (`Type[] from { … }`)](021-array-from-composition.md)

`Type[] from { … }` composes an array; `Type from { … }` stays a single value. The return-type and `for`-iterable checks compare array-ness. Amends ADR-020.

Read in full for the array-composition rule and the rejected type-alias alternative.

## [022 — Inference-based body checks: references, member access, return types, call arity](022-inference-based-checks.md)

A conservative inference (concrete only for literals/markers/`from`/param-or-binding refs) drives four body checks: bare references resolve (§7/§8), `.field` reads name a real field of a known record (§2.2/§3.4), inferred return types match the declaration (§5.1), and calls to same-module callables match arity (§5.1). Cross-module member/arity, argument-type checking, and chained-receiver inference are deferred.

Read in full for the conservative-inference stance, the fieldless-variant reference rule, and the deferred scope.

## [023 — Boolean conditions and call-argument types](023-condition-and-argument-types.md)

An `if`/`while` condition whose inferred type is concrete MUST be `bool` (§7); each inferable call argument MUST match its parameter type by leaf name, with union-variant allowance (§5.1). Generic params, `Unknown`, and cross-module callees are skipped. Extends ADR-022.

Read in full for the two rules and the still-open operator question.

## [024 — Cross-workspace git dependencies](024-git-dependencies.md)

A `pds.toml` `[dependencies]` table declares other workspaces via git; each name is an FQN root scoped to the declaring workspace. Cross-workspace targets MUST be `public`; only direct dependencies are addressable; identity is `(source, revision, path)` so versions coexist; `pds.lock` pins the graph.

Read in full for the resolution model, the side-by-side identity rule, and the rejected `use`-statement / flat-namespace / version-solver alternatives.

## [025 — The Svelte-rendered site is the sole doc renderer](025-svelte-doc-renderer.md)

`pds doc` has one renderer: a Svelte presentation, prebuilt and embedded, server-rendered through an embedded `QuickJS` engine (a wasm host supplies its own). Diagrams ship as `Scene` geometry and hydrate into interactive client islands. The Rust HTML renderer, the `--static` flag, and the `[doc].renderer` key are removed. Amends ADR-017; the `Scene` IR stays the conformance surface.

Read in full for the SSR seam, the embedded-bundle stance, and the rejected two-renderer / server-SVG / JS-toolchain alternatives.

## [026 — Local path dependencies](026-local-path-dependencies.md)

A `[dependencies]` entry with `path` and no `git` is a local sibling workspace, resolved relative to `pds.toml`. `path` is overloaded — repo subdir under a git source, local dir without. A local dependency is read live, not version-pinned, and has no `pds.lock` entry; it MUST NOT be a git dependency's resolved source. Extends ADR-024.

Read in full for the source-selection rule, the no-lock rationale, and the rejected distinct-key / lock-local alternatives.

## [027 — Bindings state their type](027-explicit-binding-types.md)

A binding is `x: Type = Expr`; an unannotated `x = Expr` is rejected. Where the initialiser's type is determinable (literal, `from`, marker, bare reference) it MUST match the annotation; a call/field/`self`/`::` path is not inferred, so the annotation stands. The rule is uniform — a composition repeats its type in the annotation. The binding's type now reads from the source, not an inlay. Amends ADR-002 and ADR-022. **Superseded by ADR-035** — the annotation is removed; a binding states its type through `from` (`x = Type from Expr`).

Read in full for the uniform-annotation choice and the rejected self-typed-exemption / inlay-hint alternatives.

## [028 — Drop `alias`](028-drop-alias.md)

`alias Name = Path;` is removed. A cross-reference is its fully-qualified name (`banking::core::AccountStore`; `dep::module::Node` across a dependency). The keyword, the AST node, alias-following resolution, and the alias diagnostics all go. (ADR-030 further requires a node/type/variant reference to be its FQN, so a bare name resolves only to a parameter, binding, or `for` binding.)

Read in full for why sugar with no new expressive power earned removal, and the §8/§10 edits (former §8.4/§8.5 renumber to §8.3/§8.4).

## [029 — The filename is a module's only identity](029-filename-module-identity.md)

A module's FQN comes from its file path relative to `pds.toml`; the `//!` inner doc documents the module but MUST NOT name it. The path-less single-file check builds an anonymous module; a rootless file's FQN is its stem.

Read in full for the `//! Configuration` shadowing bug that motivated it, and the anonymous-single-file rule.

## [030 — A node, type, or variant reference is always its FQN](030-require-full-qualification.md)

Every reference to a node, type, or union variant MUST be its FQN, including one in the same module; a bare leaf name resolves only to a parameter, a binding, or a `for` binding. Member access, primitives, and `Result`/`Option` stay bare. The checker flags a bare same-module node/type/variant name and is gated to a named module — the path-less single-file check stays lenient.

Read in full for the four-meanings-per-bare-name ambiguity it removes, and the workspace-vs-anonymous gating.

## [031 — A hyphen in a path segment normalises to `_`](031-hyphen-filename-normalisation.md)

A kebab-case directory or filename maps to an identifier FQN segment by `-`→`_`, as Cargo maps `my-crate` to `my_crate`: `web-ide/file-tree.pds` is module `web_ide::file_tree`. One-way, load-time only; the file keeps its name on disk. A dependency name is not normalised — it MUST already be a valid identifier (§8.3). Single-sourced in `module_fqn`, mirrored by the web IDE's `fqnOf`.

Read in full for the ADR-030 interaction (a hyphen file could not address its own nodes) and the dependency-name contrast.

## [032 — A fieldless union variant is referenced through its union](032-fieldless-variant-fqn.md)

ADR-030 requires a variant reference to be its FQN, but §3.5 gave only a record variant one (`module::Name`, it hoists). A fieldless variant does not hoist and its name may repeat across unions, so it is referenced `module::Union::Variant` — the union is its scope. The checker resolves a value-position variant reference (`Ok`/`Err`/`from` operand, `return` value) against the union; a made-up variant, unknown union, or wrong module is rejected.

Read in full for the daemon fieldless-error example and the rejected hoist-fieldless-too alternative.

## [033 — A union variant binds a same-module `data` (not an ADR-030 reference)](033-variant-declaration-same-module-binding.md)

§3.5 called a bare `| Name` variant a "reference" to a same-module `data`, which ADR-030 would then require to be an FQN — but the grammar `Variant = Ident [ Record ]` admits only a bare identifier. The variant declaration is a declaration-site binding, not a use-site reference; ADR-030 governs use-site references (§8.1) and does not reach it. A variant names a same-module `data` and stays bare; a qualified variant (`| other::Name`) is rejected. Cross-module composition is a record field, not a variant.

Read in full for the rejected `Variant = Path [ Record ]` alternative and why same-module keeps a union resolvable within one file.

## [034 — A `data` symbol projects an entity view; a `feature` projects a flow view](034-data-entity-and-feature-flow-views.md)

Selecting a `data` symbol fell back to the context overview (no view of the type), and selecting a `feature` — not a graph node (§5.2) — projected nothing and crashed the canvas via a lifeline fallback that could not lay out. Now a `data` symbol projects an **entity view** (§9.4): a card of its fields/variants with reference arrows to the data types they name. A `feature` projects a **flow view** (§9.5): its given/when/then steps as connected nodes. The graph carries each `data` node's disclosed shape for the entity view.

Read in full for the field-resolution rule and the rejected reuse-the-C4-card and generic-lifeline-fallback alternatives.

## [035 — `from` is the universal typed value-producer](035-from-universal-value-producer.md)

`from` carries a type onto a value and is the only way a binding states one. A target MAY be any non-node type (`data`, variant, `Result`, `Option`, primitive, array); `T from { … }` composes a `data` record/variant, `T from expr` carries `T` onto a single value and checks the source's type where determinable — including a resolvable call's declared return. The `x: Type = Expr` annotation is removed (`x = Type from Expr`). A bare `data`/node FQN in value position is not a value; a fieldless variant stays one. Supersedes ADR-020 and ADR-027; amends ADR-003, ADR-021, ADR-022, ADR-032.

Read in full for the one-producer rationale, the determinable-source rule (call-return reads), and the rejected keep-annotations / permissive-`from` / structural-check alternatives.

## [036 — A structural-qualifier path is not an FQN; only the flat FQN resolves](036-no-structural-qualifier.md)

A node reference is its flat FQN `module::Name` (ADR-030), but a leaf-fallback let the C4-ancestry form (`Syntax::Parser`, `module::System::Container::Component`) resolve by its last segment — non-uniformly, so a cross-module drill passed the checker yet built an edge to a non-existent node and broke the C4 diagram while goto and the sequence view appeared to work. Now only the exact flat FQN resolves; the leaf-fallback is removed from the graph builder and the cursor resolver, and the checker reports the drill and suggests the flat FQN. Single-file/anonymous mode (ADR-029) stays lenient; the worked model and samples are migrated.

Read in full for the non-uniform-fallback root cause and the rejected keep-the-convenience alternative.

## [037 — Architectural-principle lints are graph warnings with codes and article links](037-architectural-lints.md)

Three C4 structure rules run over the resolved graph beyond §8.2 visibility, each a `Warning` carrying a `PDS-ARCH-NNN` code and the URL of its `docs/principles/` article (threaded as `code_description` so editors render the code as a link): PDS-ARCH-001 facade bypass (a cross-module `Call` into a `component`), PDS-ARCH-002 cyclic dependency (a cycle in the module call graph), PDS-ARCH-003 system-boundary bypass (a cross-`system` `Call` into a `container`). `Diagnostic` gains `code_description`, `Edge` gains the call-site `span`. Warnings never fail a check; the flagship samples ship with their real PDS-ARCH-002 cycles.

Read in full for the warning-not-error rationale and the rejected errors-and-fourth-rule alternatives.

## [038 — Arithmetic, comparison & boolean operators are type-checked, never evaluated](038-expression-operators.md)

Adds the full operator set — arithmetic `+ - * / %`, comparison `< > <= >=`, equality `== !=`, boolean `&& ||`, unary `-` — parsed and type-checked but never run. Operators compose by a precedence cascade; `Marker` and a `from` expression stay heads that do not combine. The static rules type arithmetic/comparison operands as `number`, boolean operands as `bool`, equality across the same primitive, and stay silent on any `Unknown` operand (ADR-022). A condition MAY now be a binary expression yielding `bool`. Resolves §12 #3.

Read in full for the precedence cascade, the operand/result table, and the rejected evaluate-the-operators alternative.

## [039 — Top-level `constant` is a primitive literal in a value namespace](039-top-level-constants.md)

Adds `constant Ident = Literal`: top-level only, a single primitive literal, type inferred from the literal, `public` for cross-module, referenced by FQN `module::NAME` and immutable. Constant names occupy a new value namespace (§8.1) beside type/node/feature. A bare leaf does not resolve to a constant; only `module::NAME` does (ADR-030). A non-literal initialiser and a macro on a constant are rejected.

Read in full for the value-namespace rule, the FQN-only resolution, and the rejected operator-initialiser alternative.

## [040 — Mandatory return types](040-mandatory-return-types.md)

Every callable MUST declare a return type; `F()` without one is rejected and `void` is the explicit nothing-spelling (`F(): void`, still composable as `Result<void, E>`). §10's `Callable` production requires `":" Type`; the parser recovers a missing type as `void` with a syntax diagnostic. Mandatory signatures make a call to any resolvable callable a determinable `return` operand, extending the §5.1 type-match clause cross-module without widening ADR-022's inference.

Read in full for the Java/C#-precedent rationale and the rejected implicit-void and full-typing alternatives.

## [041 — Same-node calls are bare; `self.` is dropped](041-drop-self-qualifier.md)

`self.Name(args)` is removed; a same-node callable is invoked by a bare call `Name(args)`. A bare name in call position resolves to a callable on the enclosing node; `self` is not reserved — it is an ordinary identifier with no special meaning. The renderer follows a same-node call's body and emits its cross-boundary calls, ending the silent-drop trap (issue #71) ADR-004's collapsed self-message caused. **Supersedes ADR-004.**

Read in full for the call-vs-construction non-ambiguity, the same-node-vs-local-value rendering split, and the rejected fix-renderer-only alternative.

## [042 — Standalone container: `for` is optional, parentless renders at the context layer](042-standalone-container.md)

A `container` MAY omit `for`; a parentless container is standalone — a top-level node at the context layer (§9.1) beside persons and systems, not omitted and not an error. A component still MUST name its parent. The kind rule survives: a container's `for` parent, when named, MUST be a `system`. **Amends ADR-010.**

Read in full for the dropped stub-system workaround and the context-view edge-bubbling change (nearest in-view ancestor, not specifically a system).
