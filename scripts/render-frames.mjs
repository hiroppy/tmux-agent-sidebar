#!/usr/bin/env node
// Render every *.html in <dir> to a sibling *.png via headless Chromium,
// then composite each PNG onto a pale-blue gradient background with
// 32 CSS-px padding on all sides. The HTML itself is bare (no body
// padding / bg) so the composition step lives in one place here.
//
// Usage:  node scripts/render-frames.mjs <dir> [--scale 2]
//
// Resolves `playwright` and `sharp` relative to CWD's node_modules.
// Run from `website/` so both packages are found.

import { readdir, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "playwright";
import sharp from "sharp";

const argv = process.argv.slice(2);
const dir = resolve(argv[0] ?? ".");
const scaleIdx = argv.indexOf("--scale");
// deviceScaleFactor of 4 gives enough physical pixels that a PNG
// displayed at its natural width (features pages) still has retina
// headroom on 2x displays. Astro's Image component won't upscale
// beyond source, so the source has to carry those pixels itself.
const scale = scaleIdx >= 0 ? Number(argv[scaleIdx + 1]) : 4;
if (!Number.isFinite(scale) || scale <= 0) {
  console.error(
    `render-frames: --scale must be a positive number (got "${argv[scaleIdx + 1] ?? ""}")`,
  );
  process.exit(1);
}

const s = await stat(dir).catch(() => null);
if (!s || !s.isDirectory()) {
  console.error(`render-frames: not a directory: ${dir}`);
  process.exit(1);
}

const htmlFiles = (await readdir(dir))
  .filter((f) => f.endsWith(".html"))
  .sort();

if (htmlFiles.length === 0) {
  console.error(`render-frames: no .html files in ${dir}`);
  process.exit(1);
}

const browser = await chromium.launch();
const ctx = await browser.newContext({ deviceScaleFactor: scale });
const page = await ctx.newPage();

for (const file of htmlFiles) {
  const htmlPath = join(dir, file);
  const pngPath = htmlPath.replace(/\.html$/, ".png");
  await page.goto(pathToFileURL(htmlPath).toString(), { waitUntil: "load" });
  // Wait for webfonts so typography is stable across frames.
  await page.evaluate(() => document.fonts.ready);
  // capture/render_html wraps content in a .mac-window with chrome.
  // Screenshot that element so the chrome is baked into each frame.
  const target = (await page.$(".mac-window")) || (await page.$("pre"));
  if (!target) {
    console.error(`render-frames: no .mac-window or <pre> in ${htmlPath}`);
    process.exit(1);
  }
  const rawPng = await target.screenshot({ omitBackground: true });
  await composeFramedPng(rawPng, pngPath, scale);
}

await browser.close();
console.log(`render-frames: wrote ${htmlFiles.length} PNG(s) in ${dir}`);

// Composite the bare window screenshot onto a pale-blue gradient
// background with 32 CSS-px padding (× deviceScaleFactor to stay
// in physical pixels).
async function composeFramedPng(innerPng, outPath, scaleFactor) {
  const padding = 32 * scaleFactor;
  const { width, height } = await sharp(innerPng).metadata();
  const totalW = width + padding * 2;
  const totalH = height + padding * 2;
  // The drop-shadow lives in a dedicated SVG layer so it stays behind
  // the window. Rendering it in HTML (via box-shadow) leaked into the
  // Playwright element screenshot and showed up as grey corners after
  // the radius clip.
  const shadowBlur = 24 * scaleFactor;
  const shadowOffsetY = 10 * scaleFactor;
  const radius = 12 * scaleFactor;
  const shadow = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${totalW}" height="${totalH}">
       <defs>
         <filter id="s" x="-20%" y="-20%" width="140%" height="140%">
           <feGaussianBlur in="SourceGraphic" stdDeviation="${shadowBlur / 2}"/>
         </filter>
       </defs>
       <rect x="${padding}" y="${padding + shadowOffsetY}"
             width="${width}" height="${height}" rx="${radius}" ry="${radius}"
             fill="rgba(0,0,0,0.45)" filter="url(#s)"/>
     </svg>`,
  );
  const gradient = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${totalW}" height="${totalH}">
       <defs>
         <linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
           <stop offset="0%" stop-color="#cfe4f5"/>
           <stop offset="100%" stop-color="#b8d4ea"/>
         </linearGradient>
       </defs>
       <rect width="100%" height="100%" fill="url(#g)"/>
     </svg>`,
  );
  await sharp(gradient)
    .composite([
      { input: shadow },
      { input: innerPng, top: padding, left: padding },
    ])
    .png()
    .toFile(outPath);
}
