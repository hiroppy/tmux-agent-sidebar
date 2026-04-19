#!/usr/bin/env node
// Produce the website's og:image (1200×630) from the raw hero capture.
//
// Usage:  node scripts/hero-compose.mjs <hero.png> <og-image.png>
//
// v2 keeps the hero as-captured (no blur / vignette / gradient — the
// real tmux window is the shot). This script therefore only covers
// the og:image crop. hero.png itself is used as-is by the splash.

import sharp from "sharp";

const [, , inPath, ogPath] = process.argv;
if (!inPath || !ogPath) {
  console.error("usage: hero-compose.mjs <hero.png> <og-image.png>");
  process.exit(2);
}

await sharp(inPath)
  .resize(1200, 630, { fit: "cover", position: "left" })
  .toFile(ogPath);

console.log(`hero-compose: wrote ${ogPath}`);
