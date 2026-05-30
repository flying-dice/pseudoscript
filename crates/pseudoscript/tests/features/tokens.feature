Feature: pds tokens

  Scenario: the token stream matches the lexical golden
    Given the conformance fixture "lexical/2-3-keywords-vs-idents.pds"
    When I run pds tokens
    Then the exit code is zero
    And stdout equals the golden "lexical/2-3-keywords-vs-idents.tokens"
