# Cache-Aside

A profile changes maybe once a month. It gets *read* a thousand times a day. Asking the database that question a thousand times — when the answer is the same every time — is the kind of waste that takes a service down at scale. Cache-aside is the app being honest about that.

## The problem

**Facecard** serves user profiles behind a busy social app. A **Visitor** opens someone's page; the app calls for that profile. Profiles are read constantly and change rarely — a near-perfect read/write ratio for caching.

Point every read straight at the database and it becomes the bottleneck. The same handful of popular profiles get fetched over and over; the database burns its connections and IO answering identical questions. Worse, a sudden spike — a profile goes viral — turns into a read storm that the one shared database has to absorb alone. The data is cheap to serve and almost never changes, yet you're paying full database price on every single view.

## The pattern

Cache-aside puts a fast key-value store *beside* the database and makes the application responsible for using it. The cache is dumb; the app is in charge.

Read the flow in `Profiles.get` directly. A `Visitor.view(id)` calls `Facecard::Profiles.get` (`GET /profiles/{id}`). The first thing `get` does is `Cache.read(id)`. On a **hit** — `hit.isSome` — it returns the cached `Profile` and the database is never touched. On a **miss**, the app does three explicit steps: `Database.load(id)` to fetch the real profile, `Cache.write(id, loaded)` to populate the cache, then return it. The next reader for that `id` hits the warm cache.

The key architectural fact is in the node shapes. `Cache` and `Database` are separate `system`s. `Cache` only knows `read` and `write`; `Database` only knows `load`. Neither knows about the other — the cache *never* reads the database itself. The orchestration lives entirely in `Profiles`. That's what "aside" means: the cache sits to the side, and the application reads around it.

The `LoadOnMiss` feature pins it: an uncached profile is loaded from the database and the cache populated; later views are served from cache; "the cache never reads the database itself."

## When to use it

When reads dominate writes and the same keys are read repeatedly — profiles, product catalogs, config. It's ideal when you want full control over what's cached and can tolerate a brief window of stale data after a write.

## When to avoid it

When data changes as often as it's read (the cache is always cold), or when reads must be strictly fresh. A bank balance behind a cache-aside layer is a bug waiting to be charged.

## Trade-offs

You gain a database shielded from the read storm and cheap, fast reads on hot keys. You pay with a consistency gap — the cache and database can disagree after a write — and with the app owning invalidation, which is famously one of the two hard problems in computing.
