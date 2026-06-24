# PDS-ARCH-004 — Standalone component

## BLUF

**A `component` with no `for` parent is a standalone `container` by another name —
declare it a `container`.** A `component` exists to model the *inside* of a
container: it is the finest C4 grain, defined entirely by the container it belongs
to. Strip the `for` and that defining relationship is gone — what remains is a
flat, top-level box, which is exactly what a standalone `container` is (ADR-042).
Allowing both forms gives a model two ways to express one structure, and a reader
can no longer tell whether `component Widget` is a deliberate fine-grained part or
just a container someone forgot to promote. Pick one: the `container` is the
canonical flat-grain primitive. Either give the component a `for <container>`, or
declare it a `container`.

## Example

A top-level `component` with no container:

```pseudoscript
//! ui
// ✗ PDS-ARCH-004: Widget is a component with no container to be part of.
public component Widget {
  Render(): void;
}
```

Promote it to the canonical flat-grain form:

```pseudoscript
//! ui
// ✓ a standalone container is the top-level box; component stays inside one.
public container Widget {
  Render(): void;
}
```

Or anchor it under the container it belongs to:

```pseudoscript
//! ui
public system App;
public container Shell for ui::App;
// ✓ Widget is now a genuine component — part of the Shell container.
public component Widget for ui::Shell {
  Render(): void;
}
```

## When it is acceptable

Mid-refactor, while a flat container set is being split into systems and
components, a model may carry parentless components transiently. The warning is
advisory (§9.6) and never fails a check, so it can ride along until the hierarchy
settles — but a shipped model should resolve every one.

## Links

- `LANG.md §4` — the system → container → component nesting, and the optional `for`.
- `LANG.md §9.6` — the architectural lints this rule joins.
- [decisions/042-standalone-container.md](../../decisions/042-standalone-container.md) — standalone nodes and why the container is canonical.
- [decisions/037-architectural-lints.md](../../decisions/037-architectural-lints.md) — why these are warnings.
- [C4 model — component](https://c4model.com/#ComponentDiagram).
