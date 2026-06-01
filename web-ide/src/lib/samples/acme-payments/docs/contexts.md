# Bounded Contexts

ACME Pay is sixteen modules, one per bounded context. Each owns a slice of the domain, exposes a narrow interface, and trusts its neighbours only through their published contracts. This is the C4 container map: what each box owns, and how a payment threads through them.

## The system context

`context.pds` is the outermost frame. It declares the people — `Cardholder`, `Merchant`, `RiskAnalyst`, `SupportAgent` — and the external systems: the `CardNetwork`, the `BankingPartner`, each merchant's `MerchantEndpoint`, and an `EmailProvider`. The actors carry real behaviour: a `Merchant` *creates*, *confirms*, *captures*, and *refunds* payments; a `Cardholder` *tokenises* a card and *completes* a 3-D Secure challenge directly with ACME Pay. Every actor-to-system edge is a real call path.

The `CardNetwork` is the critical dependency: it authorises and captures, drives 3-D Secure, and is the source of settlement and chargeback callbacks. The model treats it as idempotent on a supplied key and able to fail in three ways — declined, rate-limited, or unreachable — the last of which is the indeterminate case the whole recovery design turns on.

## The contexts and what they own

**`gateway`** is the public `/v1/...` edge. Each route (`TokenApi`, `IntentApi`, `RefundApi`, `DisputeApi`, `BalanceApi`) authenticates the API key, derives the merchant *from the key* — never from the body — and delegates into a domain service. It also owns the `Idempotency` layer: object-creating POSTs carry a client idempotency key whose first result is cached and replayed on retry. The decisions it discloses are authorisation (which key may call which route, whose merchant it acts as) and idempotent replay.

**`identity`** owns merchant accounts and the three-tier key model — publishable (client-side, tokenisation only), secret (full server-side), and restricted (scoped to named permissions, the tier staff tooling uses) — across two isolated modes, live and test. `Keys.authenticateSecret`, `authenticatePublishable`, and `authenticateRestricted` are the gate every request passes; the scope check on restricted keys is the disclosed authorisation decision.

**`vault`** is the PCI boundary. Raw card data crosses into the platform exactly once, at `Tokenizer.tokenize`, and is immediately exchanged with the network for a token. Everywhere else a card is a `PaymentMethodId`; `Instruments.instrumentFor` is the only path back to something the network can charge, and it never yields a PAN.

**`intents`** is the PaymentIntent state machine and the heart of the platform. `PaymentIntents.confirm` screens for fraud, authorises (clearing 3-D Secure if the issuer demands it), and — for automatic capture — captures, with a recovery path behind every money step. It raises `PaymentSucceeded`, `PaymentFailed`, and `PaymentCanceled`, the events the rest of the system reacts to.

**`charges`** is the hardened network integration. `ChargeService` mints a charge per attempt, runs idempotent authorise/capture/refund under per-charge keys, and reconciles an indeterminate outcome against the network rather than guessing. `Gateway` is the only component that talks to the `CardNetwork`. It does *not* track settlement — captured funds mature in the ledger by time.

**`risk`** is the fraud gate. `Engine.screen` turns signals into allow / review / block; the *decision* that follows is disclosed, the signal weighting is a black box. Only a *block* stops money — it never reaches the network; a *review* lets the payment proceed and parks it in `ReviewQueue` for a human to inspect (and refund) afterwards.

**`ledger`** is the immutable, append-only double-entry log that is the system of record. Money movements are appended as `BalanceTransaction` entries (idempotent on source id); the `available`/`pending` balance is *derived* by folding the log against each entry's `availableOn` time, not stored. A `Clearing` check asserts every account nets to zero. It is the only thing payouts may draw against.

**`refunds`** reverses a captured payment. `RefundService.create` reverses the charge at the network *first* and only then debits the balance via `RefundSucceeded`; a failed reversal is surfaced, never silently debiting the merchant.

**`disputes`** handles chargebacks. The `CardNetwork` opens a dispute; `DisputeService` holds the funds (via `DisputeOpened`), waits for the merchant's evidence, and applies the network's ruling — a win returns the funds, a loss leaves them debited. Inbound callbacks are signature-verified and de-duplicated.

**`payouts`** moves available balance to a merchant's bank. `PayoutService` draws the balance with one atomic ledger withdrawal, sends it through the `BankingPartner`, and reconciles the transfer; a rejected or returned payout restores the funds.

**`webhooks`** delivers the platform's events outward. `Dispatcher` subscribes to every domain event, signs it, and POSTs it to the merchant's endpoint with at-least-once, de-duplicated, backoff-retried delivery — the mirror image of an inbound webhook.

**`notifications`** reacts to the moments that need a human — a dispute opened, a payout failed or paid — and emails the merchant.

**`backoffice`** is the dashboard surface: an analyst rules on fraud-held payments, support issues refunds, each scoped to the authenticated merchant.

**`batch`** is the scheduled machinery: the saga sweeper, the payout scheduler and reconciler, the webhook redeliverer, and the dispute deadline sweep. Each job's unit of work is one entity, processed idempotently, so one stuck item never blocks the rest.

## How a payment threads through

A typical automatic-capture payment: the `Cardholder` tokenises a card through `TokenApi` → `vault`. The `Merchant` creates an intent through `IntentApi` → `intents` (under an idempotency key), then confirms it. `intents` screens it through `risk`, resolves the instrument through `vault`, and authorises through `charges` → `CardNetwork`. If 3-D Secure is required the intent parks until the cardholder completes the challenge; otherwise `charges` captures, `intents` raises `PaymentSucceeded`, `ledger` appends a pending credit, and `webhooks` delivers `payment_intent.succeeded` to the merchant for fulfilment. The credit matures from pending to available by its `availableOn` time (no further event); the `batch` payout scheduler then draws the available balance, and `payouts` sends it to the bank.
