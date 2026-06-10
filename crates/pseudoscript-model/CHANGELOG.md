# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/flying-dice/pseudoscript/compare/pseudoscript-model-v0.1.6...pseudoscript-model-v0.1.7) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(lang)* operators + top-level constants for business rules (refs #22)
- *(model)* architectural-principle lints with code + article links ([#24](https://github.com/flying-dice/pseudoscript/pull/24))
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(editor)* fold member impl blocks by default + right-click fold controls
- *(model)* enforce union-variant FQN references and close checker gaps
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* web IDE overhaul — JetBrains/Fleet shell, canvas, LSP, export
- *(checker)* reject method calls whose member does not exist (§6)
- *(semantic)* colour `//` and `/* */` comments
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(lang)* reject from/marker as binary operands; reject duplicate constants (refs #22)
- *(model)* behavioral conformance — align self-model bodies to the implementation
- *(resolve)* honour node visibility in hover/goto/references (§8.2)
- *(semantic)* colour feature/BDD keywords and Some/None
- *(parser)* localize from source-set errors and add fix-oriented hints
- *(complete)* type chained receivers (G2) and offer nodes after `for` (G5)
- *(complete)* member completion on local bindings; Option; macro args
- *(complete)* suggest other workspace modules at the root
- *(semantic)* colour a whole macro invocation as one decorator
- *(complete)* members of a ::-qualified node in another module
- *(fold)* start folds at the declaration, not its doc comment

### Other

- release
- release v0.1.4
- Merge origin/main — regenerate wasm + skill zip on the merged tree
- *(checker)* rustfmt the call-member check
- release v0.1.0

## [0.1.6](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-model-v0.1.6) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(lang)* operators + top-level constants for business rules (refs #22)
- *(model)* architectural-principle lints with code + article links ([#24](https://github.com/flying-dice/pseudoscript/pull/24))
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(editor)* fold member impl blocks by default + right-click fold controls
- *(model)* enforce union-variant FQN references and close checker gaps
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* web IDE overhaul — JetBrains/Fleet shell, canvas, LSP, export
- *(checker)* reject method calls whose member does not exist (§6)
- *(semantic)* colour `//` and `/* */` comments
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(lang)* reject from/marker as binary operands; reject duplicate constants (refs #22)
- *(model)* behavioral conformance — align self-model bodies to the implementation
- *(resolve)* honour node visibility in hover/goto/references (§8.2)
- *(semantic)* colour feature/BDD keywords and Some/None
- *(parser)* localize from source-set errors and add fix-oriented hints
- *(complete)* type chained receivers (G2) and offer nodes after `for` (G5)
- *(complete)* member completion on local bindings; Option; macro args
- *(complete)* suggest other workspace modules at the root
- *(semantic)* colour a whole macro invocation as one decorator
- *(complete)* members of a ::-qualified node in another module
- *(fold)* start folds at the declaration, not its doc comment

### Other

- release v0.1.4
- Merge origin/main — regenerate wasm + skill zip on the merged tree
- *(checker)* rustfmt the call-member check
- release v0.1.0

## [0.1.5](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-model-v0.1.5) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(lang)* operators + top-level constants for business rules (refs #22)
- *(model)* architectural-principle lints with code + article links ([#24](https://github.com/flying-dice/pseudoscript/pull/24))
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(editor)* fold member impl blocks by default + right-click fold controls
- *(model)* enforce union-variant FQN references and close checker gaps
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* web IDE overhaul — JetBrains/Fleet shell, canvas, LSP, export
- *(checker)* reject method calls whose member does not exist (§6)
- *(semantic)* colour `//` and `/* */` comments
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(lang)* reject from/marker as binary operands; reject duplicate constants (refs #22)
- *(model)* behavioral conformance — align self-model bodies to the implementation
- *(resolve)* honour node visibility in hover/goto/references (§8.2)
- *(semantic)* colour feature/BDD keywords and Some/None
- *(parser)* localize from source-set errors and add fix-oriented hints
- *(complete)* type chained receivers (G2) and offer nodes after `for` (G5)
- *(complete)* member completion on local bindings; Option; macro args
- *(complete)* suggest other workspace modules at the root
- *(semantic)* colour a whole macro invocation as one decorator
- *(complete)* members of a ::-qualified node in another module
- *(fold)* start folds at the declaration, not its doc comment

### Other

- Merge origin/main — regenerate wasm + skill zip on the merged tree
- *(checker)* rustfmt the call-member check
- release v0.1.0

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-model-v0.1.0) - 2026-06-01

### Added

- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model
