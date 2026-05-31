# Consent & access

The central question in Caregraph is "may this clinician see this record, right now?"
The model answers it in exactly one place: `AccessPolicy`.

## One decision point

`ClinicalRecord.read` does not decide access itself. It defers to `AccessPolicy.permits`,
a `policy` node that is the single source of access truth. Routing every read through one
policy means the rule is testable, auditable, and impossible to bypass by accident — there
is no second code path that "just reads the record".

The policy is **deny by default**. It permits a read only when one of two things holds:

- **Active consent** — the patient has granted a `Consent` to this clinician, and it has
  not been withdrawn.
- **A care relationship** — there is an `in_progress` `Appointment` between them. This is
  the "break-glass" case: a clinician actively treating a patient can see the record even
  before paperwork catches up.

## Consent is live

`Consent` is its own aggregate with a tiny lifecycle: `active → withdrawn`. Withdrawal
takes effect immediately, because `AccessPolicy` reads consent state at the moment of each
read rather than caching a decision. A patient who revokes access locks out the next read,
not the next session.

## Reads and amendments are recorded

Both `read` and `amend` emit events (`RecordViewed`, `RecordAmended`). Those events feed the
`audit` context's `AccessLog`. Access control and audit are two halves of the same promise:
the policy decides *whether* access happens, the log records *that* it happened. Neither is
complete without the other — see the Audit Trail page.
