# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-emit-v0.1.2) - 2026-06-09

### Added

- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(grid)* experimental grid placement — dials, drag-to-pin, search modes
- *(emit,web-ide)* nested boundary frames in the component view
- *(dot)* faithful nested cluster layout (system ⊇ container ⊇ components)
- *(dot)* cluster header band so titles clear member nodes
- *(web-ide)* one global layout config; stronger LR spacing
- *(dot)* lengthen labeled edges to fit their label
- *(web-ide)* per-diagram layout tweaks toggle on the C4 canvas
- *(emit)* drive C4 layout with pseudoscript-dot, drop layout-rs
- *(lang)* `from` is the universal typed value-producer (ADR-035)
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(emit)* collapse parallel C4 edges into one labelled arrow
- *(ide)* dependency-aware language intelligence on a single typed wasm
- *(ide)* web IDE overhaul — JetBrains/Fleet shell, canvas, LSP, export
- *(lang)* bindings state their type — `x: Type = Expr` (ADR-027)
- *(docs)* authored Markdown docs + rich live-preview editor
- *(emit)* richer sequence diagrams for the static site
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(emit)* draw cycle-closing back-edges in C4 views
- *(doc)* render SVG text everywhere — font on a group, separate .svg files
- *(emit)* project a black-box callable as a minimal sequence diagram

### Other

- *(model)* publish cross-module contracts on container/system faces + conformance guard
- cargo fmt --all + tidy stale universe doc comments
- rustfmt emit/ide (fix CI fmt gate)
- release v0.1.0

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-emit-v0.1.0) - 2026-06-01

### Added

- *(docs)* authored Markdown docs + rich live-preview editor
- *(emit)* richer sequence diagrams for the static site
- static checker, Option/feature language features, enriched self-model

### Fixed

- *(emit)* project a black-box callable as a minimal sequence diagram
