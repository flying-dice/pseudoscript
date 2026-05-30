Feature: `for` iterates an array (LANG.md §7.3, ADR-014)

  `for (x in Expr)` requires `Expr` to be an array type `T[]`. When the
  iterable's type is statically determinable — a parameter, a literal, a marker,
  or a `from` composition — a non-array MUST be rejected. Bindings, calls, and
  field accesses are not inferred and are left unchecked.

  Scenario: Iterating a non-array parameter is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        each(x: number): void { for (i in x) { } }
      }
      """
    Then the diagnostics include "`for` iterates a non-array type `number`"
    And there is exactly 1 diagnostic

  Scenario: Iterating an array parameter is well-formed
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        each(xs: Item[]): void { for (i in xs) { } }
      }
      """
    Then there are no diagnostics

  Scenario: Iterating a `from` composition is rejected
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        each(x: number): void { for (i in Item from { x }) { } }
      }
      """
    Then the diagnostics include "`for` iterates a non-array type `Item`"
    And there is exactly 1 diagnostic
