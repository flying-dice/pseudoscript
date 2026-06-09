# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-ide-v0.1.2) - 2026-06-09

### Added

- *(model)* architectural-principle lints with code + article links ([#24](https://github.com/flying-dice/pseudoscript/pull/24))
- *(web-ide)* 3D relationship-graph view (d3-force-3d)
- *(grid)* experimental grid placement — dials, drag-to-pin, search modes
- *(web-ide)* per-diagram layout tweaks toggle on the C4 canvas
- *(emit)* drive C4 layout with pseudoscript-dot, drop layout-rs
- *(emit)* data entity (ER) and feature flow canvas views (+ crash fix)
- *(editor)* fold member impl blocks by default + right-click fold controls
- *(ide)* dependency-aware language intelligence on a single typed wasm

### Other

- *(ide)* tsify the universe snapshot DTO; flow name in the 3D timeline
- *(universe)* drop dead personality layer; clean up 3D-graph web-ide
- cargo fmt --all + tidy stale universe doc comments
- rustfmt emit/ide (fix CI fmt gate)

## [0.1.0](https://github.com/flying-dice/pseudoscript/releases/tag/pseudoscript-wasm-v0.1.0) - 2026-06-01

### Added

- *(docs)* authored Markdown docs + rich live-preview editor
- *(web-ide)* folder workspaces, model-derived views, canonical sample, interactive timeline + design pass
- Svelte SSR doc renderer, wasm compiler API, and a web IDE

### Fixed

- *(emit)* project a black-box callable as a minimal sequence diagram
