// FIM context assembly (model: ide::GhostText.assemble): the windowed buffer
// around the caret plus the grammar primer that steers a general model to emit
// valid PseudoScript. Pure string work — no editor or network types.

/** The assembled fill-in-the-middle request (model: ide::FimPrompt). */
export interface FimPrompt {
  prefix: string;
  suffix: string;
  primer: string;
}

// Window caps, in code units. Generous enough for a whole IDE-sized module yet
// bounded so a pathological buffer can't blow the request.
const PREFIX_CAP = 4000;
const SUFFIX_CAP = 1500;
// In-scope symbols are a hint, not a dump — cap the list so a large workspace
// doesn't crowd out the grammar.
const SYMBOL_CAP = 40;

// A compact LANG.md §10 digest plus a worked micro-example, phrased as `.pds`
// comments so it reads as code context to a FIM model and as documentation to a
// chat model. Static — computed once at module load.
const GRAMMAR_PRIMER = `// PseudoScript (.pds) — a C4 architecture-modeling language. Grammar digest:
//   module header   //! name — summary          doc lines   /// text
//   nodes           [public] person|system Name { … } | ;            (';' = black box)
//                   [public] container|component Name for module::Parent { … } | ;
//   callables       [public] name(arg: Type, …): Type { … } | ;      (inside node bodies)
//   data            [public] data Name { field: Type, … }  |  = VariantA | VariantB { f: T }  |  ;
//   types           number string bool datetime uuid void   T[]   Result<T, E>   Option<T>
//   statements      x = Type from expr      x = Type from { partA, partB }
//                   if (expr) { … } else { … }      for (x in xs) { … }      while (expr) { … }
//                   return expr      Ok(x)  Err(e)  Some(x)  None
//   calls           module::Node.method(args)   self.method(args)   — references are flat FQNs
//   triggers        #[http("POST /path")]  #[onevent(Event)]  #[schedule = "cron"]  #[manual]
//   behaviour       feature Name for module::Node { given "…" when "…" then "…" and "…" but "…" }
// Example:
//   public data Info { id: number }
//   public data Missing { id: number }
//   public system Banking {
//     public GetInfo(id: number): Result<bank::Info, bank::Missing> {
//       r = Result<bank::Info, bank::Missing> from bank::Mainframe.fetch(id)
//       if (r.isErr) {
//         return Err(r.error)
//       }
//       return Ok(r.value)
//     }
//   }
//   container Mainframe for bank::Banking {
//     fetch(id: number): Result<bank::Info, bank::Missing>;
//   }`;

/**
 * Assemble the provider context: the capped prefix/suffix slice of `doc` around
 * the caret at `pos` (a code-unit offset), and the grammar primer with the
 * workspace's in-scope symbol FQNs appended.
 */
export function assembleContext(doc: string, pos: number, symbols: readonly string[] = []): FimPrompt {
  const at = Math.max(0, Math.min(pos, doc.length));
  const scoped = symbols.slice(0, SYMBOL_CAP);
  const symbolLine = scoped.length ? `\n// In-scope symbols: ${scoped.join(", ")}` : "";
  return {
    prefix: doc.slice(Math.max(0, at - PREFIX_CAP), at),
    suffix: doc.slice(at, at + SUFFIX_CAP),
    primer: GRAMMAR_PRIMER + symbolLine,
  };
}
