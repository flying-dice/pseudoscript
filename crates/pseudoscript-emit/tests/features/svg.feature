Feature: SVG rendering smoke

  render_svg produces valid, self-contained SVG for each view kind: a context
  C4 diagram, a container C4 diagram, and a sequence diagram. The C4 views draw
  modern cards: each node carries a bold title and an UPPERCASE kind eyebrow.
  The assertion is well-formedness plus the presence of titles and eyebrows, not
  pixels.

  Scenario: context view renders cards with titles and kind eyebrows
    Given the model:
      """
      //! shop
      public person Customer;
      public system Shop;
      public system Payments;
      """
    When I project the "context" view
    Then the rendered SVG is well-formed
    And the rendered SVG is identical across two renders
    And the rendered SVG contains "Customer"
    And the rendered SVG contains "Shop"
    And the rendered SVG contains "Payments"
    And the rendered SVG contains "PERSON"
    And the rendered SVG contains "SYSTEM"

  Scenario: container view renders the boundary and its container cards
    Given the model:
      """
      //! shop
      public system Shop;
      public container Web for Shop;
      public container Orders for Shop;
      """
    When I project the "container" view of "shop::Shop"
    Then the rendered SVG is well-formed
    And the rendered SVG is identical across two renders
    And the rendered SVG contains "Web"
    And the rendered SVG contains "Orders"
    And the rendered SVG contains "Shop"
    And the rendered SVG contains "CONTAINER"

  Scenario: sequence view renders a titled trace with lifelines and messages
    Given the model:
      """
      //! shop
      public data Order { id: number }
      public data Rejected { reason: string }
      public system Shop;
      public container Inventory for Shop;
      public container Orders for Shop {
        #[manual]
        public Place(order: Order): Result<Order, Rejected> {
          r: Result<Order, Rejected> = Inventory.reserve(order)
          return Ok(order)
        }
      }
      """
    When I project the "sequence" view of "shop::Orders::Place"
    Then the rendered SVG is well-formed
    And the rendered SVG contains "Place"
    And the rendered SVG contains "Inventory"
    And the rendered SVG contains "reserve"
