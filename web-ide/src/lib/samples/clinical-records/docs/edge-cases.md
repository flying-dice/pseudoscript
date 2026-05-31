# Edge cases

The interesting failures, and how the model handles them.

## Consent withdrawn between reads

A patient withdraws consent while a clinician has the record open. `AccessPolicy.permits`
reads `Consent` state at the moment of *each* read, never caching a decision, so the next
`ClinicalRecord.read` is denied immediately. Withdrawal is effective on the next access, not
the next login.

## Break-glass access without consent

A patient arrives unconscious; no consent exists. The treating clinician still needs the
record. `AccessPolicy` permits a read when there is an `in_progress` `Appointment` between
them — the care relationship stands in for consent. Crucially, that read still emits
`RecordViewed`, so break-glass access is fully audited and reviewable after the fact.

## Referral without consent to share

A clinician tries to refer a patient to a specialist the patient hasn't agreed to share
with. `Referral.request` guards on an active `Consent` for the receiving clinician, so the
referral is blocked at creation — the record is never exposed to an unconsented party.

## Patient no-show, then a claim

`Appointment.mark_no_show` guards `booked → no_show`. Because `Claim.open` requires a
*completed* appointment, a no-show can never produce a clinical claim — though it may
trigger a separate no-show fee outside this model's scope.

## Claim denied after submission

The `ClaimSaga` submits to the payer and the payer denies. Each step names its compensation:
denial routes to appeal or write-off, codes can be voided, and eligibility failures fall
back to patient-responsible. The saga is idempotent on claim id, so a resubmission after a
transient payer outage never bills twice.

## Tampering with the audit log

Someone alters a past `AccessLog` entry. Because `append` hash-chains each entry to the
previous, the chain no longer validates from that point on — the tampering is detectable,
which is the whole point of an event-sourced, append-only audit trail.
