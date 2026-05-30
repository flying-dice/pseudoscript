Feature: `self.` calls resolve to a same-node callable (LANG.md §5.1, ADR-004)

  `self` is the enclosing node; `self.Name(args)` invokes one of its callables.
  A `self.` call naming no callable of the enclosing node MUST be rejected.

  Scenario: A self-call to an undeclared callable is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { self.missing() }
      }
      """
    Then the diagnostics include "`self.missing` does not name a callable of `C`"
    And there is exactly 1 diagnostic

  Scenario: A self-call to a same-node callable is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { self.helper() }
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
        loopy(): void { self.loopy() }
      }
      """
    Then there are no diagnostics
