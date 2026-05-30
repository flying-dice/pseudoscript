Feature: Lexical conformance (LANG.md §2)

  Every CONFORMANCE/lexical/*.pds must render to its sibling .tokens golden,
  byte-for-byte, via render_tokens.

  Scenario: All lexical conformance cases match their token goldens
    When I render every lexical conformance case
    Then every rendered token stream equals its golden

  Scenario: A greedy identifier containing a keyword stays one IDENT
    Given the source "systematic = forwarder.inputs"
    Then the first token is IDENT "systematic"

  Scenario: A bare keyword tokenises as a keyword
    Given the source "for"
    Then the first token is KW_FOR "for"

  Scenario: Result and primitives are identifiers, not keywords
    Given the source "Result number string"
    Then token 0 is IDENT "Result"
    And token 1 is IDENT "number"
    And token 2 is IDENT "string"

  Scenario: A hash inside a string is part of the string lexeme
    Given the source "#[http(\"GET /a#b\")]"
    Then there is a STRING token with lexeme "\"GET /a#b\""

  Scenario: A doc line splits prose and a tag in source order
    Given the source "/// Durable store. #critical"
    Then token 0 is DOC "Durable store."
    And token 1 is TAG "#critical"

  Scenario: Double colon and dot are distinct tokens
    Given the source "a::b.c"
    Then token 0 is IDENT "a"
    And token 1 is COLONCOLON "::"
    And token 2 is IDENT "b"
    And token 3 is DOT "."
    And token 4 is IDENT "c"
