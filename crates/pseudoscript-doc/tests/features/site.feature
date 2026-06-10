Feature: Svelte documentation site generation

  A resolved model graph renders, through the embedded Svelte SSR engine, to a
  self-contained static site: an index with the context diagram and stats, a
  page per module, a section per node, the universe and health pages, and a
  static search index. Every diagram is server-rendered inline SVG under the
  adaptive palette; the client scripts only progressively enhance — all
  deterministic and self-contained (LANG.md §9.3).

  Background:
    Given the banking workspace model

  Scenario: the index carries the workspace name and the context diagram
    When I render the site titled "Banking Architecture"
    Then the file "index.html" exists
    And the file "index.html" contains "Banking Architecture"
    And the file "index.html" embeds a "c4" diagram

  Scenario: a module page lists a node with its summary and tag
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" exists
    And the file "module/banking.core.html" contains "Ledger"
    And the file "module/banking.core.html" contains "Records every posting."
    And the file "module/banking.core.html" contains "#critical"

  Scenario: a system section embeds its container diagram
    When I render the site titled "Banking Architecture"
    Then the section "banking::core::Bank" on "module/banking.core.html" embeds a "c4" diagram

  Scenario: a node section renders its feature scenario card
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Scenarios"
    And the file "module/banking.core.html" contains "AppendPosting"
    And the file "module/banking.core.html" contains "the ledger records it"

  Scenario: a data section embeds its entity diagram
    When I render the site titled "Banking Architecture"
    Then the section "banking::core::Posting" on "module/banking.core.html" embeds an "entity" diagram
    And the file "module/banking.core.html" contains "Entity diagram"

  Scenario: a feature scenario card embeds its flow diagram
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Flow — AppendPosting"
    And the section "banking::core::Ledger" on "module/banking.core.html" embeds a "flow" diagram

  Scenario: a triggered callable embeds a sequence diagram on its owner section
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Sequence "
    And the file "module/banking.core.html" embeds a "sequence" diagram

  Scenario: every diagram figure carries inline SVG
    When I render the site titled "Banking Architecture"
    Then every diagram figure on "module/banking.core.html" contains inline SVG
    And every diagram figure on "index.html" contains inline SVG

  Scenario: an edge endpoint renders as a resolving cross-link
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" links to the anchor of "banking::core::Ledger"

  Scenario: the shared assets ship and pages reference them
    When I render the site titled "Banking Architecture"
    Then the file "style.css" exists
    And the file "client.js" exists
    And the file "universe.js" exists
    And the file "index.html" references "style.css"
    And the file "index.html" references "client.js"

  Scenario: page data is embedded only on the universe page
    When I render the site titled "Banking Architecture"
    Then the file "index.html" does not contain "window.__DATA__"
    And the file "module/banking.core.html" does not contain "window.__DATA__"
    And the file "universe.html" contains "window.__DATA__"
    And the file "universe.html" references "universe.js"

  Scenario: the universe page renders its fallback content
    When I render the site titled "Banking Architecture"
    Then the file "universe.html" exists
    And the file "universe.html" contains "data-universe"
    And the file "universe.html" contains "banking::core::Bank"

  Scenario: the health page reports the clean model
    When I render the site titled "Banking Architecture"
    Then the file "health.html" exists
    And the file "health.html" contains "Architecture health"
    And the file "health.html" contains "No findings."

  Scenario: the static search index carries the model's symbols
    When I render the site titled "Banking Architecture"
    Then the file "search-index.js" exists
    And the file "search-index.js" contains "window.__PDS_SEARCH__"
    And the file "search-index.js" contains "banking::core::Ledger"

  Scenario: every page ships the chrome and classic scripts
    When I render the site titled "Banking Architecture"
    Then every page contains "skip-link"
    And every page contains "theme-toggle"
    And every page loads the client script as a classic deferred script

  Scenario: generation is deterministic
    When I render the site titled "Banking Architecture"
    And I render the site titled "Banking Architecture" again
    Then both renders are identical
