Feature: Cross-module visibility resolution (LANG.md §8.2, ADR-010)

  A reference from module A to a node in module B resolves only if that node is
  public; a cross-module reference to a private node, or a dangling cross-module
  reference, is a diagnostic. Same-module references are unaffected.

  Scenario: A public node resolves across modules
    Given the workspace modules:
      | fqn          | source                                                              |
      | banking::core | //! banking::core\npublic system Banking;                          |
      | platforms     | //! platforms\npublic container Edge for banking::core::Banking;   |
    When I check the workspace
    Then the workspace has no errors

  Scenario: A private cross-module parent is rejected
    Given the workspace modules:
      | fqn          | source                                                            |
      | banking::core | //! banking::core\nsystem Banking;                               |
      | platforms     | //! platforms\npublic container Edge for banking::core::Banking; |
    When I check the workspace
    Then the workspace diagnostics include "parent `banking::core::Banking` is private to its module"

  Scenario: A dangling cross-module reference is rejected
    Given the workspace modules:
      | fqn          | source                                                            |
      | banking::core | //! banking::core\npublic system Banking;                        |
      | platforms     | //! platforms\npublic container Edge for banking::core::Missing; |
    When I check the workspace
    Then the workspace diagnostics include "dangling cross-module reference `banking::core::Missing`: target does not resolve"

  Scenario: A private same-module reference is fine
    Given the workspace modules:
      | fqn   | source                                                          |
      | shop  | //! shop\nsystem Store;\npublic container Web for shop::Store;  |
    When I check the workspace
    Then the workspace has no errors

  Scenario: A public cross-module call target resolves
    Given the workspace modules:
      | fqn   | source                                                                                                                |
      | a     | //! a\npublic system Sys;\npublic container Box for a::Sys;                                                            |
      | b     | //! b\npublic system Other;\npublic container Caller for b::Other {\n  go(): void {\n    a::Box.run()\n  }\n}          |
    When I check the workspace
    Then the workspace has no errors

  Scenario: A private cross-module call target is rejected
    Given the workspace modules:
      | fqn   | source                                                                                                         |
      | a     | //! a\npublic system Sys;\ncontainer Box for a::Sys;                                                            |
      | b     | //! b\npublic system Other;\npublic container Caller for b::Other {\n  go(): void {\n    a::Box.run()\n  }\n}   |
    When I check the workspace
    Then the workspace diagnostics include "call target `a::Box` is private to its module"

  Scenario: A public cross-module return type resolves
    Given the workspace modules:
      | fqn   | source                                                                                                     |
      | a     | //! a\npublic data Money { amount: number }                                                                 |
      | b     | //! b\npublic system Sys;\npublic container Box for b::Sys {\n  total(): a::Money { return self.total() }\n} |
    When I check the workspace
    Then the workspace has no errors

  Scenario: A private cross-module return type is rejected
    Given the workspace modules:
      | fqn   | source                                                                                          |
      | a     | //! a\ndata Money { amount: number }                                                                        |
      | b     | //! b\npublic system Sys;\npublic container Box for b::Sys {\n  total(): a::Money { return self.total() }\n} |
    When I check the workspace
    Then the workspace diagnostics include "type `a::Money` is private to its module"

  Scenario: A dangling cross-module field type is rejected
    Given the workspace modules:
      | fqn   | source                                                            |
      | a     | //! a\npublic data Wallet;                                        |
      | b     | //! b\npublic data Account { balance: a::Missing }               |
    When I check the workspace
    Then the workspace diagnostics include "dangling cross-module reference `a::Missing`: target does not resolve"

  Scenario: A cross-module generic argument is rejected
    Given the workspace modules:
      | fqn   | source                                                                                                       |
      | a     | //! a\npublic system Sys;                                                                                     |
      | b     | //! b\npublic system Other;\npublic container Box for b::Other {\n  find(): Option<a::Missing> { return self.find() }\n} |
    When I check the workspace
    Then the workspace diagnostics include "dangling cross-module reference `a::Missing`: target does not resolve"
