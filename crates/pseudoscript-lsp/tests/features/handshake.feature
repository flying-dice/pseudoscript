Feature: LSP initialize handshake over stdio

  Scenario: the server advertises its capabilities on initialize
    When I drive an in-process initialize handshake
    Then the initialize response advertises "documentFormattingProvider"
    And the initialize response advertises "pseudoscript-lsp"
    And the initialize response advertises "hoverProvider"
    And the initialize response advertises "semanticTokensProvider"
    And the initialize response advertises "completionProvider"
    And the initialize response advertises "documentSymbolProvider"
    And the initialize response advertises "workspaceSymbolProvider"
    And the initialize response advertises "referencesProvider"
    And the initialize response advertises "documentHighlightProvider"
    And the initialize response advertises "foldingRangeProvider"
    And the initialize response advertises "renameProvider"
    And the initialize response advertises "inlayHintProvider"
