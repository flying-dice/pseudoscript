---
name: spec-style
description: Enforces the terse, high-signal writing style for the PseudoScript language-spec docs. ALWAYS use this skill when creating or editing LANG.md, any file under CONFORMANCE/ (including its READMEs and .reject.expected files), or PATTERNS.md — for example adding a normative clause, writing a conformance case, documenting a pattern, or drafting a decisions/ entry. Also use it when asked to review, de-waffle, tighten, or proofread a diff touching these files, or whenever prose in them risks drifting into hedging, ceremony, future tense, rhetorical flourish, or restating a cross-reference. Trigger even if the user doesn't name the style explicitly — any writing task on these three artifacts qualifies.
---

# spec-style

Writing discipline for the PseudoScript spec docs. The goal is **lower cognitive load per read**: a reader scans a clause and knows exactly what it requires without wading through prose. Not about token cost. Not a "caveman" voice — write normal, grammatical English, just stripped of filler.

These are **hard rules**, testable against a diff. What dies: filler, hedging, ceremony, rhetorical flourish. What never dies: a load-bearing word (evaluation order, scoping, error semantics, normative force).

Three artifacts, three readers, three profiles, one shared ban-list. Read the profile for the file you're touching.

## Filler ban-list (all three artifacts)

Cut these on sight. They add reading time, not meaning.

- **Hedges:** "basically", "essentially", "simply", "just", "of course", "note that", "it's worth noting", "arguably", "more or less", "in a sense".
- **Future / speculative:** "we might", "could", "we may want to", "in the future we'd", "going forward", "eventually we". The spec describes what **is**, present tense. (Genuine deferral — `generation/` — is stated as fact: "Deferred until the spec pins a notation," not "we might define this later.")
- **Ceremony / throat-clearing:** "It is important to understand that", "As mentioned above", "In order to" (→ "to"), "the fact that", "for the purposes of".
- **Rhetorical flourish:** rephrasing a point for emphasis, the closing-line restatement, the clever inversion. If a table or rule already says it, the prose around it is flourish.
- **Self-reference about the doc:** "This section describes…", "Here we define…" — just define it.

Keep every word that fixes meaning. When unsure whether a word is filler, ask: *does removing it change what an implementer must do?* If yes, keep it.

## Describe the spec as-is

- Present tense, normative, no speculation. The language has these features now; describe them.
- **Do not state what the language does NOT do** because a thread discussed and rejected it. That "why not" is history masquerading as spec — it belongs in `decisions/`, not the doc. (e.g. a rejected implicit-coercion proposal does **not** earn a line "the language does not coerce.")
- Do not restate what a cross-reference already establishes. Cite it, move on.
- Do not compress away precision: evaluation order, scoping, error semantics, normative force stay verbatim however verbose.

## Scope boundary vs rejected-feature negation

These look identical and are opposites. Distinguish them:

- **Scope boundary — KEEP.** A non-goal of a *doc or test apparatus*: "What conformance does not test" (performance, diagnostic wording, pixel layout). This tells an implementer what they're free to vary. Load-bearing.
- **Rejected-feature negation — MOVE to `decisions/`.** A non-feature of the *language*, stated because a past thread rejected it.

Rule of thumb: *boundary of what the doc/suite covers* = keep. *Negation of a language feature driven by a past discussion* = decision record.

## Decision records

When a real fork was resolved (a feature considered and rejected, an evaluation-order choice settled, a naming convention picked over alternatives), **suggest** capturing it: "This looks like a decision worth recording — want a `decisions/00N-{name}.md` entry?" Then stop.

**Never auto-write the file.** Prompt; let the user decide. The decision record is where the rejected alternative and its reasoning live — out of the spec, preserved for context.

## Cross-reference format (pinned)

One format, for enforceability:

- **Across documents** (from CONFORMANCE/ or PATTERNS.md into the spec): `LANG.md §N` or `LANG.md §N.M` — e.g. `LANG.md §2.4`. A range is `LANG.md §3–§10`.
- **Within LANG.md**, referring to its own sections: bare `§N` / `§N.M`.

Never the bare `§6, §8` form when crossing documents — name the file. Reject diffs that mix the two.

---

## Profile: LANG.md — normative spec

Reader: an implementer. Failure mode: ambiguity. **Precision dominates.**

- **One normative statement per clause.** Don't bundle two requirements in one sentence; split them so each is independently checkable.
- **RFC 2119 keywords, not soft prose.** MUST / SHOULD / MAY / MUST NOT carry normative force. Replace "is expected to" → MUST; "should probably" → SHOULD; "you can" (when it's a permission) → MAY.
- Present tense, language as-is.
- No restating what a cross-reference establishes — `(§8)` instead of re-explaining FQN derivation.
- Never compress away precision. A long sentence that pins evaluation order is correct; shortening it to lose the order is wrong.

**Before:** "It's worth noting that, generally speaking, accessing `.value` on something that turned out to be an `Err` is basically a model error that the checker will more or less catch for you."
**After:** "Accessing `.value` on an `Err`, or `.error` on an `Ok`, is a model error (§6). The checker MUST report it."

## Profile: CONFORMANCE/ — executable contract

Reader: a person or tool pattern-matching across hundreds of cases. Failure mode: structural drift. **Exactness dominates.**

- **Structural consistency is the product.** Every case identical in shape to its siblings: same filename convention, same input/expected pairing, same section-prefix scheme. A reviewer scans by template, not by reading.
- Less prose, more filled template. Tables and conventions carry the load; sentences around them are usually flourish.
- Keep the **scope boundary** ("What conformance does not test"). It's load-bearing — see above.
- When adding a case, match the existing layout, naming, and citation format exactly. Don't invent a new field or phrasing.

**Before (real, de-waffle source):** "An implementation that passes every case here matches the spec; one that doesn't, doesn't."
**After:** *(cut)* — the table of layers and the "What a passing implementation looks like" rules already state this exactly. The inversion is flourish.

## Profile: PATTERNS.md — idioms / recipes

Reader: someone learning an idiom. Failure mode: tutorial padding burying the point. **Comprehension dominates.** This is the one place a little connective tissue earns its keep, because patterns teach.

- **Code first.** Lead with the snippet or the pattern name.
- **Then one line on when/why.** What problem it solves, when to reach for it.
- Kill tutorial padding: step-by-step narration of obvious code, "as you can see", restating the snippet in prose.
- Connective tissue between patterns is allowed where it genuinely aids comprehension — but it's connective, not decorative. One sentence, not a paragraph.

**Before:** "Now, what we want to do here is think about how we might represent the AST. As you can see, one approach that we could potentially consider is to use indices…"
**After:** "Represent the AST as flat arenas (`Vec<T>`) addressed by newtype indices (`struct NodeId(u32)`), not pointer trees. `Copy`, cache-friendly, serialisable, and it sidesteps the borrow-checker gymnastics pointer-graphs cause."

---

## Reviewing a diff

When asked to de-waffle or review a doc change, go clause by clause:

1. Cut every ban-list item.
2. Flag any future tense / speculation → present tense, or delete.
3. Flag any rejected-feature negation → suggest a `decisions/` entry, don't write it.
4. Confirm scope boundaries survived.
5. Check cross-refs match the pinned format.
6. Confirm no precision was lost (the only thing worse than waffle is a faster clause that's now wrong).
