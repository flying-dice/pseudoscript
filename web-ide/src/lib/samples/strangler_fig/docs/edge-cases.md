# Edge Cases & Resilience

The `Facade` model captures the clean routing decision. A reviewer's job is to ask what happens in the long, messy middle of a migration — the years where `Core` and `Mainframe` are both live and both half-right.

## Edge cases & failure modes

**Who owns the data?** `handle` routes a *request*, but a request reads and writes state. While a feature is half-migrated, the customer's balance may live on `Mainframe`, on `Core`, or — worst case — on both. If `Core.serve` writes a transfer that `Mainframe` never sees, the next mainframe-served request shows a stale balance. Every migrated slice needs a clear answer to "where does this data now live," and usually a period of **dual-writes**: write to both cores and reconcile, until the new core is authoritative.

**Sync drift between old and new.** Dual-writes are only as good as the sync that backs them. A failed write to one side, an out-of-order replay, or a schema mismatch leaves the two cores diverging silently. The façade can't see this — it just routes — so divergence surfaces as a customer complaint, not an error.

**The façade as a single point of failure.** Every channel now funnels through `Facade.handle`. If it is down, both cores are unreachable even though both are healthy. The thing that made migration safe also became the bank's narrowest waist.

**Rolling back a migrated slice.** Flipping a route from `Mainframe` to `Core` is easy. Flipping it *back* is not, if `Core` has already accepted writes the mainframe doesn't know about. Rollback must replay or reconcile that delta, or it loses data.

**Routing config drift.** `isMigrated` is the source of truth for the whole migration. A stale or wrong entry sends a migrated feature back to the dead mainframe, or an unmigrated one to a core that can't serve it. As the config grows to hundreds of routes, it needs the same review, versioning, and tests as application code.

**The last 20%.** The easy features migrate first. What's left is the gnarly, undocumented logic nobody wants to touch — and the project stalls there, paying for *two* cores indefinitely because the mainframe can never quite be switched off.

## Resilience

The model omits the state a real migration carries: the routing table behind `isMigrated`, the dual-write reconciliation, the per-slice authority flag. Harden it by making `isMigrated` a versioned, audited config with a fast rollback; by canarying each route flip to a small customer cohort before full cutover; by running a continuous reconciliation job that alarms on any divergence between `Core` and `Mainframe`; and by load-testing and redundantly deploying `Facade` so the single waist can't take both cores down. Above all, set an explicit deadline for the mainframe's retirement so the "last 20%" doesn't become permanent.

## Pairs well with

- **Anti-Corruption Layer** — when `Core` calls back into `Mainframe` for not-yet-migrated data, an ACL keeps the legacy model from leaking into the new core.
- **Feature flags** — `isMigrated` *is* a feature flag at heart; a real flag system gives per-cohort rollout and instant rollback for free.
- **Event sourcing / CDC** — change-data-capture off the mainframe is the cleanest way to keep `Core` in sync during dual-running.
- **Blue-green deploy** — the same route-flip discipline that switches a slice between cores can switch between versions of the new core.
