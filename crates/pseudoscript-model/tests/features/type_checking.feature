Feature: Return-type and `from` type checking (LANG.md §5.1, §6, §7.2)

  A `return` expression whose type is statically determinable — a literal, an
  `Ok`/`Err`/`Some`/`None` marker, or a `Type from { .. }` composition — must
  match the callable's declared return type. A `from` target must be a `data`
  record or union variant. Each scenario exercises one rule with a minimal
  inline model.

  Scenario: A literal return must match the declared return type
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        one(): number { return "" }
      }
      """
    Then the diagnostics include "return type `string` does not match declared `number`"
    And there is exactly 1 diagnostic

  Scenario: A literal return matching the declared type is well-formed
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        one(): number { return 1 }
      }
      """
    Then there are no diagnostics

  Scenario: An Ok marker returned from a non-Result callable is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        one(): number { return Ok(1) }
      }
      """
    Then the diagnostics include "return type `Result` does not match declared `number`"
    And there is exactly 1 diagnostic

  Scenario: A None marker returned from a non-Option callable is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        one(): number { return None }
      }
      """
    Then the diagnostics include "return type `Option` does not match declared `number`"
    And there is exactly 1 diagnostic

  Scenario: Ok returned from a Result callable is well-formed
    Given the model file:
      """
      //! example
      public data E { msg: string }
      public system S;
      public container C for S {
        one(): Result<number, E> { return Ok(1) }
      }
      """
    Then there are no diagnostics

  Scenario: Some and None returned from an Option callable are well-formed
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        maybe(o: Option<Item>): Option<Item> {
          if (o.isNone) { return None }
          return Some(o.value)
        }
      }
      """
    Then there are no diagnostics

  Scenario: A `from` composition returned where an array is declared is rejected
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        all(x: number): Item[] { return Item from { x } }
      }
      """
    Then the diagnostics include "return type `Item` does not match declared `Item[]`"
    And there is exactly 1 diagnostic

  Scenario: A `from` composition matching the declared type is well-formed
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        one(x: number): Item { return Item from { x } }
      }
      """
    Then there are no diagnostics

  Scenario: A `from` target that is a primitive is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        build(x: number): string { return string from { x } }
      }
      """
    Then the diagnostics include "`from` target `string` is not a `data` record or variant"
    And there is exactly 1 diagnostic

  Scenario: An array `from` composition matches an array return (ADR-021)
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        all(x: Item, y: Item): Item[] { return Item[] from { x, y } }
      }
      """
    Then there are no diagnostics

  Scenario: A binding of the wrong type returned is rejected
    Given the model file:
      """
      //! example
      public system S;
      public container C for S {
        one(): number {
          x: string = ""
          return x
        }
      }
      """
    Then the diagnostics include "return type `string` does not match declared `number`"
    And there is exactly 1 diagnostic

  Scenario: A parameter returned at its declared type is well-formed
    Given the model file:
      """
      //! example
      public data Item { id: number }
      public system S;
      public container C for S {
        echo(it: Item): Item { return it }
      }
      """
    Then there are no diagnostics
