Feature: pds outline

  Scenario: the workspace outline lists declared nodes with their kinds
    Given a writable copy of fixture workspace "ws"
    When I run pds outline on the workspace
    Then the exit code is zero
    And stdout contains "banking::core::Banking"
    And stdout contains ""kind": "system""
    And stdout contains ""kind": "person""

  Scenario: the workspace outline lists feature blocks under their target
    Given a writable copy of fixture workspace "ws"
    When I run pds outline on the workspace
    Then the exit code is zero
    And stdout contains "banking::core::OpenAccount"
    And stdout contains ""kind": "feature""
    And stdout contains ""parent": "banking::core::Banking""
