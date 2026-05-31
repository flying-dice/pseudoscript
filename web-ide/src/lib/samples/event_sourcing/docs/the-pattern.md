# Event Sourcing

Split a holiday with friends and the awkward question is never "what's the balance?" — it's "wait, who paid for the boat, and when?" A number can't answer that. A list of what actually happened can. Event sourcing is the decision to keep the list and compute the number.

## The problem

**Tallyho** tracks shared expenses. A **Member** logs a cost; later, everyone wants to know who owes what. The obvious design stores a balance per member and edits it on each expense: `balance += amount`. It's compact and the balance is instant to read.

It's also lossy. Once you overwrite `balance`, the individual expenses that produced it are gone. You can't audit it — "the maths says I owe £40, prove it" has no answer. You can't undo a fat-fingered entry without guessing what the balance *was*. You can't add a new view later ("expenses per day") because the raw data that view needs was thrown away the moment you folded it into a total. The current value is all you kept, and the current value is the least interesting thing.

## The pattern

Event sourcing never stores the balance. It stores the *events* and derives the balance.

In Tallyho, `Member.addExpense(amount)` calls `Tallyho::Journal.record`. `Journal` (exposed at `POST /expenses`) does one thing: it appends. It calls `Events.append(Expense from { member, amount })` — it never edits a total. `Events` is the immutable, append-only log: `append` adds, `history(member)` returns every event for a member *in order*, for replay. Nothing in `Events` updates in place.

The balance lives in `Balances`. It listens for each `Expense` (`#[onevent(Expense)]`) and calls `self.fold` — folding one event into the running total. And critically, `Balances.rebuild(member)` can reconstruct that total from scratch by replaying `Events.history` from the beginning. The current balance is just a cache of a fold; the log is the truth.

The `StateIsTheLog` feature states it plainly: a balance "is computed as the fold over those events," a report "can be rebuilt by replaying the log," but "no event is ever edited or deleted in place." State is a function of history, not a thing you mutate.

## When to use it

When history *is* the requirement — audit, regulatory traceability, "how did we get here," temporal queries, or the ability to derive new views from old facts. It shines wherever an immutable record matters more than write convenience, and pairs naturally with CQRS read models.

## When to avoid it

When you only ever need the latest value and history is noise. A settings toggle or a session store doesn't want a permanent event log — it wants a row you overwrite. Event sourcing also raises the floor of complexity: don't pay it for CRUD.

## Trade-offs

You gain a perfect audit trail, free undo, and the freedom to rebuild any view from raw facts. You pay with replay cost, the discipline of versioning events forever, and projections that are eventually consistent with the log.
