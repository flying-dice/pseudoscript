# ADR-031 — A hyphen in a path segment normalises to `_`

**Status:** Accepted
**Affects:** LANG.md §8.1

## Context

A module's FQN derives from its file path: each directory and the filename stem becomes a `::`-joined segment (§8.1). An FQN segment is referenced as an identifier, and an identifier (§2.2) admits `_` but not `-`. A kebab-case path — `web-ide/file-tree.pds`, common for files mirroring a kebab-case project — produced the segment `web-ide`, which no reference could spell: `web-ide::Node` does not lex, the `-` ending the identifier `web`.

ADR-030 sharpened the failure. Every node, type, and variant reference — including one in the same module — MUST now be its FQN. So a hyphen-named file could not even address its *own* nodes: `file-tree::Widget` was unwriteable, leaving the file's declarations unreferenceable from anywhere, itself included.

## Decision

A hyphen in any path segment normalises to `_` when deriving the FQN, as Cargo maps a `my-crate` package to the `my_crate` identifier. `web-ide/file-tree.pds` is the module `web_ide::file_tree`; its nodes are `web_ide::file_tree::Node`.

- Normalisation is per segment, applied to directories and the filename stem alike.
- It is one-way and load-time only: the file keeps its hyphenated name on disk; only the FQN changes.
- A **dependency name** (§8.3) is not normalised — it MUST already be a valid identifier, rejected otherwise. A dependency name is authored in `pds.toml` and written verbatim in code as an FQN root, so silently rewriting it would split one name across two spellings. A filename is a filesystem artifact the author may not freely rename, so it is met where it is.

## Consequences

- LANG.md §8.1 states the rule and contrasts it with the dependency-name requirement.
- `pseudoscript_project::module_fqn` normalises each segment; it is the single derivation the native loader, the dependency loader, and the LSP (via `path_fqn`/`uri_stem`) all route through. The web IDE's `fqnOf` mirrors it.
- A collision — `a-b.pds` and `a_b.pds` both mapping to `a_b` — is left to the module-uniqueness check, the same as any two files claiming one FQN.
- Rejected alternative: reject hyphenated filenames outright (the dependency-name rule). It is the stricter, simpler rule, but it bars a conventional, often externally-imposed filename for no gain the author can act on, where a deterministic normalisation just works.
