Feature: Syntax conformance (LANG.md §3–§10)

  Accept cases parse with zero error diagnostics; reject cases produce an error
  whose message matches the .reject.expected prose.

  Scenario: All syntax accept cases parse cleanly
    When I parse every syntax accept case
    Then no accept case produces an error diagnostic

  Scenario: All static fixtures parse cleanly
    When I parse every static fixture
    Then no static fixture produces an error diagnostic

  Scenario: A large, grammar-exercising model parses cleanly
    When I parse the bundled worked model
    Then it produces no error diagnostic

  Scenario: All syntax reject cases are rejected with the expected message
    When I parse every syntax reject case
    Then every reject case produces an error matching its expected message

  Scenario: A return marker without parentheses parses
    Given the source "public system S { f(): void { return Ok } }"
    Then parsing produces no error diagnostic

  Scenario: A from-composition with one source parses
    Given the source "public system S { f(): void { x: T = T from { a } } }"
    Then parsing produces no error diagnostic
