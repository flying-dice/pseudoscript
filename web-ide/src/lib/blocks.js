// Brace-matched `{ … }` block extents for a PseudoScript source, used by the
// editor for folding and the navigation flash. A char-level scan that skips
// string literals and `//`/`///`/`//!` line comments, so braces inside them
// don't open or close a block. PseudoScript has no block comments and no
// multi-line strings, which keeps this exact.

/**
 * The matched brace pairs in `text`, innermost-last (a pair is pushed when its
 * `}` is seen). Each entry is `{ open, close }` — the code-unit offsets of the
 * `{` and its matching `}`.
 * @param {string} text
 * @returns {{ open: number, close: number }[]}
 */
export function blockRanges(text) {
  const ranges = [];
  const stack = [];
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inString) {
      if (c === "\\") i++; // skip the escaped char
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') inString = true;
    else if (c === "/" && text[i + 1] === "/") {
      const nl = text.indexOf("\n", i);
      if (nl === -1) break;
      i = nl; // resume at the newline
    } else if (c === "{") stack.push(i);
    else if (c === "}") {
      const open = stack.pop();
      if (open !== undefined) ranges.push({ open, close: i });
    }
  }
  return ranges;
}
