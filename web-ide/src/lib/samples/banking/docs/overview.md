# Internet Banking

A canonical [C4](https://c4model.com) model in PseudoScript, taken to the **container** level — the smallest example that still shows a whole system.

A C4 container diagram shows three things, and so does this model:

- **A person** — the `Customer`.
- **The system in focus** — `InternetBanking`, split into the runnable and storage units it's built from: the `Web` server, the `SinglePageApp`, the `MobileApp`, the `Api`, and the `Database`.
- **The systems it integrates with** — the bank's core `Mainframe` and the `Email` system.

The `Api` container discloses its HTTP surface as signature-only callables tagged `#[http(...)]`, so the diagram draws the inbound request edges; the `data` records at the bottom are the shapes that API speaks.

Open `banking.pds`, then select a node on the canvas (or run **Doc**) to see the C4 view. To go a level deeper, drill into a container and add `component`s with disclosed callable bodies — that's the component level, where the flows and provenance live.
