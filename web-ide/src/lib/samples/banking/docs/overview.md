# Internet Banking System

The [C4](https://c4model.com) **container view** of the canonical Internet Banking System, after Simon Brown's example (c4model.com, [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).

A container diagram shows the containers a system is built from and how they — and the people and systems around them — talk to each other. This model has exactly that:

- **The customer** — `Customer`, who uses the single-page app and is e-mailed by the bank.
- **The Internet Banking System's containers**
  - `StaticContent` — serves and delivers the single-page app;
  - `SinglePageApp` — the in-browser client;
  - `Backend` — the JSON/HTTP API, which orchestrates everything;
  - `Database` — user accounts, credentials, access logs;
  - `StatementStore` — rendered PDF statements.
- **The systems it integrates with** — the `CoreBanking` system and the `Email` (AWS SES) system.

Each relationship in the container diagram is a **call** between nodes, so the diagram draws every edge: customer → static content → single-page app → backend → {database, statement store, core banking, e-mail}, and e-mail → customer. The model stays at the container level — the `Backend`'s callables are listed as the API surface, not taken down into components.

Open `banking.pds` and select the `InternetBanking` system (or any container) to see the container view. To go a level deeper — the component view in the original example — drill into `Backend` and add `component`s.
