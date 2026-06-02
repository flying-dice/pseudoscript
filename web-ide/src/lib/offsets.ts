// The compiler reports spans as UTF-8 byte offsets; CodeMirror counts UTF-16
// code units. They coincide for ASCII but diverge on any multi-byte character,
// which would misplace squiggles. `byteToChar` maps a byte offset to the
// code-unit offset for a given source string.

/**
 * Converts a UTF-8 byte offset into `source` to a UTF-16 code-unit offset.
 * Clamps to the document length.
 */
export function byteToChar(source: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  for (let i = 0; i < source.length; i++) {
    if (bytes >= byteOffset) return i;
    const code = source.codePointAt(i) ?? 0;
    bytes += utf8Len(code);
    // Surrogate pair: codePointAt consumed two code units.
    if (code > 0xffff) i++;
  }
  return source.length;
}

/**
 * Converts a UTF-16 code-unit offset into `source` to a UTF-8 byte offset — the
 * inverse of {@link byteToChar}, for handing an editor position to the compiler.
 * Clamps to the document's byte length.
 */
export function charToByte(source: string, charOffset: number): number {
  if (charOffset <= 0) return 0;
  let bytes = 0;
  for (let i = 0; i < source.length && i < charOffset; i++) {
    const code = source.codePointAt(i) ?? 0;
    bytes += utf8Len(code);
    if (code > 0xffff) i++; // surrogate pair: two code units, one code point
  }
  return bytes;
}

/** UTF-8 byte length of a single Unicode code point. */
function utf8Len(code: number): number {
  if (code <= 0x7f) return 1;
  if (code <= 0x7ff) return 2;
  if (code <= 0xffff) return 3;
  return 4;
}
