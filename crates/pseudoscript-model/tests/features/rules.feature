Feature: Static rules, one scenario each (LANG.md §2.4, §3.5, §4, §5.1, §6, §8)

  Each scenario exercises a single static rule with a minimal inline model, so
  every rule is independently documented and tested.

  Scenario: Unknown macro is rejected (closed namespace)
    Given the model file:
      """
      //! example
      public system Banking;
      public container Api for Banking {
        #[retry(3)]
        Ping() { }
      }
      """
    Then the diagnostics include "unknown macro `retry`"
    And there is exactly 1 diagnostic

  Scenario: A trigger macro cannot target a structural declaration
    Given the model file:
      """
      //! example
      public system Banking;
      #[http("GET /ping")]
      public container Api for Banking;
      """
    Then the diagnostics include "macro `http` cannot target a container"
    And there is exactly 1 diagnostic

  Scenario: An onevent handler parameter must match the event type
    Given the model file:
      """
      //! banking::core
      public data BankAccCreated { accId: string }
      public data AuditEntry { note: string }
      public system Banking;
      public container Audit for Banking {
        #[onevent(banking::core::BankAccCreated)]
        StoreCreation(e: banking::core::AuditEntry) { }
      }
      """
    Then the diagnostics include "handler parameter type `banking::core::AuditEntry` does not match triggered event `banking::core::BankAccCreated`"
    And there is exactly 1 diagnostic

  Scenario: An onevent handler whose parameter matches is well-formed
    Given the model file:
      """
      //! banking::core
      public data BankAccCreated { accId: string }
      public system Banking;
      public container Audit for Banking {
        #[onevent(banking::core::BankAccCreated)]
        StoreCreation(e: banking::core::BankAccCreated) { }
      }
      """
    Then there are no diagnostics

  Scenario: An inline union variant colliding with a data declaration is rejected
    Given the model file:
      """
      //! example
      data BankAccClosed { accId: string }
      data AccountEvent =
        | BankAccCreated { accId: string }
        | BankAccClosed  { accId: string, reason: string }
      """
    Then the diagnostics include "variant `BankAccClosed` collides with data `BankAccClosed`"
    And there is exactly 1 diagnostic

  Scenario: A container parented to a non-system is rejected
    Given the model file:
      """
      //! example
      public system Banking;
      public container Mainframe for Banking;
      public container Ledger for Mainframe;
      """
    Then the diagnostics include "container `Ledger` parent `Mainframe` is not a system"
    And there is exactly 1 diagnostic

  Scenario: A component parented to a non-container is rejected
    Given the model file:
      """
      //! example
      public system Banking;
      public component Widget for Banking;
      """
    Then the diagnostics include "component `Widget` parent `Banking` is not a container"
    And there is exactly 1 diagnostic

  Scenario: A non-void callable that falls through is rejected
    Given the model file:
      """
      //! example
      public data Info { id: number }
      public data Missing { id: number }
      public system Banking;
      public container Mainframe for Banking {
        Get(): Result<Info, Missing> {
          r = Result<Info, Missing> from self.Fetch()
          if (r.isErr) {
            return Err(r.error)
          }
        }
        Fetch(): Result<Info, Missing>;
      }
      """
    Then the diagnostics include "callable `Get` does not return on all paths"
    And there is exactly 1 diagnostic

  Scenario: A void callable needs no return
    Given the model file:
      """
      //! example
      public data Info { id: number }
      public system Banking;
      public container Mainframe for Banking {
        Run(): void {
          self.Make()
        }
        Make(): Info;
      }
      """
    Then there are no diagnostics

  Scenario: Re-binding a name is rejected (single-assignment)
    Given the model file:
      """
      //! example
      public data Info { id: number }
      public system Banking;
      public container Mainframe for Banking {
        Run(): void {
          r = Info from self.Make()
          r = Info from self.Make()
        }
        Make(): Info;
      }
      """
    Then the diagnostics include "re-binding of `r`: bindings are single-assignment"
    And there is exactly 1 diagnostic

  Scenario: Reading .value on an Err branch is rejected
    Given the model file:
      """
      //! banking::core
      public data BankingInfo { id: number }
      public data NotFound { id: number }
      public system Banking;
      public container Mainframe for Banking {
        public GetBankingInfo(id: number): Result<BankingInfo, NotFound> {
          r = Result<BankingInfo, NotFound> from AccountStore::Repository.fetch(id)
          if (r.isErr) {
            return Err(r.value)
          }
          return Ok(r.value)
        }
      }
      public container AccountStore for Banking;
      component Repository for AccountStore {
        fetch(id: number): Result<BankingInfo, NotFound>;
      }
      """
    Then the diagnostics include "`.value` read on an `Err` branch"
    And there is exactly 1 diagnostic

  Scenario: Reading .error on an Ok branch is rejected
    Given the model file:
      """
      //! example
      public data Info { id: number }
      public data Bad { id: number }
      public system Banking;
      public container Mainframe for Banking {
        Get(): Result<Info, Bad> {
          r = Result<Info, Bad> from self.Fetch()
          if (r.isOk) {
            return Ok(r.error)
          }
          return Err(r.error)
        }
        Fetch(): Result<Info, Bad>;
      }
      """
    Then the diagnostics include "`.error` read on an `Ok` branch"
    And there is exactly 1 diagnostic
