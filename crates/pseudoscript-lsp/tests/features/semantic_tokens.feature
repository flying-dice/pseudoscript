Feature: LSP semantic tokens
  The server colours identifiers by their declared role (AST-aware) and the
  remaining leaves — keywords, doc comments, literals — from the token stream.
  Tokens are matched by the first occurrence of their source text.

  Scenario: a system name is a declared namespace
    Given the inline source:
      """
      public system Banking;
      """
    When I compute semantic tokens
    Then the "Banking" token has type "namespace"
    And the "Banking" token is declared

  Scenario: a data name is a declared class, with fields and types
    Given the inline source:
      """
      data Account { id: uuid }
      """
    When I compute semantic tokens
    Then the "Account" token has type "class"
    And the "Account" token is declared
    And the "id" token has type "property"
    And the "uuid" token has type "type"

  Scenario: callables, parameters, types, self, and calls
    Given the inline source:
      """
      system S {
        run(name: string): uuid {
          return self.alloc(name)
        }
      }
      """
    When I compute semantic tokens
    Then the "run" token has type "method"
    And the "run" token is declared
    And the "name" token has type "parameter"
    And the "string" token has type "type"
    And the "self" token has type "keyword"
    And the "alloc" token has type "method"

  Scenario: a macro invocation is one decorator span
    Given the inline source:
      """
      #[diagram]
      system S;
      """
    When I compute semantic tokens
    Then the "#[diagram]" token has type "decorator"

  Scenario: keywords and string literals
    Given the inline source:
      """
      system S {
        f(): void {
          return Err("boom")
        }
      }
      """
    When I compute semantic tokens
    Then the "Err" token has type "keyword"
    And some token has type "string"

  Scenario: union variants are enum members
    Given the inline source:
      """
      data Shape = | Circle | Square
      """
    When I compute semantic tokens
    Then the "Circle" token has type "enumMember"
    And the "Square" token has type "enumMember"
