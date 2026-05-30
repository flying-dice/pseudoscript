Feature: Field access resolves to a declared field (LANG.md §2.2, §3.4)

  A `.field` read on a value of a known same-module `data` record must name one
  of that record's fields. Black-box data, unions, cross-module types, and
  call/accessor results are not checked (their fields are not known here).

  Scenario: Accessing an undeclared field is rejected
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(it: Item): number { return it.missing }
      }
      """
    Then the diagnostics include "no field `missing` on `Item`"
    And there is exactly 1 diagnostic

  Scenario: Accessing a declared field is well-formed
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(it: Item): number { return it.id }
      }
      """
    Then there are no diagnostics

  Scenario: A field access on a black-box data type is not checked
    Given the model file:
      """
      //! example
      public data Opaque;
      public system S;
      public container C for S {
        get(it: Opaque): number { return it.anything }
      }
      """
    Then there are no diagnostics
