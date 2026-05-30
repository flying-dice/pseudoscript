Feature: Static analysis conformance (LANG.md §2.4, §3.5, §4, §5.1, §6, §8)

  The checker's error set for each CONFORMANCE/static fixture must equal that
  fixture's .diagnostics golden, and well-formed models must produce nothing.

  Scenario: Every static fixture matches its golden
    When I check every static conformance fixture
    Then every fixture's error set equals its golden

  Scenario: Well-formed models yield no errors
    When I check the worked-example fixture and the worked model
    Then neither produces an error diagnostic
