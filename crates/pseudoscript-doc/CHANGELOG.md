# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/flying-dice/pseudoscript/compare/pseudoscript-doc-v0.1.7...pseudoscript-doc-v0.2.0) - 2026-06-24

### Added

- [**breaking**] drop `self.` qualifier — same-node calls are bare `Name(args)` (refs #71) ([#73](https://github.com/flying-dice/pseudoscript/pull/73))
- *(doc)* the 3D universe island — the IDE's ForceGraph in the doc site
- *(doc)* rebuild the doc site — server SVG, health, search, system theme
- *(doc)* embed data entity and feature flow diagrams on doc pages (closes #42)
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(doc)* Markdown renderer with inline SVG; ban hyphens in dependency names
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(doc)* generated docs render the web IDE's sequence diagram
- *(docs)* authored Markdown docs + rich live-preview editor
- Svelte SSR doc renderer, wasm compiler API, and a web IDE
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(doc)* render SVG text everywhere — font on a group, separate .svg files

### Other

- apply rustfmt to fix main build ([#64](https://github.com/flying-dice/pseudoscript/pull/64))
- release v0.1.7 ([#63](https://github.com/flying-dice/pseudoscript/pull/63))
- *(doc)* pin the seeded-lint path — health page, section badge, nav count
- release
- release v0.1.4
- release v0.1.0

## [0.1.7](https://github.com/flying-dice/pseudoscript/compare/pseudoscript-doc-v0.1.6...pseudoscript-doc-v0.1.7) - 2026-06-10

### Added

- *(doc)* the 3D universe island — the IDE's ForceGraph in the doc site
- *(doc)* rebuild the doc site — server SVG, health, search, system theme
- *(doc)* embed data entity and feature flow diagrams on doc pages (closes #42)
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(doc)* Markdown renderer with inline SVG; ban hyphens in dependency names
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(doc)* generated docs render the web IDE's sequence diagram
- *(docs)* authored Markdown docs + rich live-preview editor
- Svelte SSR doc renderer, wasm compiler API, and a web IDE
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(doc)* render SVG text everywhere — font on a group, separate .svg files

### Other

- *(doc)* pin the seeded-lint path — health page, section badge, nav count
- release
- release v0.1.4
- release v0.1.0

## [0.1.6](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-doc-v0.1.6) - 2026-06-10

### Added

- *(doc)* embed data entity and feature flow diagrams on doc pages (closes #42)
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(doc)* Markdown renderer with inline SVG; ban hyphens in dependency names
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(doc)* generated docs render the web IDE's sequence diagram
- *(docs)* authored Markdown docs + rich live-preview editor
- Svelte SSR doc renderer, wasm compiler API, and a web IDE
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(ci)* release-plz reads releases from git tags — git_only + dep version reqs
- *(doc)* render SVG text everywhere — font on a group, separate .svg files

### Other

- release v0.1.4
- release v0.1.0

## [0.1.5](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-doc-v0.1.5) - 2026-06-10

### Added

- *(doc)* embed data entity and feature flow diagrams on doc pages (closes #42)
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(doc)* Markdown renderer with inline SVG; ban hyphens in dependency names
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(doc)* generated docs render the web IDE's sequence diagram
- *(docs)* authored Markdown docs + rich live-preview editor
- Svelte SSR doc renderer, wasm compiler API, and a web IDE
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(doc)* render SVG text everywhere — font on a group, separate .svg files

### Other

- release v0.1.0

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-doc-v0.1.0) - 2026-06-01

### Added

- *(docs)* authored Markdown docs + rich live-preview editor
- Svelte SSR doc renderer, wasm compiler API, and a web IDE
- static checker, Option/feature language features, enriched self-model
