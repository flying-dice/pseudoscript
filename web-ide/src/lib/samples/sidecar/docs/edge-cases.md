# Sidecar / Ambassador

The model shows the clean intent: `Checkout` makes a local call, `Sidecar` adds the wire concerns, `Payments` is reached only through it. The interesting failures hide in the word "retries."

## Edge cases & failure modes

- **Retries without idempotency double-charge.** The `Sidecar` adds *retries* and then calls `Payments.charge`. If the charge succeeds but the *response* is lost to a timeout, a blind retry charges the diner's `Charge` twice. The sidecar can't tell "never happened" from "happened, reply dropped." Retry safety depends entirely on `Payments` being an idempotent receiver — the model adds retries but says nothing about that.
- **Timeout vs. retry interaction.** A short timeout plus retries can *amplify* load on a struggling `Payments`: every slow call times out, retries, and piles on more work exactly when the service is already drowning. Retry storms turn a brownout into an outage.
- **The sidecar is in the hot path.** `Checkout` can only reach `Payments` *through* `Sidecar`. If the proxy crashes, leaks memory, or is slow to start, checkout can't charge anyone — even though both checkout and payments are healthy. The proxy's lifecycle is now a checkout dependency.
- **Hardcoded request shape.** `pay` builds `Charge from { order, 42 }` with a literal amount — fine for a demo, but it means the model can't show how a real amount, currency, or idempotency key would flow through the sidecar untouched. A production sidecar must pass an idempotency key through verbatim.
- **Silent policy mismatch.** mTLS and timeouts are configured *on the sidecar*, out of band from the code. A misconfigured proxy can fail closed (checkout can't pay) or fail open (traffic without mTLS) and the checkout code has no idea either way.

## Resilience

Make `Payments.charge` an **idempotent receiver**: pass a stable idempotency key in the `Charge` and have payments dedupe, so a retried call is safe. Cap retries with **jittered exponential backoff** and wrap the forward in a **circuit breaker** so the sidecar fails fast instead of hammering a sick `Payments`. Treat the proxy as a first-class dependency — health-check it, set startup ordering so checkout waits for a ready sidecar, and resource-limit it so it can't starve the app container. This small model deliberately omits the idempotency key, backoff config, circuit breaking, and the mTLS/timeout settings themselves — it shows *where* those concerns live, not their tuning.

## Pairs well with

- **Idempotent receiver** — the essential partner; it's what makes the sidecar's retries safe against double charges.
- **Retry** and **circuit breaker** — the resilience policies the sidecar exists to apply uniformly.
- **API Gateway** — the same "concerns at the edge" idea, but for inbound client traffic rather than outbound service calls.
- **Backends for Frontends** — keep app code pure by pushing the right concerns into a dedicated layer.
