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

  Scenario: A bare same-module `for` parent must be fully qualified (ADR-030)
    Given the workspace modules:
      | fqn   | source                                              |
      | shop  | //! shop\npublic system Store;\ncontainer Web for Store; |
    When I check the workspace
    Then the workspace diagnostics include "`Store` must be fully qualified: `shop::Store`"

  Scenario: A bare same-module feature target must be fully qualified (ADR-030)
    Given the workspace modules:
      | fqn   | source                                                       |
      | shop  | //! shop\npublic system App;\nfeature Show for App {\n  when "x"\n  then "y"\n} |
    When I check the workspace
    Then the workspace diagnostics include "`App` must be fully qualified: `shop::App`"

  Scenario: A bare same-module field type must be fully qualified (ADR-030)
    Given the workspace modules:
      | fqn   | source                                                                  |
      | shop  | //! shop\npublic data Money { amount: number }\npublic data Account { balance: Money } |
    When I check the workspace
    Then the workspace diagnostics include "`Money` must be fully qualified: `shop::Money`"

  Scenario: A bare same-module body reference must be fully qualified (ADR-030)
    Given the workspace modules:
      | fqn   | source                                                                                                              |
      | shop  | //! shop\npublic system App;\npublic container Box for shop::App {\n  run(): void;\n}\npublic container Caller for shop::App {\n  go(): void {\n    Box.run()\n  }\n} |
    When I check the workspace
    Then the workspace diagnostics include "`Box` must be fully qualified: `shop::Box`"

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

  Scenario: A nested structural call target suggests its flat FQN (ADR-030)
    Given the workspace modules:
      | fqn   | source                                                                                                                              |
      | a     | //! a\npublic system Sys;\npublic container Box for a::Sys;\npublic component Comp for a::Box {\n  run(): void;\n}                    |
      | b     | //! b\npublic system Other;\npublic container Caller for b::Other {\n  go(): void {\n    a::Sys::Box::Comp.run()\n  }\n}              |
    When I check the workspace
    Then the workspace diagnostics include "call target `a::Sys::Box::Comp` is not a fully-qualified name; use `a::Comp`"

  Scenario: A same-module structural drill suggests its flat FQN (ADR-036)
    Given the workspace modules:
      | fqn   | source                                                                                                                                                                  |
      | shop  | //! shop\npublic system App;\npublic container Box for shop::App;\npublic component Repo for shop::Box {\n  run(): void;\n}\npublic container Caller for shop::App {\n  go(): void {\n    Box::Repo.run()\n  }\n} |
    When I check the workspace
    Then the workspace diagnostics include "call target `Box::Repo` is not a fully-qualified name; use `shop::Repo`"

  Scenario: The flat FQN of the same component resolves
    Given the workspace modules:
      | fqn   | source                                                                                                                              |
      | a     | //! a\npublic system Sys;\npublic container Box for a::Sys;\npublic component Comp for a::Box {\n  run(): void;\n}                    |
      | b     | //! b\npublic system Other;\npublic container Caller for b::Other {\n  go(): void {\n    a::Comp.run()\n  }\n}                        |
    When I check the workspace
    Then the workspace has no errors

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
