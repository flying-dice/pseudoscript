//! BDD harness for the canonical formatter.
//!
//! Scenarios live in `tests/features/`. Sample `.pds` inputs are located via
//! `CARGO_MANIFEST_DIR` so the suite runs from any working directory.

use std::fs;
use std::path::{Path, PathBuf};

use cucumber::{World, given, then, when};
use pseudoscript_format::format;
use pseudoscript_syntax::{Token, parse, tokenize};

/// A stable, grammar-exercising model used for the idempotency and
/// semantics-preservation checks. Kept inline rather than reading the top-level
/// `pseudoscript.pds`, which evolves with the real architecture and is never
/// used in tests.
const WORKED_MODEL: &str = r#"//! shop — a worked model exercising the grammar.

/// A retail customer.
/// #external
public person Customer {
  /// Places an order.
  #[manual]
  Order(item: Sku): Result<Receipt, OrderError>;
}

public data Sku { code: string }
public data Receipt { total: number }
public data OrderError { reason: string }

data OrderPlaced { id: string }
data OrderEvent =
  | OrderPlaced
  | OrderCancelled { id: string, reason: string }

alias Store = shop::Warehouse;

public system Shop;

public container Web for Shop {
  /// Handles checkout.
  #[http("POST /checkout")]
  public checkout(cart: Sku[]): Result<Receipt, OrderError> {
    for (item in cart) {
      line: Result<Receipt, OrderError> = Warehouse::Pricing.price(item)
      if (line.isErr) {
        return Err(line.error)
      }
    }
    r: Result<Receipt, OrderError> = self.finalize(cart)
    return Ok(r.value)
  }

  finalize(cart: Sku[]): Result<Receipt, OrderError>;
}

public container Warehouse for Shop;

component Pricing for Warehouse {
  price(item: Sku): Result<Receipt, OrderError> {
    base: Receipt = Catalog.lookup(item).value
    receipt: Receipt = Receipt from { base }
    return Ok(receipt)
  }
}

component Catalog for Warehouse {
  lookup(item: Sku): Result<Receipt, OrderError>;
}
"#;

/// Resolves a workspace-relative path from the crate manifest dir.
fn workspace_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(rel)
        .canonicalize()
        .unwrap_or_else(|e| panic!("resolve {rel}: {e}"))
}

/// The meaningful (non-trivia) token stream as `(kind, lexeme)` pairs. Comments
/// and blank-line gaps are excluded by `tokenize`, so two strings that differ
/// only in layout produce identical vectors.
fn token_signature(src: &str) -> Vec<(&'static str, String)> {
    tokenize(src)
        .iter()
        .map(|t: &Token| (t.kind.name(), t.text.clone()))
        .collect()
}

#[derive(Debug, Default, World)]
struct FormatWorld {
    /// The current input source under test.
    source: String,
    /// The formatted output of `source`, once computed.
    formatted: Option<String>,
}

// --- input -----------------------------------------------------------------

#[given(regex = r#"^the source "(.*)"$"#)]
// cucumber's regex-capture macro requires an owned `FromStr` param type; `&str` won't compile.
#[allow(clippy::needless_pass_by_value)]
fn given_inline_source(world: &mut FormatWorld, raw: String) {
    world.source = unescape(&raw);
    world.formatted = None;
}

#[given(regex = r"^the sample file (\S+)$")]
// cucumber's regex-capture macro requires an owned `FromStr` param type; `&str` won't compile.
#[allow(clippy::needless_pass_by_value)]
fn given_sample_file(world: &mut FormatWorld, rel: String) {
    world.source = fs::read_to_string(workspace_path(&rel)).expect("readable sample file");
    world.formatted = None;
}

#[given("the bundled worked model")]
fn given_worked_model(world: &mut FormatWorld) {
    WORKED_MODEL.clone_into(&mut world.source);
    world.formatted = None;
}

// --- actions ---------------------------------------------------------------

#[when("I format it")]
fn when_format(world: &mut FormatWorld) {
    let out = format(&world.source).expect("source formats without parse errors");
    world.formatted = Some(out);
}

// --- assertions ------------------------------------------------------------

#[then("formatting is idempotent")]
fn then_idempotent(world: &mut FormatWorld) {
    let once = format(&world.source).expect("first format");
    let twice = format(&once).expect("second format");
    assert_eq!(
        once, twice,
        "format is not idempotent for: {}",
        world.source
    );
}

#[then("the result re-parses without errors")]
fn then_reparses(world: &mut FormatWorld) {
    let out = world.formatted.clone().expect("formatted output");
    let parsed = parse(&out);
    let errors: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        errors.is_empty(),
        "reformatted output has parse errors {errors:?}:\n{out}"
    );
}

#[then("the result preserves the meaningful tokens")]
fn then_preserves_tokens(world: &mut FormatWorld) {
    let out = world.formatted.clone().expect("formatted output");
    let before = token_signature(&world.source);
    let after = token_signature(&out);
    assert_eq!(
        before, after,
        "token stream changed by formatting.\ninput:\n{}\noutput:\n{out}",
        world.source
    );
}

#[then(regex = r#"^the result contains "(.*)"$"#)]
// cucumber's regex-capture macro requires an owned `FromStr` param type; `&str` won't compile.
#[allow(clippy::needless_pass_by_value)]
fn then_contains(world: &mut FormatWorld, needle: String) {
    let needle = unescape(&needle);
    let out = world.formatted.clone().expect("formatted output");
    assert!(
        out.contains(&needle),
        "expected output to contain {needle:?}:\n{out}"
    );
}

#[then(regex = r#"^the result equals "(.*)"$"#)]
// cucumber's regex-capture macro requires an owned `FromStr` param type; `&str` won't compile.
#[allow(clippy::needless_pass_by_value)]
fn then_equals(world: &mut FormatWorld, expected: String) {
    let expected = unescape(&expected);
    let out = world.formatted.clone().expect("formatted output");
    assert_eq!(out, expected, "formatted output mismatch");
}

// --- helpers ---------------------------------------------------------------

/// Expands `\n`, `\t`, `\"`, and `\\` in a Gherkin string argument.
fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') | None => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn main() {
    futures::executor::block_on(FormatWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
