Feature: Call arity matches the callable's parameters (LANG.md §5.1)

  A call to a resolvable same-module callable — a bare same-node call or a node — must
  pass as many arguments as the callable declares. Cross-module callees are not
  visible here and are not checked.

  Scenario: A self-call with too many arguments is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { add(1, 2, 3) }
        add(a: number, b: number): number { return a }
      }
      """
    Then the diagnostics include "callable `add` expects 2 argument(s), got 3"
    And there is exactly 1 diagnostic

  Scenario: A node call with the wrong arity is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { Store.put(1) }
      }
      public container Store for S {
        put(): void { }
      }
      """
    Then the diagnostics include "callable `put` expects 0 argument(s), got 1"
    And there is exactly 1 diagnostic

  Scenario: A call with the right arity is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): number { return add(1, 2) }
        add(a: number, b: number): number { return a }
      }
      """
    Then there are no diagnostics
