# Lexical conformance

Tokenisation cases for `LANG.md` §2. Each `name.pds` pairs with a `name.tokens`
golden: the exact token stream a conforming lexer must produce, **one token per
line**, in source order:

```
KIND@line:col "lexeme"
```

- `line` / `col` are 1-based; `col` is the column of the token's first character.
- `lexeme` is the raw source slice, quoted. For doc tokens it is the doc text
  with the marker and surrounding horizontal whitespace stripped (see below).
- Comments are **discarded** — `//` line and `/* */` block comments emit no
  token. Doc comments (`///`, `//!`) do emit tokens.

`LANG.md` §2 describes the lexical structure in prose but does not enumerate
token-class names; this file defines the taxonomy the goldens assert against.

## Token kinds

**Keywords** (§2.3) — one kind each:

```
KW_SYSTEM  KW_CONTAINER  KW_COMPONENT  KW_PERSON
KW_DATA    KW_FOR        KW_ALIAS      KW_FROM
KW_PUBLIC  KW_SELF
KW_RETURN  KW_OK    KW_ERR
KW_IF      KW_ELSE  KW_WHILE  KW_IN
KW_TRUE    KW_FALSE
```

Bools tokenise as `KW_TRUE` / `KW_FALSE`; the parser builds a `Bool` literal
from them (§10).

**Identifiers** — `IDENT`. Primitive type names (`number`, `string`, `bool`,
`datetime`, `uuid`, `void`, §3.1) and `Result` are **not** keyword tokens, so
the lexer emits them as `IDENT` and the parser classifies them in type position.
They are reserved (§2.3) — using one as a declaration name is a static error,
not a lexical one. Identifiers are matched greedily: `forwarder` is one `IDENT`,
never `KW_FOR` + `warder`.

**Punctuation & operators:**

```
COLONCOLON  ::        DOT         .         COLON     :
SEMI        ;         COMMA       ,
LBRACE      {         RBRACE      }
LPAREN      (         RPAREN      )
LBRACKET    [         RBRACKET    ]
EQ          =         PIPE        |
QUESTION    ?         LANGLE      <         RANGLE    >
BANG        !
```

`QUESTION` (`?`) is lexed but unused by the grammar (§3.3 has no optionality
marker); a `?` in type position is a parse error, not a lexical one.

**Literals** — `STRING` (double-quoted, lexeme includes the quotes),
`NUMBER` (digit run).

**Annotations** (§2.1, §2.4):

- `DOC` — a `///` line's text. Lexeme = content after `///`, leading/trailing
  horizontal whitespace stripped.
- `INNER_DOC` — a `//!` line's text, same stripping.
- `TAG` — a `#name` occurring on a `///` line (§2.4). Lexeme includes the `#`.
  A `///` line emits its prose as `DOC` segments and each `#name` as a separate
  `TAG`, in source order.
- `HASH_LBRACKET` — `#[`, opening a macro. The macro's name, arguments, and
  closing `]` (`RBRACKET`) tokenise as ordinary `IDENT` / punctuation / literal
  tokens.
- A `#` that is neither a `///`-line tag nor the start of `#[` is literal prose
  (§2.4); inside a `STRING` it is simply part of the `STRING` lexeme.

## Cases

| Case | Rule |
| --- | --- |
| `2-1-comments-and-docs` | §2.1 — `//`/`/* */` discarded; `//!` → `INNER_DOC`; `///` → `DOC`. |
| `2-2-paths-colon-vs-dot` | §2.2 — `::` (`COLONCOLON`) walks a path; `.` (`DOT`) accesses/invokes. |
| `2-3-keywords-vs-idents` | §2.3 — keywords vs. greedily-matched identifiers that merely contain a keyword. |
| `2-3-result-self-keywords` | §2.3 — `self`/`Ok`/`Err`/`true`/`false` as keywords; `Result` and primitives as `IDENT`. |
| `2-4-hash-disambiguation` | §2.4 — `#name` tag in a doc, `#[` macro open, and a literal `#` inside a string. |
