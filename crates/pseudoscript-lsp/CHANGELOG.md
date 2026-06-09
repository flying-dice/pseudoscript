# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-lsp-v0.1.2) - 2026-06-09

### Added

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
