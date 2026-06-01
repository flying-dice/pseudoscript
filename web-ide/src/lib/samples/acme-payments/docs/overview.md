# ACME Pay

A merchant's checkout page collects a card. A second later that money has to be authorised at the cardholder's bank, screened for fraud, captured, recorded against the merchant's balance, and — days later — paid out to their bank account. Somewhere in the middle the network times out, the card needs a 3-D Secure challenge, the cardholder disputes the charge, or the merchant's server retries the request twice. None of that may move a cent more or less than once.

That is the entire design problem. ACME Pay — a card-payments platform modelled here as sixteen bounded contexts — exists to take "charge this card" and make it correct across every failure the money path can throw: no double charges, no charge without a record, no payout that draws money the merchant doesn't have, no balance that drifts from the truth at the network.

## The hard part

Naively, taking a payment is one API call to a card network. The difficulty is in the corners that only appear under failure and concurrency:

- **Money over a network you don't control.** Authorise and capture run through the card network. Calls time out with the charge in an unknown state. Treating a timeout as a decline charges a card and tells the merchant it failed; treating it as success ships goods for money that never moved. Neither is acceptable.
- **Idempotency.** Merchants retry. Networks redeliver webhooks. The same request, the same callback, arriving twice must take effect once — on the charge, on the balance, on the payout.
- **A balance that must reconcile.** What a merchant is owed is the sum of every capture, refund, and dispute. Post any of them twice, or lose one, and the number is wrong — and payouts draw against that number.
- **Asynchronous truth.** Settlement, disputes, and payout outcomes arrive later, out of band, over webhooks that can be replayed, reordered, or lost. The ledger has to converge anyway.

## The architecture at a glance

The C4 system context (`context.pds`) names the platform `AcmePay`, four kinds of person — `Cardholder`, `Merchant`, `RiskAnalyst`, `SupportAgent` — and the external systems it integrates with: a `CardNetwork` (authorise/capture/refund and the source of settlements and chargebacks), a `BankingPartner` (payouts to merchant banks), each `MerchantEndpoint` (where outbound webhooks are delivered), and an `EmailProvider`. ACME Pay never sees a card number — only network tokens and references.

Inside, the work is split into bounded contexts, each its own module:

- **`gateway`** — the public `/v1/...` REST edge.
- **`identity`** — merchant accounts and the secret/publishable API-key model.
- **`vault`** — tokenisation; the PCI boundary the PAN never crosses.
- **`intents`** — the PaymentIntent state machine and confirm saga. The heart of the platform.
- **`charges`** — idempotent authorise/capture/refund against the network.
- **`risk`** — the fraud-scoring gate every confirmation passes.
- **`ledger`** — the double-entry merchant balance.
- **`refunds`** — reversing a captured payment.
- **`disputes`** — chargebacks: hold funds, contest, resolve.
- **`payouts`** — the bank-payout saga.
- **`webhooks`** — signed, retried outbound event delivery to merchants.
- **`notifications`** — merchant email on the moments that need a human.
- **`backoffice`** — the dashboard actions staff take (role-scoped).
- **`batch`** — the scheduled jobs that keep all of this converged.
- **`shared`** — the value objects every context uses.

## The headline guarantees

Four promises hold across every failure mode the design anticipates:

1. **No double money movement.** A client idempotency key makes each object-creating POST replay-safe at the edge; each network call carries a per-charge key; every balance entry and webhook delivery de-duplicates on a source id. A retried request, a replayed webhook, a re-run sweep — each takes effect exactly once.
2. **No charge lost to a timeout.** A charge with an unknown outcome is parked in `Processing`, never failed on a guess. A background sweeper reconciles it against the network — to succeeded or failed — so a cardholder is never failed while their charge might be real.
3. **A balance that can't be overdrawn.** The ledger is an immutable double-entry log: captures append a credit (net of fees), refunds and lost disputes append debits, and funds mature from pending to available by time. Payouts draw available with one atomic check-and-append, failed payouts append a reversal, and a clearing check asserts the books balance — money never vanishes between the ledger and the bank.
4. **Fraud screened before money moves.** Every confirmation passes the risk engine first. A blocked payment never reaches the network; a payment merely flagged for review proceeds but is surfaced for a human to inspect afterwards (blocking every flagged payment up front would reject too many good customers).

The rest of these docs walk through how each guarantee is built — start with the bounded-context map, then the PaymentIntent lifecycle, then the money-movement and balance deep dives, then the peer-review of edge cases.
