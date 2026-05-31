# Backends for Frontends

The model draws the clean split: a rich `TvBff`, a lean `PhoneBff`, both over `Catalog` and `Recommendations`. The trouble starts as the BFFs multiply and drift.

## Edge cases & failure modes

- **Logic drift across BFFs.** `TvBff.compose` stitches catalogue and recommendations; `PhoneBff` returns recommendations raw. The moment a real rule appears — "hide titles the viewer already finished" — it has to land in *both* BFFs, in sync, or the phone and TV quietly disagree about what's watchable. Two backends means two places to forget.
- **Partial-failure asymmetry.** `TvBff.home()` reads two services; `PhoneBff.home()` reads one. If `Catalog.rows()` is down, the TV home screen breaks while the phone — which never calls `Catalog` — sails on. Same outage, different blast radius per device, which makes incidents confusing to triage.
- **The thin BFF hides a coupling.** `PhoneBff` returns `Recommendations.forYou()` *directly* — its wire contract is now the recommendation service's shape. If `Recommendations` changes its `Lineup`, the phone app breaks with no BFF layer absorbing the change. A pass-through BFF gives up the very insulation that justifies it.
- **A new device is a new backend.** Add a watch or a car dashboard and you add a whole BFF, not a query parameter. The pattern scales in surface area, not in conditionals — that's the deal, but it's easy to underestimate.
- **No fan-out resilience shown.** `TvBff` calls `Catalog` and `Recommendations` in sequence with no timeout or fallback. A slow recommender stalls the entire TV home screen even though the catalogue rows are ready to render.

## Resilience

Pull genuinely shared rules *down* into `Catalog` and `Recommendations` (or a thin shared library) so the BFFs stay presentation-shaping, not rule-bearing. In `TvBff`, fan out to `Catalog` and `Recommendations` concurrently, each behind its own timeout, and degrade gracefully — render the catalogue rows with a "recommendations unavailable" placeholder rather than blocking the whole screen. Cache the recommendation payload per viewer so a flaky upstream doesn't blank the home page. This small model deliberately omits timeouts, fallbacks, caching, and any auth/personalization context — it shows the *shape* of the split, not its hardening.

## Pairs well with

- **API Gateway** — a gateway can route `/tv/home` and `/phone/home` to the right BFF and own auth and rate limits centrally, so the BFFs stay focused on shaping.
- **Circuit breaker** and **retry** — wrap each BFF's fan-out to `Catalog` and `Recommendations`.
- **Cache-aside** — memoize the recommendation lineup to cut latency and shield the shared service.
- **Sidecar / Ambassador** — keep mTLS and tracing out of the BFF code so it stays purely about presentation.
