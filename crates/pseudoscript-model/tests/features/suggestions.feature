Feature: "Did you mean" suggestions (Levenshtein)

  When a reference, field, or self-call does not resolve, the diagnostic
  suggests the closest in-scope name within a small edit distance.

  Scenario: An unresolved reference suggests a near parameter
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(payload: Item): Item { return paylod }
      }
      """
    Then the diagnostics include "unresolved reference `paylod`; did you mean `payload`?"

  Scenario: An unknown field suggests a near field
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        get(it: Item): number { return it.ide }
      }
      """
    Then the diagnostics include "no field `ide` on `Item`; did you mean `id`?"

  Scenario: An unknown self-call suggests a near callable
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        run(): void { helpr() }
        helper(): void { }
      }
      """
    Then the diagnostics include "`helpr` does not name a callable of `C`; did you mean `helper`?"

  Scenario: A distant typo gets no suggestion
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
