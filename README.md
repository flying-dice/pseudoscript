<div align="center">

# PseudoScript

### Model the intent. Implement the detail.

Architecture-as-code for the agentic era. Write your system as plain
[pseudocode](https://en.wikipedia.org/wiki/Pseudocode); the toolchain validates
the model and generates the C4 and sequence diagrams your team and your agents
build against. [Model-driven engineering](https://en.wikipedia.org/wiki/Model-driven_engineering),
reignited.

[**Open the Web IDE →**](http://ide.pdscript.dev/)  ·  [Language spec](LANG.md)  ·  [Patterns](PATTERNS.md)

</div>

---

## Where things are

| Path | What it is |
| --- | --- |
| [`LANG.md`](LANG.md) | The normative language spec (§1–§12). The source of truth. |
| [`PATTERNS.md`](PATTERNS.md) | Idioms and recipes for modelling in PseudoScript. |
| [`CONFORMANCE/`](CONFORMANCE/) | The executable contract — lexical, syntax, static, generation cases. |
| [`decisions/`](decisions/) | Architecture Decision Records, one per resolved fork. |
| [`crates/`](crates/) | The Rust toolchain: parser, checker, formatter, diagram emitter, LSP, and the `pds` binary. |
| [`web-ide/`](web-ide/) | The browser IDE — the whole toolchain compiled to WebAssembly. |
| [`web-landing/`](web-landing/) | The marketing site. |
| [`model/`](model/) | PseudoScript modelling its own design — the compiler crates, the web IDE, and the landing site. |

## The `pds` CLI

```sh
pds init          # scaffold a workspace
pds eval          # read a model from stdin, report diagnostics (great for agents)
pds check <file>  # check a file or workspace
pds doc --serve   # render the live C4 + sequence-diagram doc site
pds lang | pds skill   # print the grammar / authoring method for an LLM
```

Building a model with an agent? `pds skill` and `pds lang` teach it the method
and grammar; `pds eval` lets it check its work as it goes.
