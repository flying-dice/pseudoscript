Feature: `if`/`while` conditions are boolean (LANG.md §7)

  A condition whose type is statically determinable must be `bool`. Accessor and
  call conditions (`r.isErr`, `self.ready()`) infer to `Unknown` and are not
  checked.

  Scenario: A non-bool `if` condition is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(x: number): void { if (x) { } }
      }
      """
    Then the diagnostics include "condition must be `bool`, found `number`"
    And there is exactly 1 diagnostic

  Scenario: A bool parameter condition is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(ok: bool): void { if (ok) { } }
      }
      """
    Then there are no diagnostics

  Scenario: A negation condition is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(ok: bool): void { while (!ok) { } }
      }
      """
    Then there are no diagnostics
