Feature: Bare same-node calls resolve to a same-node callable (LANG.md §5.1, ADR-041)

  A same-node callable is invoked by a bare call `Name(args)`. A bare call
  naming no callable of the enclosing node MUST be rejected. The removed `self.`
  qualifier is diagnosed.

  Scenario: A same-node call to an undeclared callable is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { missing() }
      }
      """
    Then the diagnostics include "`missing` does not name a callable of `C`"
    And there is exactly 1 diagnostic

  Scenario: A same-node call to a sibling callable is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { helper() }
        helper(): void { }
      }
      """
    Then there are no diagnostics

  Scenario: Self-recursion resolves
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        loopy(): void { loopy() }
      }
      """
    Then there are no diagnostics

  Scenario: The removed `self.` qualifier is diagnosed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { helper() }
        helper(): void { self.helper() }
      }
      """
    Then the diagnostics include "`self.` is removed; call `Name(args)` directly (ADR-041)"
