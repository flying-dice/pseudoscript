Feature: Svelte documentation site generation

  A resolved model graph renders, through the embedded Svelte SSR engine, to a
  self-contained static site: an index with the context diagram, a page per
  module, a section per node, the page chrome and text server-rendered, and the
  diagrams emitted as client-mounted islands carrying their scene data — all
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
    Then the section "banking::core::Posting" on "module/banking.core.html" embeds a "svg" diagram
    And the file "module/banking.core.html" contains "Entity diagram"

  Scenario: a feature scenario card embeds its flow diagram
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Flow — AppendPosting"
    And the section "banking::core::Ledger" on "module/banking.core.html" embeds a "svg" diagram

  Scenario: a triggered callable embeds a sequence diagram on its owner section
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Sequence "
    And the file "module/banking.core.html" embeds a "sequence" diagram

  Scenario: an edge endpoint renders as a resolving cross-link
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" links to the anchor of "banking::core::Ledger"

  Scenario: the shared assets ship and pages reference them
    When I render the site titled "Banking Architecture"
    Then the file "style.css" exists
    And the file "client.js" exists
    And the file "index.html" references "style.css"
    And the file "index.html" references "client.js"

  Scenario: the page embeds hydration data
    When I render the site titled "Banking Architecture"
    Then the file "index.html" contains "window.__DATA__"

  Scenario: generation is deterministic
    When I render the site titled "Banking Architecture"
    And I render the site titled "Banking Architecture" again
    Then both renders are identical
