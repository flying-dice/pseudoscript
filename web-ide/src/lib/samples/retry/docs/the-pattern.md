# The Pattern

Most remote failures aren't failures at all — they're blips that clear in a hundred milliseconds. **Postbird**, a transactional-email sender, shows how retrying with backoff rides out the blips without giving up on the real errors or stampeding a recovering provider.

## The problem

`Postbird` sends notification emails through `Provider`, a third-party email/SMS service. An `App` calls `notify`, and `Postbird`'s `Dispatcher` hands the `Message` to `Provider.deliver`.

`Provider` is "mostly fine, sometimes a transient blip" — a momentary network hiccup, a brief 503, a connection reset. The naive design fails the whole send on the first such blip, so a user misses their welcome email because of a glitch that lasted 200ms. But the opposite mistake is just as bad: retrying *everything*, *forever*, *immediately*. That turns a struggling provider into a downed one — every client retrying in a tight loop becomes a self-inflicted DDoS, and a permanently bad address gets retried to infinity.

## The pattern

A retry policy distinguishes *transient* failures (worth another go) from *permanent* ones (hopeless), and spaces its attempts so it doesn't amplify the problem. In the model, `Dispatcher.send` owns this:

1. `send` calls `Provider.deliver(msg)`. On `Ok`, it returns `Ok(Sent)` — done.
2. On `Err`, it hands off to `self.retry(msg, attempt.error)`.

`retry` is where the policy lives, and the model's types make the rules explicit. `SendFailed` carries a `transient: bool` — the signal for whether a failure is even *eligible* for retry. The doc-comment spells out the policy: **exponential backoff + jitter** (each attempt waits longer, with randomness so many clients don't sync up), bounded by a **budget**, and *"a permanent failure (a bad address) is not retried."* When the budget is spent, `retry` returns `Err(GaveUp)` — a distinct, honest outcome.

The `RetryWithBackoff` feature pins both halves: *"it retries with exponential backoff until the send succeeds, but a permanent failure like a bad address is surfaced without retrying."* Transient blips are absorbed silently; permanent errors fail fast. The module header also flags the crucial companion: *"pairs with an idempotent receiver so a retried send doesn't double-email."*

## When to use it

Retry transient, self-healing failures over a network: timeouts, 503s, throttling responses, dropped connections. It's the cheapest resilience pattern when the dependency mostly works and the occasional failure clears on its own.

## When to avoid it

Never retry a *permanent* failure — a 400, a bad address, an auth error. Retrying won't help and wastes the budget. Avoid naive retries on non-idempotent operations (charging a card, sending an email) unless the receiver dedupes, or you'll double-act. And don't retry when latency matters more than success; each attempt adds delay.

## Trade-offs

Retries trade latency and load for success rate. Backoff makes a slow request slower, sometimes much slower, before it gives up. Aggressive retries add load exactly when the system is weakest — backoff, jitter, and a budget exist to bound that, and getting them wrong reintroduces the stampede. And retries quietly mask flakiness: a provider failing 40% of calls *looks* healthy through a retry layer, right up until it isn't.
