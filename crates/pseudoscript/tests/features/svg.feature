Feature: pds svg

  Scenario: the context view renders a self-contained SVG
    Given a writable copy of fixture workspace "ws"
    When I run pds svg for the context view
    Then the exit code is zero
    And stdout contains "<svg xmlns="

  Scenario: an unknown symbol exits non-zero with a clear message
    Given a writable copy of fixture workspace "ws"
    When I run pds svg for an unknown symbol
    Then the exit code is non-zero
    And stderr contains "no node named"
