# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/flying-dice/pseudoscript/releases/tag/v0.1.2) - 2026-06-09

### Added

- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(doc)* support [doc].format in pds.toml (html|md)
- *(doc)* Markdown renderer with inline SVG; ban hyphens in dependency names
- reimagine landing on model-driven theme; add `pds eval`
- *(ide)* web IDE overhaul — JetBrains/Fleet shell, canvas, LSP, export
- *(cli)* local path deps, monorepo tooling, hardened git deps + ADR-026
- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model

### Fixed

- honour flat-FQN rule in examples + make pds check workspace-aware

### Other

- *(model)* publish cross-module contracts on container/system faces + conformance guard
- Merge feat/from-universal-value-producer into develop (rebased on main)
- release v0.1.0
- Initial commit: PseudoScript spec, conformance suite, and tooling

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/v0.1.0) - 2026-06-01

### Added

- *(cli)* local path deps, monorepo tooling, hardened git deps + ADR-026
- *(docs)* authored Markdown docs + rich live-preview editor
- git workspace dependencies — pds add/install + cross-workspace refs
- static checker, Option/feature language features, enriched self-model

### Other

- Initial commit: PseudoScript spec, conformance suite, and tooling
