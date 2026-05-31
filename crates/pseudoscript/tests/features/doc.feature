Feature: pds doc

  Scenario: a multi-module workspace generates a site
    Given a writable copy of fixture workspace "ws"
    When I run pds doc on the workspace
    Then the exit code is zero
    And the site has "index.html"
    And the site has "style.css"
    And the site has "client.js"
    And the site has a "module/" page
    And the site index contains "Banking Fixture"

  Scenario: the loader derives module FQNs from file paths
    Given a writable copy of fixture workspace "ws"
    When I run pds doc on the workspace
    Then the exit code is zero
    And the site has "module/banking.core.html"
    And the site has "module/banking.events.html"

  Scenario: a model with a static error still generates the site
    Given a writable copy of fixture workspace "broken"
    When I run pds doc on the workspace
    Then the exit code is zero
    And the site has "index.html"
    And stderr contains "is not a system"
