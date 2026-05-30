Feature: References in a body resolve (LANG.md §7, §8)

  A bare single-segment name used in a callable body must resolve to a
  parameter, a binding, a `for` binding, a node, or an alias. An unresolved
  reference is rejected. (Multi-segment `::` paths are left to cross-module
  resolution.)

  Scenario: An undefined reference is rejected
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(): Item { return ghost }
      }
      """
    Then the diagnostics include "unresolved reference `ghost`"
    And there is exactly 1 diagnostic

  Scenario: Parameters and bindings resolve
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(x: Item): Item {
          y = x
          return y
        }
      }
      """
    Then there are no diagnostics

  Scenario: A node name resolves
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { Store.ping() }
      }
      public container Store for S {
        ping(): void { }
      }
      """
    Then there are no diagnostics

  Scenario: A fieldless union variant is referenced by name
    Given the model file:
      """
      //! example
      public data Fault = | Timeout | Offline
      public system S;
      public container C for S {
        run(): Result<number, Fault> { return Err(Offline) }
      }
      """
    Then there are no diagnostics
