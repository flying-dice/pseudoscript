Feature: pds lang

  Scenario: prints the full bundled grammar suite in one stream
    When I run pds lang
    Then the exit code is zero
    And stderr is empty
    And stdout contains "# PseudoScript"
    And stdout contains "===== LANG.md ====="
    And stdout contains "===== PATTERNS.md ====="
    And stdout contains "===== CONFORMANCE/lexical/2-1-comments-and-docs.pds ====="

  Scenario: the spec alias prints the same bundle
    When I run pds spec
    Then the exit code is zero
    And stdout contains "===== LANG.md ====="

  Scenario: prints the authoring skill, signposting the lang spec
    When I run pds skill
    Then the exit code is zero
    And stderr is empty
    And stdout contains "name: pseudoscript"
    And stdout contains "pds lang"
