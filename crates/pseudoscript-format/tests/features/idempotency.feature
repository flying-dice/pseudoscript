Feature: Formatting is idempotent

  Formatting an already-formatted document changes nothing: format(format(x))
  equals format(x). This holds for canonical inputs and for deliberately messy
  ones with odd spacing, indentation, and extra blank lines.

  Scenario: a minimal black-box system
    Given the source "public   system   Banking ;"
    Then formatting is idempotent

  Scenario: messy spacing around a record
    Given the source "public data BankingInfo{id:number,balance:number}"
    Then formatting is idempotent

  Scenario: extra blank lines between declarations
    Given the source "public system A;\n\n\n\n\npublic system B;"
    Then formatting is idempotent

  Scenario: a disclosed container with control flow
    Given the source "public container M for B {\n public Get(id:number):Result<Info,NotFound>{\nr:Result<Info,NotFound>=Repo.fetch(id)\nif(r.isErr){return Err(r.error)}\nreturn Ok(r.value)\n}\n}"
    Then formatting is idempotent

  Scenario: a union with inline records
    Given the source "public data View =\n| Context\n| Container{system:string}\n| Sequence{entry:string}"
    Then formatting is idempotent

  Scenario: stacked macros and doc tags
    Given the source "public container C for B {\n/// Run it.\n/// #headline\n#[manual]\n#[http(\"POST /a\")]\npublic run(req:Request):Result<Diagram,PipelineError>;\n}"
    Then formatting is idempotent

  Scenario: a feature scenario with messy spacing
    Given the source "feature  Open   for  Bank{given \"a verified owner\"\nand \"no duplicate\"\nwhen \"opened\"\nthen \"info returned\"}"
    Then formatting is idempotent

  Scenario: a large worked model
    Given the bundled worked model
    Then formatting is idempotent

  Scenario: the static worked example
    Given the sample file CONFORMANCE/static/0-ok-worked-example.pds
    Then formatting is idempotent
