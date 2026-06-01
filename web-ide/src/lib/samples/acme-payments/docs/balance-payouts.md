# Balance, Payouts & Disputes

The `ledger` is the single source of truth for what a merchant is owed: an **immutable, append-only log of double-entry balance transactions**. Entries are never updated or deleted; the balance is *derived* by folding the log. Captures, refunds, disputes, and payouts all converge here, and payouts draw only against it.

## An immutable log, a derived balance

Money movements are recorded as `BalanceTransaction` entries appended to `ledger::EntryLog` — never as updates to a mutable counter. The rationale: an immutable log of events lets you reconstruct past state by replaying every event up to any point, which is exactly what financial systems need for audit and correction. The merchant's `Balance` is computed on demand by `derive`, which folds the log.

Every append is **idempotent on the entry's `(type, source)`**, so an at-least-once event delivered twice appends once:

| Event | Entry appended | Source key |
|---|---|---|
| `intents::PaymentSucceeded` | `ChargeCredit`, net of fee, pending until `availableOn` | charge id |
| `refunds::RefundSucceeded` | `RefundDebit` (available immediately) | refund id |
| `disputes::DisputeOpened` | `DisputeDebit` (amount + dispute fee) | dispute id |
| `disputes::DisputeWon` | `DisputeReversal` (returns the amount) | dispute id |
| payout draw / failure | `PayoutDebit` / `PayoutReversal` | payout id |

## Pending vs available is a function of time

A `BalanceTransaction` carries an `availableOn` timestamp. `derive` sums entries whose `availableOn` has passed into **available**, and the rest into **pending** — so a captured payment matures from pending to available *purely by the passage of time*, with no settlement event to process. Funds transition from pending to available at each balance transaction's `availableOn` time. The settlement window (T+X) is a black box on the credit, not a webhook. Only available funds can be drawn for a payout.

(An earlier draft matured funds on a per-charge settlement webhook; that was replaced with time-based maturation, which is both simpler and standard for card settlement.)

## Clearing — the double-entry invariant

`ledger::Clearing.clear`, run hourly by `batch::LedgerClearing`, asserts the invariant that makes double-entry trustworthy: every credit nets against a debit, payout, or reversal, so internal accounts return to zero at steady state. A nonzero account surfaces a missing or incorrect entry (`Unbalanced`) for investigation rather than being silently absorbed — the "clearing" check. Because the log is immutable, this is a pure fold over history, not a mutable reconciliation.

## Payouts — drawing without overdrawing

`payouts::PayoutService.schedule` moves available balance to a merchant's bank. The risky part is concurrency: two scheduler runs, or a retry, must not pay the same balance twice or draw money that isn't there.

1. **Idempotent entry.** A merchant with a payout already in flight returns it — no second draw (`PayoutIsIdempotent`).
2. **Atomic withdrawal.** `ledger::Balance.withdraw` appends a `PayoutDebit` *only if* available covers it — one atomic check-and-append (`appendIfAvailable`), returning `InsufficientFunds` otherwise (`PayoutCannotOverdraw`). Two concurrent payouts cannot both draw the same funds, and the balance can't go negative from a payout.
3. **Send through the partner.** The payout is recorded `Pending`, then sent through the `BankingPartner` under an idempotency key, via a transactional outbox written with the withdrawal — so a crash mid-payout resumes without double-paying.
4. **Compensate on failure.** If the partner rejects, or later returns the transfer, `refund` appends a `PayoutReversal` (idempotent on the payout) and records the payout `Failed`, raising `PayoutFailed`.

Two timings the model keeps separate: the **payout schedule** (when a payout is sent — `batch::PayoutScheduler`) is distinct from **settlement timing** (when funds become available — the entry's `availableOn`). Each payout reflects the available balance at the moment it was created. The invariant, from `RejectedPayoutRestoresFunds`: **money never vanishes between the ledger and the bank.**

## Disputes — money held, then resolved

A chargeback inverts the usual flow: the `CardNetwork` initiates it, and ACME Pay reacts. `disputes::DisputeService.open` is an inbound webhook — signature-verified and de-duplicated like any network callback — that records the dispute and raises `DisputeOpened`. That event appends a `DisputeDebit` for the disputed amount *plus the dispute fee* immediately (`DisputeHoldsFundsImmediately`): the platform holds the money before the merchant can spend it.

The merchant then `submitEvidence`, which forwards to the network and moves the dispute to `UnderReview`. The network rules through another inbound callback, `resolve`:

- **Won** → raise `DisputeWon` → `ledger` appends a `DisputeReversal`, returning the held amount (`WonDisputeReturnsFunds`).
- **Lost** → the funds stay debited; nothing more to do.

A merchant who never responds is swept up by `batch::DisputeSweeper`, which `forfeit`s disputes past their deadline — marking them `Lost` without a further debit, since the funds were already held at open.

## The pattern across all three

Balance, payouts, and disputes share one shape: **append an immutable entry, derive truth by folding, and key every entry so it's safe to repeat.** It's the same discipline as the charge path — idempotency plus compensation — applied to the slower, asynchronous money flows where the truth arrives over a webhook or matures with the clock.
