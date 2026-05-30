Feature: pds lsp

  Scenario: the server starts and answers an initialize handshake over stdio
    When I run pds lsp and send an initialize handshake
    Then the lsp response advertises "documentFormattingProvider"
    And the lsp response advertises "pseudoscript-lsp"
