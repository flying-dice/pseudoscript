# Caregraph

Caregraph is a clinical-records platform. It schedules encounters between patients and
clinicians, holds the medical record, and — above all — controls and audits every access
to that record. In healthcare, *who saw what, when, and why* is as important as the data.

## The problem

Clinical software is a privacy and correctness problem first, a workflow second:

- **Access is the hard part.** A record must be readable by the right clinician at the
  right moment and by no one else. Consent can be granted and withdrawn at any time.
- **The audit trail is a legal artefact.** Every access must be recorded completely and
  tamper-evidently. Patients have a right to see who touched their record.
- **Care spans clinicians.** Referrals move a patient between providers, and that handoff
  must carry consent with it.
- **Encounters bill.** A completed appointment becomes an insurance claim that can be
  coded, submitted, paid, or denied — each a failure path that must reconcile cleanly.

## The contexts

- **scheduling** — patients, clinicians, and the `Appointment` state machine.
- **records** — the `ClinicalRecord`, patient `Consent`, and the `AccessPolicy` decision point.
- **audit** — the event-sourced `AccessLog` and its compliance projections.
- **referrals** — the `Referral` workflow that hands care to a specialist, gated by consent.
- **claims** — the `ClaimSaga` that bills a completed appointment to an insurer.

## How to read this model

Start with **records** — `AccessPolicy` is the conceptual centre, and everything orbits the
question it answers. Then read **audit** (how access is recorded) and **scheduling** (how an
encounter creates the care relationship that access depends on).

## Patterns on display

- **Policy** — `AccessPolicy` is the single, deny-by-default access decision.
- **State machine** — `Appointment` and `Referral` are guarded lifecycles.
- **Event sourcing** — `AccessLog` is the append-only, tamper-evident audit truth.
- **Projection** — `PatientAccessTrail` and `AccessAnomalies` fold the audit log.
- **Saga** — `ClaimSaga` bills an encounter with per-step compensation.
