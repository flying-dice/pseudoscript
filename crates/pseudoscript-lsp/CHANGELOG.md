# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/flying-dice/pseudoscript/compare/pseudoscript-lsp-v0.1.7...pseudoscript-lsp-v0.2.0) - 2026-06-24

### Added

- [**breaking**] drop `self.` qualifier — same-node calls are bare `Name(args)` (refs #71) ([#73](https://github.com/flying-dice/pseudoscript/pull/73))
- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(semantic)* colour a whole macro invocation as one decorator
- *(lsp)* keep completion scoped once a prefix is typed

### Other

- release v0.1.7 ([#63](https://github.com/flying-dice/pseudoscript/pull/63))
- release
- release v0.1.4
- extract pseudoscript-lsp-core; wasm = LSP-over-wasm
- release v0.1.0

## [0.1.7](https://github.com/flying-dice/pseudoscript/compare/pseudoscript-lsp-v0.1.6...pseudoscript-lsp-v0.1.7) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(semantic)* colour a whole macro invocation as one decorator
- *(lsp)* keep completion scoped once a prefix is typed

### Other

- release
- release v0.1.4
- extract pseudoscript-lsp-core; wasm = LSP-over-wasm
- release v0.1.0

## [0.1.6](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-lsp-v0.1.6) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(semantic)* colour a whole macro invocation as one decorator
- *(lsp)* keep completion scoped once a prefix is typed

### Other

- release v0.1.4
- extract pseudoscript-lsp-core; wasm = LSP-over-wasm
- release v0.1.0

## [0.1.5](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-lsp-v0.1.5) - 2026-06-10

### Added

- *(lang)* mandatory return types — explicit `: void`, call-operand return checking (refs #49)
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* align highlighting + folding to the LSP; add test suite
- *(ide)* source completion from the shared LSP engine
- *(docs)* authored Markdown docs + rich live-preview editor
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(semantic)* colour a whole macro invocation as one decorator
- *(lsp)* keep completion scoped once a prefix is typed

### Other

- extract pseudoscript-lsp-core; wasm = LSP-over-wasm
- release v0.1.0

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-lsp-v0.1.0) - 2026-06-01

### Added

- *(docs)* authored Markdown docs + rich live-preview editor
- static checker, Option/feature language features, enriched self-model
