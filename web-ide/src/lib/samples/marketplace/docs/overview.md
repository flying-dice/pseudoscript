# ACME Market

ACME Market is a multi-vendor marketplace. Shoppers browse one catalog and check out one basket,
but that basket can span many independent sellers — each with their own stock, their own
fulfilment, and their own payout. The marketplace is the trusted middle.

## The hard part

A marketplace is three hard problems wearing one storefront:

- **One basket, many sellers.** A single checkout must hold stock and confirm fulfilment
  across vendors that know nothing about each other — atomically enough that the buyer never
  pays for an item that just sold out.
- **Read-heavy browsing, write-careful inventory.** Search and listing pages dwarf purchases
  in volume, but inventory counts must stay exact under concurrent buyers.
- **Money owed to many.** The buyer pays once; the marketplace later owes dozens of vendors.
  A lost or duplicated payout is a vendor-trust disaster.

## The architecture at a glance

The C4 context (`context.pds`) names the marketplace `Market`, three actors — `Shopper`,
`Vendor`, `FulfilmentOp` — and two external systems: a `KycProvider` and a `PaymentProvider`.
The work is split into bounded contexts, each its own module:

- **`shared`** — the value objects, payloads, and event/error families every context uses.
- **`vendors`** — `Vendors` onboards sellers through KYC before they can list.
- **`catalog`** — `Catalog` and `Inventory` (write side) plus the `Search` projection (read side).
- **`orders`** — the `Orders` saga that holds stock, charges once, and confirms across vendors.
- **`payouts`** — the `Payouts` idempotent receiver, the `Ledger`, and the `Outbox` that pays vendors.

## How to read this model

Start with **catalog** to see the CQRS split, then **orders** — the checkout saga is the
showpiece and pulls on every other context — then **payouts** to see how one buyer payment
fans out to many vendor payouts through an outbox.

## Patterns on display

- **CQRS** — `Catalog`/`Inventory` (write) vs the `Search` projection (read).
- **Inventory holds** — `Catalog.hold` atomically reserves stock to prevent oversell.
- **Saga** — `Orders.place` holds, charges, and confirms across vendors with compensation.
- **Transactional outbox** — `Ledger` and `Outbox` commit together; a relay drains payouts.
- **Idempotent receiver** — `Payouts.onShipped` dedupes replayed shipment events.
