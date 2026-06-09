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

  Scenario: the data view renders an entity diagram (LANG.md §9.4)
    Given a writable copy of fixture workspace "ws"
    When I run pds svg for the "data" view of "banking::events::NotFound"
    Then the exit code is zero
    And stdout contains "<svg xmlns="

  Scenario: the feature view renders a flow diagram (LANG.md §9.5)
    Given a writable copy of fixture workspace "ws"
    When I run pds svg for the "feature" view of "banking::core::OpenAccount"
    Then the exit code is zero
    And stdout contains "<svg xmlns="

  Scenario: an unknown view exits non-zero naming the accepted views
    Given a writable copy of fixture workspace "ws"
    When I run pds svg for an unknown view
    Then the exit code is non-zero
    And stderr contains "expected context/container/component/sequence/data/feature"
