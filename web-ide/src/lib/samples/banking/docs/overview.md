# Internet Banking System

The [C4](https://c4model.com) model of the canonical Internet Banking System, down to the **component** level, after Simon Brown's example (c4model.com, [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).

Each relationship is a **call** between nodes, so every diagram draws its own edges. Select a node to choose the view.

## Containers

A container diagram shows the containers a system is built from and how they — and the people and systems around them — talk to each other:

- **The customer** — `Customer`, who uses the single-page app and is e-mailed by the bank.
- **The Internet Banking System's containers**
  - `StaticContent` — serves and delivers the single-page app;
  - `SinglePageApp` — the in-browser client;
  - `Backend` — the JSON/HTTP API, which orchestrates everything;
  - `Database` — user accounts, credentials, access logs;
  - `StatementStore` — rendered PDF statements.
- **The systems it integrates with** — the `CoreBanking` system and the `Email` (AWS SES) system.

Select the `InternetBanking` system (or any container) for its container view: customer → static content → single-page app → backend → {database, statement store, core banking, e-mail}, and e-mail → customer.

## Components

`Backend` is decomposed into the seven Spring components of the original example. Select `Backend` for its component view — the single-page app, database, statement store, core banking, and e-mail systems frame it as external boxes:

- `SigninApi`, `AccountsSummaryApi`, `StatementApi` — the Spring MVC API endpoints the single-page app calls;
- `SecurityComponent` — validates credentials and tokens against the database;
- `CoreBankingSystemAdapter` — wraps the Core Banking System's API;
- `StatementComponent` — reads PDF statements from the statement store;
- `EmailComponent` — sends e-mail via AWS SES.

Edge labels are the callee method names (PseudoScript's relationship label); each component's technology — Spring MVC, Spring Bean — is in its `///` description.
