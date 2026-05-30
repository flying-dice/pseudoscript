Feature: generation conformance

  The Scene IR projected from each CONFORMANCE/generation fixture serialises to
  the exact text its sibling goldens pin (CONFORMANCE/generation/README.md).

  Scenario: every scene golden is reproduced byte-for-byte
    When I project every generation fixture against its scene goldens
    Then every projected scene equals its golden byte-for-byte
