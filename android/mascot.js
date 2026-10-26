#!/usr/bin/env node
/* app/src/main/res/drawable-nodpi/mascot.png: About's mascot as the
   desktop's About draws it at rest (ui/mascot.js, without motion), rendered
   by Chromium's WebGL 2 on a transparent ground, for the Android app, which
   shows it still. The picture is the scene's own box (mascot.js, draw: 70 x
   1.08 high, rxz x 1.07 either side), so that fitting it into a stage
   frames it as the desktop's camera does; 1200 px high, a phone's stage at
   3x. Dark mode changes only the shadow, which at rest is under the feet:
   the light and dark renders are the same bytes, and one picture serves both.

     NODE_PATH=/path/to/node_modules node android/mascot.js

   Needs playwright-core and its Chromium; the picture is checked in, so a
   build does not. */
"use strict";
const path = require("path");
const { chromium } = require("playwright-core");

const root = path.dirname(__dirname);
const out = path.join(__dirname, "app", "src", "main", "res", "drawable-nodpi", "mascot.png");
const TALL = 1200;

(async () => {
  // WebGL 2 without a GPU: SwiftShader, which Chromium uses only when asked
  const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
  // mascot.js draws at the stage's size times a pixel ratio of at most 2
  const page = await browser.newPage({ viewport: { width: 1600, height: 1600 }, deviceScaleFactor: 2,
    reducedMotion: "reduce" });
  page.on("pageerror", (e) => {
    console.error("mascot.js:", e.message);
    process.exit(1);
  });
  await page.setContent(`<style>html,body{margin:0;background:transparent}#stage{position:relative}
    canvas{position:absolute;inset:0;width:100%;height:100%;display:block}</style>
    <div id="stage"><canvas id="mascot"></canvas></div>`);
  await page.addScriptTag({ path: path.join(root, "ui", "mascot-model.js") });
  await page.addScriptTag({ path: path.join(root, "ui", "mascot.js") });
  const ok = await page.evaluate((tall) => {
    const halfH = 70 * 1.08,
      halfW = MASCOT.rxz * 1.07;
    const stage = document.getElementById("stage");
    stage.style.width = `${Math.round(((tall / 2) * halfW) / halfH)}px`;
    stage.style.height = `${tall / 2}px`;
    window.mascot = drawMascot(stage, document.getElementById("mascot"));
    if (!window.mascot) return false;
    window.mascot.show();
    return true;
  }, TALL);
  if (!ok) throw new Error("no WebGL 2");
  await page.waitForTimeout(1500);
  // at rest, as hiding it draws it for the next time
  await page.evaluate(() => window.mascot.hide());
  await page.waitForTimeout(300);
  await page.locator("#mascot").screenshot({ path: out, omitBackground: true });
  await browser.close();
  console.log(path.relative(root, out));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
