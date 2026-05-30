Feature: LSP hover and go-to-definition

  Scenario: hover on a declared system reports its kind and FQN
    Given the inline source:
      """
      //! demo

      /// The core platform.
      public system Banking;
      """
    Then hovering the first "Banking" mentions "system"

  Scenario: hover surfaces the doc summary
    Given the inline source:
      """
      //! demo

      /// The core platform.
      public system Banking;
      """
    Then hovering the first "Banking" mentions "The core platform."

  Scenario: go-to-definition resolves a later reference to its declaration
    Given the inline source:
      """
      //! demo

      public system Banking;

      public container Mainframe for Banking;
      """
    Then go-to-definition on the second "Banking" resolves
