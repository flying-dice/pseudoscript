//! Smoke render: build the banking example graph, render the Svelte site, and
//! write it to a directory passed as the first argument. For manual inspection.

use pseudoscript_doc::{DocConfig, try_render_site};
use pseudoscript_model::{WorkspaceModule, graph};

const MODEL: &str = r#"//! banking::core

/// A retail banking customer.
public person Customer;

/// A double-entry posting.
public data Posting { id: number, amount: number }

/// The core banking platform.
public system Bank;

/// Records every posting.
public container Ledger for Bank {
  /// Append a posting to the ledger.
  public Append(posting: Posting): number;
}

/// A balanced posting is recorded.
feature AppendPosting for Ledger {
  given "a balanced posting"
  when "it is appended"
  then "the ledger records it"
}

/// Public-facing API surface.
public container Api for Bank {
  component Validator for Api { check(posting: Posting): number; }
  /// Post a new entry.
  #[manual]
  public Post(posting: Posting): number {
    n = Ledger.Append(posting)
    return n
  }
}
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/svelte-smoke".to_owned());
    let g = graph(&[WorkspaceModule::new("banking::core", MODEL)]);
    let config = DocConfig {
        name: "Banking Architecture".to_owned(),
        ..DocConfig::default()
    };
    let site = try_render_site(&g, &config)?;
    for file in &site.files {
        let dest = std::path::Path::new(&out).join(&file.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &file.contents)?;
    }
    println!("wrote {} files to {out}", site.files.len());
    Ok(())
}
