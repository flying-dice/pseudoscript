# Architectural principles

The PseudoScript linter flags violations of C4 architectural principles as
**warnings** (the model stays valid) carrying a stable `PDS-ARCH-NNN` code. Each
code links to one article here: a BLUF verdict, a worked `.pds` example, and
links out.

The principles are enforced over the resolved architecture graph (`LANG.md §9`),
beyond the §8.2 visibility rules. A `public` node still satisfies visibility; these
rules judge *how* one part reaches another.

| Code | Principle | Fires when |
|------|-----------|------------|
| [PDS-ARCH-001](PDS-ARCH-001-backdooring-facade.md) | Facade bypass (backdooring) | A cross-module body call reaches an internal `component` instead of its container's published face |
| [PDS-ARCH-002](PDS-ARCH-002-cyclic-dependency.md) | Cyclic dependency | The module dependency graph (from body calls) contains a cycle |
| [PDS-ARCH-003](PDS-ARCH-003-system-boundary.md) | System-boundary bypass | A call crosses a `system` boundary into a `container` of the other system instead of its published face |

Warnings are advisory. Suppress none by default — fix the structure, or accept the
warning where the coupling is deliberate.
