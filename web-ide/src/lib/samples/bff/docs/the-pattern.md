# Backends for Frontends

One home screen on a 65-inch TV and the same home screen on a phone over cellular are not the same product — so why should they share a backend?

## The problem

Bingewatch streams video to a living-room **TvViewer** and a commuting **PhoneViewer**. Both open the same conceptual screen: a home page of shows. But the right answer differs wildly by device.

The TV wants a *rich* lineup — many rows, large artwork, autoplay previews — because the screen is huge and the box is on fast home WiFi. The phone wants a *lean* one — a single column, small images, no autoplay — because the screen is small and the viewer is paying for every megabyte on cellular.

A single generic `/home` backend forces a bad compromise. Make it rich and the phone wastes data and battery loading artwork it shrinks to a thumbnail. Make it lean and the TV looks empty. Worse, the client has to do the trimming: the phone downloads the fat payload and throws most of it away. Every device-specific tweak now means a conditional in one shared, ever-branching endpoint.

## The pattern

Bingewatch gives each frontend its *own* backend. `TvViewer.open()` calls `Bingewatch::TvBff.home()`; `PhoneViewer.open()` calls `Bingewatch::PhoneBff.home()`. Two containers, two contracts.

The **TvBff** does the heavy lifting. Its `home()` route (`GET /tv/home`) reads `Catalog.rows()` *and* `Recommendations.forYou()`, then calls its own `compose(catalog, picks)` to stitch them into one rich `Lineup { rows }` — many rows, the artwork-heavy screen the TV wants.

The **PhoneBff** is deliberately spare. Its `home()` route (`GET /phone/home`) skips the catalogue entirely and returns `Recommendations.forYou()` straight through — one column, no autoplay, minimal bytes on the wire.

The crucial part: both BFFs sit over the *same* shared services. `Catalog` and `Recommendations` are unchanged systems behind both. The device-shaping logic lives in the BFF that owns that device; the source-of-truth services stay generic and reusable. The `TailoredPerDevice` feature pins the guarantee — each device gets a screen shaped for it, "but both read the same catalogue and recommendations."

## When to use it

Use a BFF when distinct clients — TV, phone, web, watch, a partner integration — have genuinely different needs in payload shape, chattiness, or aggregation, and a one-size endpoint has become a thicket of `if device == ...`. It lets each client team own and ship its own backend at its own pace.

## When to avoid it

If your clients are near-identical (a web app and its mobile-web twin), a BFF per client is duplicated effort. Don't let BFFs accumulate real business rules — that belongs in the shared services, or you'll fork the truth N ways.

## Trade-offs

You trade one backend for several, multiplying code, deploys, and the surface to keep in sync. Common logic risks being copy-pasted across BFFs. The win is sharp: each client gets exactly the payload it wants, the shared services stay clean and generic, and device teams stop fighting over one endpoint.
