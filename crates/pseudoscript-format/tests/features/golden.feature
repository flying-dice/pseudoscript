Feature: Golden canonical outputs

  Messy inputs reformat to one exact canonical form, covering records, unions,
  stacked macros, docs with tags, nested bodies, black-box vs disclosed forms,
  control flow, from-composition, and postfix chains.

  Scenario: a record collapses to inline canonical form
    Given the source "public data BankingInfo{id:number,balance:number}"
    When I format it
    Then the result equals "public data BankingInfo { id: number, balance: number }\n"

  Scenario: a union, one variant per line
    Given the source "public data View =\n| Context\n| Container{system:string}"
    When I format it
    Then the result equals "public data View =\n  | Context\n  | Container { system: string }\n"

  Scenario: stacked macros and a tagged doc on a callable
    Given the source "public container C for B {\n/// Run it.\n/// #headline\n#[manual]\n#[http(\"POST /a\")]\npublic run(req:Request):Result<Diagram,E>;\n}"
    When I format it
    Then the result equals "public container C for B {\n  /// Run it.\n  /// #headline\n  #[manual]\n  #[http(\"POST /a\")]\n  public run(req: Request): Result<Diagram, E>;\n}\n"

  Scenario: a black-box component callable keeps its semicolon
    Given the source "public component Repo for S { fetch(id:number):Result<Info,E>; }"
    When I format it
    Then the result equals "public component Repo for S {\n  fetch(id: number): Result<Info, E>;\n}\n"

  Scenario: nested body, control flow, from-composition, and postfix chain
    Given the source "public container M for B {\nrun(s:Source):Result<Ast,E>{\ntree=Ast from {s.text,s.path}\nx=string from Repo.fetch(id).value.owner\nfor(item in xs){self.handle(item)}\nreturn Ok(tree)\n}\n}"
    When I format it
    Then the result equals "public container M for B {\n  run(s: Source): Result<Ast, E> {\n    tree = Ast from { s.text, s.path }\n    x = string from Repo.fetch(id).value.owner\n    for (item in xs) {\n      self.handle(item)\n    }\n    return Ok(tree)\n  }\n}\n"

  Scenario: a feature reformats to one step per line
    Given the source "feature  Open   for  Bank{given \"a\"\nwhen \"b\"\nthen \"c\"}"
    When I format it
    Then the result equals "feature Open for Bank {\n  given \"a\"\n  when \"b\"\n  then \"c\"\n}\n"

  Scenario: an empty disclosed record renders with a space
    Given the source "public data Empty { }"
    When I format it
    Then the result equals "public data Empty { }\n"

  Scenario: an empty disclosed node renders with a space
    Given the source "public system Z {}"
    When I format it
    Then the result equals "public system Z { }\n"

  Scenario: a doc block with extended description and a tag
    Given the source "/// Summary.\n///\n/// More.\n/// #tag\npublic system S;"
    When I format it
    Then the result equals "/// Summary.\n///\n/// More.\n/// #tag\npublic system S;\n"
