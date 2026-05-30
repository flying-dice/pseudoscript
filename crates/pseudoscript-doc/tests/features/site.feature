Feature: Documentation site generation

  A resolved model graph renders to a self-contained static site: an index with
  the context diagram, a page per module, a section per node, embedded diagrams,
  and resolving cross-links — all deterministic and self-contained (LANG.md §9.3).

  Background:
    Given the banking workspace model

  Scenario: the index carries the workspace name and the context diagram
    When I render the site titled "Banking Architecture"
    Then the file "index.html" exists
    And the file "index.html" contains "Banking Architecture"
    And the file "index.html" contains an inline SVG

  Scenario: a module page lists a node with its summary and tag
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" exists
    And the file "module/banking.core.html" contains "Ledger"
    And the file "module/banking.core.html" contains "Records every posting."
    And the file "module/banking.core.html" contains "#critical"

  Scenario: a system section embeds its container diagram
    When I render the site titled "Banking Architecture"
    Then the section "banking::core::Bank" on "module/banking.core.html" contains an inline SVG

  Scenario: a node section renders its feature scenario card
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Scenarios"
    And the file "module/banking.core.html" contains "AppendPosting"
    And the file "module/banking.core.html" contains "the ledger records it"

  Scenario: a triggered callable embeds a sequence diagram on its owner section
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" contains "Sequence — Post"
    And the file "module/banking.core.html" contains an inline SVG

  Scenario: an edge endpoint renders as a resolving cross-link
    When I render the site titled "Banking Architecture"
    Then the file "module/banking.core.html" links to the anchor of "banking::core::Ledger"

  Scenario: the shared assets ship and pages reference them
    When I render the site titled "Banking Architecture"
    Then the file "style.css" exists
    And the file "app.js" exists
    And the file "index.html" references "style.css"
    And the file "index.html" references "app.js"

  Scenario: generation is deterministic
    When I render the site titled "Banking Architecture"
    And I render the site titled "Banking Architecture" again
    Then both renders are identical
