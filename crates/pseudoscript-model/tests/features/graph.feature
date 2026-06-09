Feature: Resolved relationship graph (LANG.md §9)

  The graph is a pure projection of the resolved workspace: every structural
  node and callable, the typed edges between them (for-parent, call, trigger,
  provenance), and an ordered sequence trace per disclosed callable.

  Scenario: The worked-example model yields the expected node kinds and wiring
    Given the graph of the top-level model
    Then node "pseudoscript::Developer" has kind "person"
    And node "pseudoscript::Pseudoscript" has kind "system"
    And node "pseudoscript::Cli" has kind "container"
    And node "pseudoscript::Generate" has kind "component"
    And node "pseudoscript::Generate::run" has kind "callable"
    And node "pseudoscript::Cli" has parent "pseudoscript::Pseudoscript"
    And node "pseudoscript::Generate" has parent "pseudoscript::Cli"
    And node "pseudoscript::Generate::run" has parent "pseudoscript::Generate"
    And node "pseudoscript::Generate::run" is public
    And node "pseudoscript::Args" is private

  Scenario: for-parent edges wire containers to their system
    Given the graph of the top-level model
    Then there is a "forparent" edge from "pseudoscript::Cli" to "pseudoscript::Pseudoscript"

  Scenario: a trigger macro synthesises an initiator edge
    Given the graph of the top-level model
    Then node "pseudoscript::Generate::run" has trigger initiator "caller"
    And there is a "trigger" edge from "caller" to "pseudoscript::Generate::run"

  Scenario: a cross-boundary body call is a call edge
    Given the graph of the top-level model
    Then there is a "call" edge from "pseudoscript::Generate" to "pseudoscript::Parser" labelled "parse"

  Scenario: the Generate.run sequence trace is in evaluation order with an alt
    Given the graph of the top-level model
    Then the trace of "pseudoscript::Generate::run" is:
      """
      call pseudoscript::Parser.parse
      alt (ast.isErr)
        return Err
      call pseudoscript::Builder.build
      alt (graph.isErr)
        return Err
      call pseudoscript::Views.extract
      call pseudoscript::Transpiler.emit
      alt (out.isErr)
        return Err
      return Ok
      """

  Scenario: an onevent trigger names its event initiator
    Given the graph of the model:
      """
      //! shop
      public data OrderPlaced { id: string }
      public system Shop;
      public container Orders for shop::Shop {
        #[onevent(shop::OrderPlaced)]
        public onPlaced(e: shop::OrderPlaced): void { }
      }
      """
    Then node "shop::Orders::onPlaced" has trigger initiator "event:shop::OrderPlaced"
    And there is a "trigger" edge from "event:shop::OrderPlaced" to "shop::Orders::onPlaced"

  Scenario: a self-call and a provenance edge are traced
    Given the graph of the model:
      """
      //! shop
      public data Cart { total: number }
      public system Shop;
      public container Web for shop::Shop {
        public checkout(): void {
          a = number from self.price()
          cart = shop::Cart from { Web.lookup() }
        }
        price(): number { return 0 }
        lookup(): number { return 0 }
      }
      """
    Then the trace of "shop::Web::checkout" is:
      """
      self.price
      call shop::Web.lookup
      """
    And there is a "provenance" edge from "shop::Web" to "shop::Cart"

  Scenario: a for loop becomes a loop frame
    Given the graph of the model:
      """
      //! shop
      public data Item { id: string }
      public system Shop;
      public container Web for shop::Shop {
        public each(items: shop::Item[]): void {
          for (it in items) {
            self.handle()
          }
        }
        handle(): void { }
      }
      """
    Then the trace of "shop::Web::each" is:
      """
      loop (items)
        self.handle
      """

  Scenario: a binary condition renders in the alt frame label (§7.5)
    Given the graph of the model:
      """
      //! shop
      public constant LIMIT = 100
      public system Shop;
      public container Web for shop::Shop {
        public guard(x: number): void {
          if (x > shop::LIMIT && x >= 0) {
            self.handle()
          }
        }
        handle(): void { }
      }
      """
    Then the trace of "shop::Web::guard" is:
      """
      alt (x > shop::LIMIT && x >= 0)
        self.handle
      """
