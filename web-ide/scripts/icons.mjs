// Generates the PWA / app icons from the brand mark (the nested-C4 logo in
// accent on the app's dark surface) into static/icons. Run `npm run icons` after
// changing the mark; the PNGs are committed so the build and CI need no image
// toolchain. Requires `sharp` (a devDependency).

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "../static/icons");
mkdirSync(outDir, { recursive: true });

const BG = "#0a0b0e"; // --bg (dark surface)
const ACCENT = "#ff5a36"; // --accent

// The brand mark on a full-bleed background. `scale` is the mark's fraction of
// the 512 canvas — smaller for the maskable variant so it survives a circular
// mask's safe zone. The mark is authored in a 24-unit box (matching logo.svg).
function iconSvg(scale) {
  const size = 512;
  const box = size * scale;
  const offset = (size - box) / 2;
  const k = box / 24;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <rect width="${size}" height="${size}" fill="${BG}"/>
  <g transform="translate(${offset} ${offset}) scale(${k})">
    <rect x="2.5" y="2.5" width="19" height="19" rx="4" fill="none" stroke="${ACCENT}" stroke-width="1.4" opacity="0.5"/>
    <rect x="6.5" y="6.5" width="11" height="11" rx="2.6" fill="none" stroke="${ACCENT}" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="2.3" fill="${ACCENT}"/>
  </g>
</svg>`;
}

async function emit(svg, size, name) {
  const png = await sharp(Buffer.from(svg)).resize(size, size).png().toBuffer();
  writeFileSync(join(outDir, name), png);
  console.log(`  ${name} (${size}×${size}, ${png.length} bytes)`);
}

// "any" icons fill the tile; the maskable leaves a safe margin for OS masking.
await emit(iconSvg(0.72), 192, "icon-192.png");
await emit(iconSvg(0.72), 512, "icon-512.png");
await emit(iconSvg(0.56), 512, "icon-maskable-512.png");
await emit(iconSvg(0.72), 180, "apple-touch-icon.png");
console.log("icons → static/icons");
