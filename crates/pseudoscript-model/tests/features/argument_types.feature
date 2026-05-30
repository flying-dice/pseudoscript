Feature: Call argument types (LANG.md §5.1)

  An argument whose type is inferable, passed to a resolvable same-module
  callable, must match the callee's parameter type (a union variant satisfies
  its union). Generic (`Result`/`Option`) parameters and `Unknown` arguments are
  not checked.

  Scenario: An argument of the wrong type is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { self.take("x") }
        take(n: number): void { }
      }
      """
    Then the diagnostics include "argument 1: expected `number`, found `string`"
    And there is exactly 1 diagnostic

  Scenario: Matching argument types are well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { self.take(5) }
        take(n: number): void { }
      }
      """
    Then there are no diagnostics

  Scenario: An array argument matches an array parameter
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        run(a: Item, b: Item): void { self.take(Item[] from { a, b }) }
        take(xs: Item[]): void { }
      }
      """
    Then there are no diagnostics
