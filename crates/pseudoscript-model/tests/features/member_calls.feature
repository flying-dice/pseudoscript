Feature: Method calls resolve to a declared callable (LANG.md §6)

  A `.name(args)` call's member MUST exist on the receiver's type where the
  receiver resolves to a same-module node or `data` record. A node receiver must
  name one of its callables; a `data` record has only fields, so any call on a
  record value is rejected. Receivers whose type is not inferred (call results,
  `::` paths) are not checked (ADR-022); `self.` calls are checked separately.

  Scenario: Calling an undeclared method on a node receiver is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container Db for S;
      public component Store for Db {
        get(): void;
      }
      public container C for S {
        run(): void { Store.nope() }
      }
      """
    Then the diagnostics include "no method `nope` on `Store`"
    And there is exactly 1 diagnostic

  Scenario: Calling a declared method on a node receiver is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container Db for S;
      public component Store for Db {
        get(): void;
      }
      public container C for S {
        run(): void { Store.get() }
      }
      """
    Then there are no diagnostics

  Scenario: Calling a method on a data value is rejected
    Given the model file:
      """
      //! example
      public data Conv { id: uuid }
      public system S;
      public container C for S {
        run(c: Conv): void { c.doThing() }
      }
      """
    Then the diagnostics include "no method `doThing` on `Conv`"
    And there is exactly 1 diagnostic

  Scenario: A method call through a field chain is checked against the field's type
    Given the model file:
      """
      //! example
      public data Conv { id: uuid }
      public data Deps { conversation: Conv }
      public system S;
      public container C for S {
        run(d: Deps): void { d.conversation.participantHasWriteAccess(d) }
      }
      """
    Then the diagnostics include "no method `participantHasWriteAccess` on `Conv`"
    And there is exactly 1 diagnostic

  Scenario: A method call on a call result is not checked
    Given the model file:
      """
      //! example
      public data Conv { id: uuid }
      public system S;
      public container C for S {
        fetch(): Conv;
        run(): void { self.fetch().anything() }
      }
      """
    Then there are no diagnostics
