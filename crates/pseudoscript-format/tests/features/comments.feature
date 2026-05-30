Feature: Comments survive formatting

  Line comments, block comments, doc comments, and inner-doc comments are
  reproduced in the formatted output.

  Scenario: a line comment before a declaration
    Given the source "// a banner comment\npublic system Banking;"
    When I format it
    Then the result contains "// a banner comment"
    And the result re-parses without errors

  Scenario: a block comment before a declaration
    Given the source "/* a block note */\npublic system Banking;"
    When I format it
    Then the result contains "/* a block note */"

  Scenario: a doc comment with summary and tag
    Given the source "/// Durable store.\n/// #critical\npublic system Banking;"
    When I format it
    Then the result contains "/// Durable store."
    And the result contains "/// #critical"

  Scenario: an inner doc comment
    Given the source "//! module docs\npublic system Banking;"
    When I format it
    Then the result contains "//! module docs"

  Scenario: a doc block with extended description
    Given the source "/// Summary line.\n///\n/// Extended detail.\npublic system Banking;"
    When I format it
    Then the result contains "/// Summary line.\n///\n/// Extended detail."
