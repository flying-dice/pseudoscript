# ADR-009 — Doc-comment summary/body split on a blank `///` line

**Status:** Accepted
**Affects:** LANG.md §2.1, §10

## Context

§2.1 splits a `///` block into a short summary (compact diagrams) and an extended description (tooltips) but never defined where the boundary is.

## Decision

- A `///` line with no text after the marker is a **blank doc line** and ends the summary.
- Everything before the first blank doc line is the summary; everything after is the extended description.
- A block with no blank doc line is summary-only; its extended description is empty.

```pds
/// Fetches current banking info.      <- summary
///
/// Reads from the durable store       <- extended
/// and never mutates.
```

## Consequences

- §2.1: defines the blank-`///`-line delimiter.
- §10: `DocLine` admits an empty form (marker followed by newline).
- Rejected alternatives: first-line-is-summary; first-sentence-is-summary.
