Feature: Reserved words are not identifiers (LANG.md §2.3, ADR-012)

  A keyword, a primitive type name, or `Result`/`Option` must not be used as a
  declared identifier. (A keyword in a strict-name position — a `data`/node/
  callable name — is a parse error; this static check covers the lenient
  positions — fields, parameters, variants — and a primitive used as any name.)

  Scenario: A primitive type name as a `data` name is rejected
    Given the model file:
      """
      //! example
      data string;
      """
    Then the diagnostics include "reserved word `string` cannot be used as an identifier"
    And there is exactly 1 diagnostic

  Scenario: A reserved word as a field name is rejected
    Given the model file:
      """
      //! example
      data Edge { from: number }
      """
    Then the diagnostics include "reserved word `from` cannot be used as an identifier"
    And there is exactly 1 diagnostic

  Scenario: A reserved word as a parameter name is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(data: number): void { }
      }
      """
    Then the diagnostics include "reserved word `data` cannot be used as an identifier"
    And there is exactly 1 diagnostic

  Scenario: Ordinary names are well-formed
    Given the model file:
      """
      //! example
      data Edge { source: number, target: number }
      public system S;
      public container C for S {
        run(payload: number): void { }
      }
      """
    Then there are no diagnostics
