# Catalog & search (CQRS)

Browsing and buying have opposite shapes. Browsing is enormous, read-only, and tolerant of
being a second stale. Buying is rare, write-heavy, and must be exact to the unit. The
`catalog` context splits these into separate containers — the textbook CQRS move.

## The write side

Two write containers carry the truth:

- **`Catalog`** is what a vendor drives: `publish` saves a `Listing` to `ListingStore` and
  emits `ListingPublished` so the read side can project it.
- **`Inventory`** is the authoritative stock counter, deliberately split out from the listing.
  `Catalog.hold` calls `Inventory.reserve`, an **atomic check-and-reserve** that returns
  `OutOfStock` when free stock is insufficient. `Catalog.release` calls `Inventory.unreserve`
  to return a hold.

Splitting inventory from the listing matters under load: a flash sale serialises on a small
counter, not on every shopper viewing the page. This is the no-oversell guarantee, enforced at
the data layer rather than by hoping concurrency stays low.

## The read side

`Search` is the read model. `Catalog.publish` and the inventory path emit `CatalogEvent`s, and
`Search.project` folds them into `SearchStore`. Every search hits `Search.query` (triggered by
`#[http("GET /search")]`); **nothing on the read path ever touches `Catalog` or `Inventory`**.

## Why CQRS here

- **Scale the sides independently.** The search index can be replicated and cached aggressively;
  the write containers stay small and consistent.
- **Rebuildable reads.** Because the index is a fold over events, it can be re-projected from
  scratch to add a facet or recover from corruption, without touching the source of truth.
- **Honest staleness.** A listing can be a moment behind in search and that's fine — the *hold*
  at checkout consults `Inventory` directly, so oversell is impossible even when the index lags.
  The `HoldStock` feature pins exactly that contract.
