//! Conformance harness: runs the CONFORMANCE/ cases as cucumber scenarios.

use std::fs;
use std::path::{Path, PathBuf};

use cucumber::{World, given, then, when};
use pseudoscript_syntax::{Token, parse, render_tokens, tokenize};

/// Absolute path to the workspace `CONFORMANCE/` directory.
fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../CONFORMANCE")
        .canonicalize()
        .expect("CONFORMANCE directory exists")
}

/// A stable, grammar-exercising model parsed as the "large model parses clean"
/// case. Kept inline rather than reading the top-level `pseudoscript.pds`, which
/// evolves with the real architecture and is never used in tests.
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

public system Shop;

public container Web for Shop {
  /// Handles checkout.
  #[http("POST /checkout")]
  public checkout(cart: Sku[]): Result<Receipt, OrderError> {
    for (item in cart) {
      line = Result<Receipt, OrderError> from Warehouse::Pricing.price(item)
      if (line.isErr) {
        return Err(line.error)
      }
    }
    r = Result<Receipt, OrderError> from self.finalize(cart)
    return Ok(r.value)
  }

  finalize(cart: Sku[]): Result<Receipt, OrderError>;
}

public container Warehouse for Shop;

component Pricing for Warehouse {
  price(item: Sku): Result<Receipt, OrderError> {
    base = Receipt from Catalog.lookup(item).value
    receipt = Receipt from { base }
    return Ok(receipt)
  }
}

component Catalog for Warehouse {
  lookup(item: Sku): Result<Receipt, OrderError>;
}
"#;

/// Lists `*.pds` files in `dir`, sorted, excluding `.reject` siblings.
fn pds_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("readable conformance dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pds"))
        .collect();
    files.sort();
    files
}

#[derive(Debug, Default, World)]
struct ConformanceWorld {
    tokens: Vec<Token>,
    diagnostic_messages: Vec<String>,
    failures: Vec<String>,
}

// --- lexical ---------------------------------------------------------------

#[when("I render every lexical conformance case")]
fn render_lexical(world: &mut ConformanceWorld) {
    let dir = conformance_dir().join("lexical");
    for pds in pds_files(&dir) {
        let golden = pds.with_extension("tokens");
        let src = fs::read_to_string(&pds).expect("readable .pds");
        let expected = fs::read_to_string(&golden).expect("readable .tokens");
        let actual = render_tokens(&src);
        if actual != expected {
            world.failures.push(format!(
                "{}:\n--- expected ---\n{expected}\n--- actual ---\n{actual}",
                pds.file_name().unwrap().to_string_lossy()
            ));
        }
    }
}

#[then("every rendered token stream equals its golden")]
fn check_lexical(world: &mut ConformanceWorld) {
    assert!(
        world.failures.is_empty(),
        "lexical mismatches:\n{}",
        world.failures.join("\n\n")
    );
}

// cucumber step macros parse each regex capture into an owned argument and move
// it in; the signature is fixed by the framework, so the by-value params stay.
#[allow(clippy::needless_pass_by_value)]
#[given(regex = r#"^the source "(.*)"$"#)]
fn set_source(world: &mut ConformanceWorld, raw: String) {
    let src = unescape(&raw);
    world.tokens = tokenize(&src);
    let parsed = parse(&src);
    world.diagnostic_messages = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.message.clone())
        .collect();
}

#[allow(clippy::needless_pass_by_value)] // framework-fixed signature, see set_source
#[then(regex = r#"^the first token is (\w+) "(.*)"$"#)]
fn first_token(world: &mut ConformanceWorld, kind: String, lexeme: String) {
    assert_token(world, 0, &kind, &unescape(&lexeme));
}

#[allow(clippy::needless_pass_by_value)] // framework-fixed signature, see set_source
#[then(regex = r#"^token (\d+) is (\w+) "(.*)"$"#)]
fn nth_token(world: &mut ConformanceWorld, idx: usize, kind: String, lexeme: String) {
    assert_token(world, idx, &kind, &unescape(&lexeme));
}

#[allow(clippy::needless_pass_by_value)] // framework-fixed signature, see set_source
#[then(regex = r#"^there is a (\w+) token with lexeme "(.*)"$"#)]
fn has_token(world: &mut ConformanceWorld, kind: String, lexeme: String) {
    let lexeme = unescape(&lexeme);
    let found = world
        .tokens
        .iter()
        .any(|t| t.kind.name() == kind && t.text == lexeme);
    assert!(
        found,
        "no {kind} token with lexeme {lexeme:?} in {:?}",
        world.tokens
    );
}

fn assert_token(world: &ConformanceWorld, idx: usize, kind: &str, lexeme: &str) {
    let token = world
        .tokens
        .get(idx)
        .unwrap_or_else(|| panic!("token {idx} missing; have {:?}", world.tokens));
    assert_eq!(token.kind.name(), kind, "token {idx} kind");
    assert_eq!(token.text, lexeme, "token {idx} lexeme");
}

// --- syntax accept ---------------------------------------------------------

#[when("I parse every syntax accept case")]
fn parse_accept(world: &mut ConformanceWorld) {
    let dir = conformance_dir().join("syntax");
    for pds in pds_files(&dir) {
        check_clean_parse(world, &pds);
    }
}

#[when("I parse every static fixture")]
fn parse_static(world: &mut ConformanceWorld) {
    let dir = conformance_dir().join("static");
    for pds in pds_files(&dir) {
        check_clean_parse(world, &pds);
    }
}

#[when("I parse the bundled worked model")]
fn parse_root(world: &mut ConformanceWorld) {
    let parsed = parse(WORKED_MODEL);
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).collect();
    if !errors.is_empty() {
        world.failures.push(format!(
            "worked model: {} error(s): {}",
            errors.len(),
            errors
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
}

fn check_clean_parse(world: &mut ConformanceWorld, pds: &Path) {
    let src = fs::read_to_string(pds).expect("readable .pds");
    let parsed = parse(&src);
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).collect();
    if !errors.is_empty() {
        world.failures.push(format!(
            "{}: {} error(s): {}",
            pds.file_name().unwrap().to_string_lossy(),
            errors.len(),
            errors
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
}

#[then("no accept case produces an error diagnostic")]
#[then("no static fixture produces an error diagnostic")]
#[then("it produces no error diagnostic")]
fn check_no_errors(world: &mut ConformanceWorld) {
    assert!(
        world.failures.is_empty(),
        "unexpected parse errors:\n{}",
        world.failures.join("\n")
    );
}

#[then("parsing produces no error diagnostic")]
fn check_inline_no_errors(world: &mut ConformanceWorld) {
    assert!(
        world.diagnostic_messages.is_empty(),
        "unexpected errors: {:?}",
        world.diagnostic_messages
    );
}

// --- syntax reject ---------------------------------------------------------

#[when("I parse every syntax reject case")]
fn parse_reject(world: &mut ConformanceWorld) {
    let dir = conformance_dir().join("syntax");
    let mut rejects: Vec<_> = fs::read_dir(&dir)
        .expect("readable syntax dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "reject"))
        .collect();
    rejects.sort();

    for reject in rejects {
        let expected_path = reject.with_extension("reject.expected");
        let src = fs::read_to_string(&reject).expect("readable .reject");
        let expected = fs::read_to_string(&expected_path)
            .expect("readable .reject.expected")
            .trim()
            .to_owned();
        let parsed = parse(&src);
        let matched = parsed
            .diagnostics
            .iter()
            .any(|d| d.is_error() && d.message == expected);
        if !matched {
            let got: Vec<_> = parsed
                .diagnostics
                .iter()
                .filter(|d| d.is_error())
                .map(|d| d.message.as_str())
                .collect();
            world.failures.push(format!(
                "{}: expected error {:?}, got {:?}",
                reject.file_name().unwrap().to_string_lossy(),
                expected,
                got
            ));
        }
    }
}

#[then("every reject case produces an error matching its expected message")]
fn check_rejects(world: &mut ConformanceWorld) {
    assert!(
        world.failures.is_empty(),
        "reject mismatches:\n{}",
        world.failures.join("\n")
    );
}

/// Decodes the minimal escapes used in feature-file string literals.
fn unescape(raw: &str) -> String {
    raw.replace("\\\"", "\"").replace("\\n", "\n")
}

fn main() {
    futures::executor::block_on(ConformanceWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
