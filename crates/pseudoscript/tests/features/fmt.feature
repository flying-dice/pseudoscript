Feature: pds fmt

  Scenario: formatting a well-formed model is idempotent
    Given the conformance fixture "static/0-ok-worked-example.pds"
    When I run pds fmt
    Then the exit code is zero
    And stdout is canonical and idempotent

  Scenario: --write overwrites the file with canonical text
    Given a writable copy of fixture "static/0-ok-worked-example.pds"
    When I run pds fmt --write
    Then the exit code is zero

  Scenario: a parse error exits non-zero without writing the file
    Given a writable copy of fixture "syntax/4-component-missing-for.reject"
    When I run pds fmt --write
    Then the exit code is non-zero
    And stderr contains "parse errors"
    And the file is unchanged
