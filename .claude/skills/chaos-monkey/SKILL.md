---
name: chaos-monkey
description: >-
  Find the unhappy paths an existing Cucumber/BDD suite doesn't cover and write them up as new Gherkin
  scenarios in the feature files. Use whenever the user wants to harden, stress, or find gaps in
  Cucumber tests, .feature files, or Gherkin scenarios, or mentions a "chaos monkey", negative testing,
  edge cases, failure modes, or unhappy paths. Trigger even on a casual "think of ways to break this"
  or "what are we not testing?" about a BDD codebase.
---

# Chaos Monkey

You are a chaos monkey loose in a Cucumber suite. The existing scenarios describe the happy path — the
well-behaved user getting the expected result. Your job is to find every way the system could break and
capture each as a new Gherkin scenario in the feature files.

## Method

1. **Read** the happy-path scenarios. Note the actors, every precondition, every input, and every
   dependency — each one is something that can be absent, malformed, out of order, or fail.
2. **Hunt.** Don't stop at the two or three obvious negatives. Work the taxonomy below category by
   category and keep pushing: for each input ask what the *worst* value is, for each step ask what
   happens if it's skipped or repeated, for each dependency ask what happens when it dies. The
   interesting bugs hide past the obvious ones — chase the weird, the rare, and the "surely nobody
   would do that" until the category is genuinely exhausted.
3. **Write** each worthwhile failure as a Gherkin scenario in the relevant `.feature` file. Match the
   repo's existing phrasing, tags, and style so it reads like the rest of the suite; don't duplicate
   what's already there. Tag new scenarios `@chaos` so they're easy to run or exclude.

## Where to look

| Category | Ask |
|---|---|
| Invalid input | wrong type/format, garbage, negative, injection-looking strings |
| Boundaries | 0, −1, max, max+1, empty, off-by-one on dates/quantities |
| Missing data | required field omitted, null, empty list, whitespace-only |
| Auth | not logged in, expired session, wrong role, another user's resource |
| State & order | steps out of order, acting on deleted/cancelled/completed entities |
| Duplicates | same request twice, replay, duplicate key |
| Concurrency | two actors on one record, the last item sold twice |
| Dependencies | downstream 500s, times out, returns partial/garbage, unreachable |
| Limits | rate limit, quota, payload too large, page past the end |
| Time | expiry boundary, timezones, DST, leap day, clock skew |
| Encoding | unicode, emoji, RTL, very long strings, locale formats |
| Partial failure | network drop mid-flow, step 2 of 3 fails — is step 1 rolled back? |

`references/chaos-taxonomy.md` has sharper provocations and examples per category — read it to go deep.

## What good looks like

- **One behaviour per scenario**, asserting the failure *and* the expected handling (clean rejection,
  helpful message, state unchanged) — so it documents intended behaviour, not just pokes the system.
- **`Scenario Outline` + `Examples`** for families of bad inputs that share an expected outcome.
- **Realism over volume.** Ten sharp, plausible unhappy paths beat fifty contrived ones. Skip failure
  modes that can't occur for this feature.
- **When the intended behaviour is unknown** (what *should* happen when the dependency times out?),
  write your best-guess expectation and flag it rather than asserting something arbitrary.
- Security-flavoured scenarios assert that bad input is safely *rejected* — not working exploits.

## Example

Happy path:

```gherkin
Scenario: Customer withdraws available funds
  Given my account balance is 100
  When I withdraw 40
  Then my balance should be 60
```

Chaos:

```gherkin
@chaos
Scenario Outline: Withdrawals at and beyond the balance boundary
  Given my account balance is 100
  When I withdraw <amount>
  Then the withdrawal should be <result>
  And my balance should be <balance>

  Examples:
    | amount | result   | balance |
    | 100    | accepted | 0       |
    | 101    | rejected | 100     |
    | 0      | rejected | 100     |
    | -40    | rejected | 100     |

@chaos
Scenario: Withdrawal is blocked when the account is frozen
  Given my account balance is 100
  And my account is frozen
  When I withdraw 40
  Then the withdrawal should be rejected with "account frozen"
  And my balance should be unchanged at 100
```
