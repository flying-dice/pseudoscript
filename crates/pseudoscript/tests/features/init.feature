Feature: pds init bootstraps a workspace

  Scenario: init writes a manifest and a starter module
    Given an empty workspace directory
    When I run pds init
    Then the exit code is zero
    And the workspace contains "pds.toml"
    And the workspace contains "main.pds"

  Scenario: the generated module checks clean
    Given an empty workspace directory
    When I run pds init
    And I run pds check on the generated module
    Then the exit code is zero
    And stderr is empty

  Scenario: the generated module checks clean by bare name from the workspace
    Given an empty workspace directory
    When I run pds init
    And I run pds check on the bare module name from the workspace
    Then the exit code is zero
    And stderr is empty

  Scenario: init refuses to overwrite an existing workspace
    Given an empty workspace directory
    When I run pds init
    And I run pds init
    Then the exit code is non-zero
    And stderr contains "already exists"
