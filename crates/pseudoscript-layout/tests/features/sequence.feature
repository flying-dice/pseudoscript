Feature: sequence layout

  The sequence engine positions any structural diagram so that labels fit their
  lanes, activations span each participant's involvement, and combined fragments
  enclose their sections split by dividers.

  Scenario: a simple call and return
    Given the sequence diagram:
      """
      {
        "participants": [
          {"id": "A", "label": "Client", "kind": "person"},
          {"id": "B", "label": "Api", "kind": "container"}
        ],
        "items": [
          {"type": "message", "from": "A", "to": "B", "kind": "call", "label": "SignIn", "detail": "(): Result<Session, string>"},
          {"type": "message", "from": "B", "to": "A", "kind": "return", "label": "Ok", "detail": "Session"}
        ]
      }
      """
    When it is laid out
    Then no participant cards overlap
    And every message label fits its lane
    And the activation for "B" covers its messages

  Scenario: a long label still fits its lane
    Given the sequence diagram:
      """
      {
        "participants": [
          {"id": "A", "label": "A", "kind": "container"},
          {"id": "B", "label": "B", "kind": "component"}
        ],
        "items": [
          {"type": "message", "from": "A", "to": "B", "kind": "call", "label": "AuthenticateAndIssueSessionToken", "detail": "(credentials: Credentials): Result<Session, AuthError>"}
        ]
      }
      """
    When it is laid out
    Then no participant cards overlap
    And every message label fits its lane

  Scenario: an alt splits into two compartments
    Given the sequence diagram:
      """
      {
        "participants": [
          {"id": "A", "label": "Client", "kind": "person"},
          {"id": "B", "label": "Api", "kind": "container"}
        ],
        "items": [
          {"type": "message", "from": "A", "to": "B", "kind": "call", "label": "SignIn", "detail": ""},
          {"type": "fragment", "kind": "alt", "sections": [
            {"guard": "checked.isOk", "body": [
              {"type": "message", "from": "B", "to": "A", "kind": "return", "label": "Ok", "detail": "Session"}
            ]},
            {"guard": "", "body": [
              {"type": "message", "from": "B", "to": "A", "kind": "return", "label": "Err", "detail": "string"}
            ]}
          ]}
        ]
      }
      """
    When it is laid out
    Then every fragment box has ordered interior dividers
    And fragment 0 splits "Ok" above "Err"

  Scenario: a self-message label fits within the canvas
    Given the sequence diagram:
      """
      {
        "participants": [
          {"id": "A", "label": "Service", "kind": "component"}
        ],
        "items": [
          {"type": "message", "from": "A", "to": "A", "kind": "self", "label": "validate", "detail": ""}
        ]
      }
      """
    When it is laid out
    Then every message label fits its lane

  Scenario: a nested loop inside an alt stays enclosed
    Given the sequence diagram:
      """
      {
        "participants": [
          {"id": "A", "label": "A", "kind": "container"},
          {"id": "B", "label": "B", "kind": "component"}
        ],
        "items": [
          {"type": "message", "from": "A", "to": "B", "kind": "call", "label": "Run", "detail": ""},
          {"type": "fragment", "kind": "alt", "sections": [
            {"guard": "ok", "body": [
              {"type": "fragment", "kind": "loop", "sections": [
                {"guard": "each item", "body": [
                  {"type": "message", "from": "A", "to": "B", "kind": "call", "label": "Step", "detail": ""}
                ]}
              ]}
            ]}
          ]}
        ]
      }
      """
    When it is laid out
    Then no participant cards overlap
    And every fragment box has ordered interior dividers
