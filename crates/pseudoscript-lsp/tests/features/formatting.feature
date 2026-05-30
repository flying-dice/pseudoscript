Feature: LSP formatting edit

  Scenario: valid input yields a whole-document edit
    Given the inline source:
      """
      public   system   Banking ;
      """
    When I compute a formatting edit
    Then there is a formatting edit
    And the edit text is "public system Banking;\n"

  Scenario: a parse error yields no edit, leaving the buffer intact
    Given the inline source:
      """
      public system ;
      """
    When I compute a formatting edit
    Then there is no formatting edit
