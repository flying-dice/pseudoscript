Feature: LSP diagnostics mapping

  Scenario: well-formed source produces no diagnostics
    Given the source fixture "0-ok-worked-example.pds"
    When I compute LSP diagnostics
    Then there are 0 diagnostics

  Scenario: a re-bind static error maps to one error diagnostic
    Given the source fixture "7-rebind-rejected.pds"
    When I compute LSP diagnostics
    Then there are 1 diagnostics
    And every diagnostic has error severity
    And a diagnostic message contains "re-binding of `r`"

  Scenario: a wrong-accessor error maps to an error diagnostic
    Given the source fixture "6-result-wrong-accessor.pds"
    When I compute LSP diagnostics
    Then every diagnostic has error severity
    And a diagnostic message contains "`.value` read on an `Err` branch"

  Scenario: an unknown macro maps to an error diagnostic
    Given the source fixture "2-unknown-macro.pds"
    When I compute LSP diagnostics
    Then a diagnostic message contains "unknown macro `retry`"

  Scenario: diagnostic ranges are 0-based
    Given the source fixture "7-rebind-rejected.pds"
    When I compute LSP diagnostics
    Then a diagnostic starts on 0-based line 12
