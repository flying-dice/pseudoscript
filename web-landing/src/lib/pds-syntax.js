/* PseudoScript .pds tokenizer — ported from the PseudoScript Web IDE
   (pseudoscript-language.js). Returns an array of { c: className, s: text }
   spans so the live-typing animation can emit syntax-coloured characters. */

const KEYWORDS = new Set(['system', 'container', 'component', 'person', 'data', 'feature', 'for', 'public', 'return', 'if', 'else', 'while', 'alias', 'from', 'void']);
const STEP = new Set(['given', 'when', 'then', 'and', 'but']);
const ATOMS = new Set(['Result', 'Option', 'Ok', 'Err', 'Some', 'None', 'true', 'false']);
const PRIMS = new Set(['number', 'string', 'bool']);

export function pdsTokenize(line) {
  const out = [];
  let i = 0;
  const push = (c, s) => out.push({ c, s });
  while (i < line.length) {
    const rest = line.slice(i);
    let m;
    if ((m = rest.match(/^\s+/))) { push('ws', m[0]); i += m[0].length; continue; }
    if (rest.startsWith('//!') || rest.startsWith('///')) { push('doc', rest); break; }
    if (rest.startsWith('//')) { push('cmt', rest); break; }
    if ((m = rest.match(/^"(?:[^"\\]|\\.)*"?/))) { push('str', m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^#\[[^\]]*\]/))) { push('macro', m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^#[A-Za-z0-9_-]+/))) { push('tag', m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^[0-9]+(?:\.[0-9]+)?/))) { push('num', m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^[A-Za-z_][A-Za-z0-9_]*/))) {
      const w = m[0];
      const prev = line[i - 1];
      const immediate = line[i + w.length];
      let cls;
      if (prev === '.') cls = 'mem';
      else if (KEYWORDS.has(w)) cls = 'kw';
      else if (STEP.has(w)) cls = 'step';
      else if (ATOMS.has(w)) cls = 'atom';
      else if (PRIMS.has(w)) cls = 'prim';
      else if (immediate === '(') cls = 'mem';
      else if (/^[A-Z]/.test(w)) cls = 'ty';
      else cls = 'var';
      push(cls, w); i += w.length; continue;
    }
    push('punct', line[i]); i += 1;
  }
  return out;
}

// Build syntax-highlighted HTML for a whole multi-line source string.
// Each line is a display:block .ln containing a trailing "\n" so empty
// lines keep their height (matches the IDE editor). Lines join with "".
export function pdsHighlight(src) {
  return src.split('\n').map(function (ln) {
    const inner = pdsTokenize(ln).map(function (tk) {
      const esc = tk.s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
      return tk.c === 'ws' ? esc : '<span class="' + tk.c + '">' + esc + '</span>';
    }).join('');
    return '<span class="ln">' + inner + '\n</span>';
  }).join('');
}
