Feature: pds check

  Scenario: a well-formed model exits zero with no errors
    Given the conformance fixture "static/0-ok-worked-example.pds"
    When I run pds check
    Then the exit code is zero
    And stderr is empty

  Scenario: a static error exits non-zero and reports the message
    Given the conformance fixture "static/7-rebind-rejected.pds"
    When I run pds check
    Then the exit code is non-zero
    And stderr contains "re-binding of `r`: bindings are single-assignment"

  Scenario: a parse error exits non-zero
    Given the conformance fixture "syntax/4-component-missing-for.reject"
    When I run pds check
    Then the exit code is non-zero

  Scenario: workspace diagnostics carry the file and source location
    Given a workspace whose module has a static error
    When I run pds check
    Then the exit code is non-zero
    And stderr contains "bad.pds:1:24: error: parent `repro::Missing` does not resolve"
