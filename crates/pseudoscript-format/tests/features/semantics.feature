Feature: Formatting preserves semantics

  Formatted output re-parses with zero error diagnostics and carries exactly the
  same meaningful (non-trivia) token stream as the input — only whitespace and
  comment layout change.

  Scenario: messy record re-parses with identical tokens
    Given the source "public data Info{id:number,balance:number}"
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens

  Scenario: callable with control flow
    Given the source "public container M for B {\npublic Get(id:number):Result<Info,NotFound>{\nr:Result<Info,NotFound>=Repo.fetch(id)\nif(r.isErr){return Err(r.error)}\nreturn Ok(r.value)\n}\n}"
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens

  Scenario: from-composition and postfix chain
    Given the source "public container M for B {\nrun(s:Source):Result<Ast,E>{\ntree:Ast=Ast from {s.text,s.path}\nx:string=Repo.fetch(id).value.owner\nreturn Ok(tree)\n}\n}"
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens

  Scenario: array types and aliases
    Given the source "alias R = a::b::Repo;\npublic container M for B {\nparse(argv:string[]):Result<Request,E>;\n}"
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens

  Scenario: a large worked model preserves tokens
    Given the bundled worked model
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens

  Scenario: the static worked example preserves tokens
    Given the sample file CONFORMANCE/static/0-ok-worked-example.pds
    When I format it
    Then the result re-parses without errors
    And the result preserves the meaningful tokens
