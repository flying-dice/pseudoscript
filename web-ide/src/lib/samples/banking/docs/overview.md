# Internet Banking

A canonical [C4](https://c4model.com) model in PseudoScript, taken to the **container** level — the smallest example that still shows a whole system.

A C4 container diagram shows three things, and so does this model:

- **A person** — the `Customer`.
- **The system in focus** — `InternetBanking`, split into the runnable and storage units it's built from: the `Web` server, the `SinglePageApp`, the `MobileApp`, the `Api`, and the `Database`.
- **The systems it integrates with** — the bank's core `Mainframe` and the `Email` system.

The `Api` container's `#[http]` handlers are **disclosed**: `GetAccount` and `MakePayment` orchestrate calls to the core `Mainframe`, the `Database`, and the `Email` system. Because each is a trigger entry point, it renders as a **sequence flow** as well as contributing the container edges. The `data` records at the bottom are the shapes the API speaks.

Open `banking.pds`, then select a node for the C4 view — or select `MakePayment` to see its flow (the authorise → record → notify sequence, with its `Err` short-circuit). To go deeper, drill into a container and add `component`s to model the level below.
