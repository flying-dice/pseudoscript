// Packs the authoring skill folder (.claude/skills/pseudocode) into a real,
// DEFLATE-compressed `.zip` served as a static asset, so the IDE's "Download
// skill" button is a plain link — no client-side zip code. Run with Bun
// (`npm run bundle:skill`), which supplies the compression; the resulting
// static/pseudocode-skill.zip is committed and refreshed when the skill changes.

import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const skillDir = join(here, "../../.claude/skills/pseudocode");
const rootName = "pseudocode"; // the folder name under .claude/skills/
const outFile = join(here, "../static/pseudocode-skill.zip");

// IEEE CRC-32 — ZIP stores one per entry regardless of compression method.
const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const enc = new TextEncoder();
const entry = (path, bytes) => {
  const raw = new Uint8Array(bytes);
  const deflated = Bun.deflateSync(raw, { raw: true }); // Bun supplies DEFLATE
  return { name: enc.encode(path), raw, deflated, crc: crc32(raw) };
};

const entries = walk(skillDir).map((full) =>
  entry(`${rootName}/${relative(skillDir, full).split(/[\\/]/).join("/")}`, readFileSync(full)),
);
// SKILL.md defers every syntax question to `references/LANG.md`; ship the
// canonical spec (the repo root LANG.md) at that path so the skill is
// self-contained once downloaded.
const langMd = readFileSync(join(here, "../../LANG.md"));
entries.push(entry(`${rootName}/references/LANG.md`, langMd));

// The same spec also backs the IDE's Help → Language reference: vendored under
// src/lib/bundled/ so the bundle imports it (`?raw`). Committed (the root
// .gitignore reserves `reference/` for the Graphviz oracle clone), and
// refreshed by this script whenever LANG.md changes.
const bundledDir = join(here, "../src/lib/bundled");
mkdirSync(bundledDir, { recursive: true });
writeFileSync(join(bundledDir, "LANG.md"), langMd);

const locals = [];
const central = [];
let offset = 0;
for (const e of entries) {
  const local = new Uint8Array(30 + e.name.length);
  const lv = new DataView(local.buffer);
  lv.setUint32(0, 0x04034b50, true);
  lv.setUint16(4, 20, true); // version needed
  lv.setUint16(8, 8, true); // method: deflate
  lv.setUint16(12, 0x21, true); // mod date 1980-01-01
  lv.setUint32(14, e.crc, true);
  lv.setUint32(18, e.deflated.length, true); // compressed size
  lv.setUint32(22, e.raw.length, true); // uncompressed size
  lv.setUint16(26, e.name.length, true);
  local.set(e.name, 30);
  locals.push(local, e.deflated);

  const cd = new Uint8Array(46 + e.name.length);
  const cv = new DataView(cd.buffer);
  cv.setUint32(0, 0x02014b50, true);
  cv.setUint16(4, 20, true);
  cv.setUint16(6, 20, true);
  cv.setUint16(10, 8, true); // method: deflate
  cv.setUint16(14, 0x21, true);
  cv.setUint32(16, e.crc, true);
  cv.setUint32(20, e.deflated.length, true);
  cv.setUint32(24, e.raw.length, true);
  cv.setUint16(28, e.name.length, true);
  cv.setUint32(42, offset, true);
  cd.set(e.name, 46);
  central.push(cd);

  offset += locals[locals.length - 2].length + e.deflated.length;
}

const centralSize = central.reduce((n, c) => n + c.length, 0);
const end = new Uint8Array(22);
const ev = new DataView(end.buffer);
ev.setUint32(0, 0x06054b50, true);
ev.setUint16(8, entries.length, true);
ev.setUint16(10, entries.length, true);
ev.setUint32(12, centralSize, true);
ev.setUint32(16, offset, true);

const parts = [...locals, ...central, end];
const total = parts.reduce((n, p) => n + p.length, 0);
const zip = new Uint8Array(total);
let at = 0;
for (const p of parts) {
  zip.set(p, at);
  at += p.length;
}

mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, zip);
console.log(`packed ${entries.length} skill file(s) → static/pseudocode-skill.zip (${zip.length} bytes)`);
