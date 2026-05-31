# Audit trail

In healthcare the audit trail is not a debugging convenience — it is a regulated, legal
artefact. Caregraph models it as an event-sourced log so it is complete by construction.

## The log is the truth

`AccessLog` (in `audit`) is an append-only aggregate. Every clinical-data access — every
`RecordViewed`, every `RecordAmended` — is appended as an `AuditEvent`. Nothing is ever
updated or deleted. `append` chains each entry to the previous one (a hash chain), so any
tampering with a past entry is detectable: the trail is tamper-evident, not merely stored.

Modelling the log as the source of truth, rather than as a side effect of some other table,
is the event-sourcing move. There is no "real" access record that the log shadows; the log
*is* the record.

## Reads are folds

Two `projection` nodes fold the same log for different audiences:

- **`PatientAccessTrail`** answers the patient's right to know: "who has accessed my
  record?" It folds log entries scoped to one patient into a readable history.
- **`AccessAnomalies`** serves compliance: it folds the log to surface unusual patterns —
  for example a clinician reading records with no care relationship behind them.

Because both are folds over the append-only log, they can be rebuilt at any time, and a new
compliance question becomes a new projection rather than a schema migration.

## Why event sourcing fits here

- **Completeness.** You cannot forget to write the audit row, because the access event *is*
  the audit row.
- **Immutability.** Append-only plus hash chaining gives the tamper evidence regulators
  expect.
- **Answerability.** New oversight questions are new folds over an unchanged truth.
