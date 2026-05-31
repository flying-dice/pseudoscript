# Third-Party Payments

The payment provider is the one part of the system ACME Tickets does not control. Its calls cross a network it can't trust, can return *I don't know* instead of yes or no, and it talks back asynchronously over a webhook that may arrive late, twice, or never. `payments.pds` is the module that makes a reliable ledger out of an unreliable partner. Card data never enters the model — only provider tokens and references.

## Idempotent charge

`PaymentService.charge` runs authorise-then-capture in one flow, and the very first thing it does is check `receiptFor(order)`: if a receipt already exists, it returns it without calling the provider again (`ChargeIsIdempotent`). Combined with the order saga's deterministic per-hold id, this means a retried checkout — from an impatient user, a load balancer, the saga sweeper — can re-enter `charge` freely and never double-bill the card.

Every provider call also carries an `IdempotencyKey` scoped to the order and operation (`keyFor(order, "charge")`). So even below the receipt check, the *provider itself* is asked to dedupe: a retried authorise or capture lands on the same key and settles once. Two layers of idempotency — the local ledger and the provider key — guard the same money.

## The indeterminate timeout

This is the case that breaks naive payment code. When `Gateway.authorize` or `capture` fails, the provider error might be a **definite** decline (the card was refused) or **indeterminate** (the provider was unreachable, or the call timed out — the charge may or may not have gone through). `isDefinite` separates them, and the two paths could not be more different:

- **Definite** → mark the payment `Failed`. On a failed *capture*, also void the authorised hold (`cancelAuth`) so the customer's funds are released promptly.
- **Indeterminate** → do *nothing destructive*. Leave the payment in `Charging` and let reconciliation resolve it.

Treating a timeout as a failure is how you fail a buyer who was actually charged. The model refuses to guess. `reconcileCharge` is the resolver: for a payment still missing a receipt, it asks the provider for ground truth — `lookupCharge` converges it to `Charged` if the charge really settled, `chargeDefinitelyMissing` converges it to `Failed` only if the provider *definitively* says no charge exists. An outcome still unknown is left for a later sweep. `definitelyFailed` is the narrow signal the order saga abandons on — a still-`Charging` payment returns `Err`, so the saga never abandons on uncertainty.

## Settlement: the webhook, de-duplicated

Capture initiates settlement; the provider confirms it out of band by POSTing to `WebhookHandler.handle`. Inbound webhooks are hostile by default, so the handler defends in three steps:

1. **Verify the signature** — `Err(BadSignature)` if it doesn't check out.
2. **De-duplicate by event id** — `EventLog.seen`. A replayed webhook is *acknowledged* (so the provider stops retrying) but changes nothing (`WebhookReplayIgnored`).
3. **Reconcile into the ledger** via `settle`.

`settle` adds the crucial replay guard: it only applies to a payment **still awaiting settlement**. A payment that's already settled, refunded, or failed is left untouched, so a stale webhook arriving after a refund cannot resurrect the reversed payment (`LateWebhookCannotResurrect`). That guard-and-write — `awaitingSettlement` then `applySettlement` — is a single atomic compare-and-set, so a concurrent webhook and reconciler can't both apply.

## The pull reconciler

A webhook can be lost entirely — dropped connection, an outage on either side. Push delivery alone is never enough for money. So `reconcile` complements it from the other direction: for a payment still awaiting settlement, it *pulls* the truth from the provider (`Gateway.lookup`) and settles from that. The `batch::Reconciler` runs this every five minutes over `OrderService.unsettled`. A lost callback still converges the ledger; it just takes a sweep longer.

## A deliberate scope cut

Refunds are synchronous-final: the seat is released only after the provider acknowledges, and a failed refund surfaces (`RefundFailed`) rather than being swallowed. Asynchronous refund settlement — a `Refunding` state and a refund webhook mirroring capture — is a *documented* cut, not an oversight. Refunds are off the surge hot path and rarely contended, so the extra machinery isn't earned. Naming the cut is the point: the model says what it chose not to build.
