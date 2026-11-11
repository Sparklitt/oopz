// Makes the user guide's pictures (docs/guide/images/<lang>/<name>.png) from the UI with its invented data (the mock
// backend): `pnpm screenshots`, or `pnpm screenshots <name>…` for some. Each picture is one entry of
// screenshots.config.mjs. The same code gives the same pixels: the page's clock is fixed, animations and the caret are
// off, and the fonts come from npm (pinned in package.json) instead of the system, so a picture changes only when the
// UI does. A picture whose pixels didn't change is not written again (PNG bytes may differ, pixels don't), and
// pictures no entry makes any more are removed.
// Needs Playwright's Chromium once: `pnpm exec playwright install chromium` (on Linux CI: `--with-deps chromium`).
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { build } from "vite";
import { LOCALES } from "../src/lib/i18n/index.ts";
import { CLOCK, VIEWPORTS, shots } from "./screenshots.config.mjs";

const desktop = fileURLToPath(new URL("..", import.meta.url));
const modules = join(desktop, "node_modules");
const out = fileURLToPath(new URL("../../../docs/guide/images/", import.meta.url));
const ORIGIN = "http://oopz.guide";
/** Pictures made at once (each in its own browser context). */
const AT_ONCE = 4;
const only = process.argv.slice(2);
const unknown = only.filter((n) => !shots.some((s) => s.name === n));
if (unknown.length) throw new Error(`no such picture in screenshots.config.mjs: ${unknown.join(", ")}`);

// Fonts: the app asks for the system's; here every system gets the same ones. Emoji are Twemoji (COLR), which
// headless Chromium draws in colour; it draws Noto Color Emoji's web font blank.
const FONTS = [
  ["noto-sans", ["400", "500", "600", "700"]],
  ["noto-sans-arabic", ["400", "600"]],
  ["noto-sans-mono", ["400", "600"]],
];
const fontCss = FONTS.flatMap(([pkg, weights]) =>
  weights.map((w) => readFileSync(join(modules, "@fontsource", pkg, `${w}.css`), "utf8").replaceAll("url(./files/", `url(/__fonts/@fontsource/${pkg}/files/`)),
).join("\n");
const STYLE = `${fontCss}
@font-face { font-family: "Twemoji"; src: url(/__fonts/twemoji-colr-font/twemoji.woff2) format("woff2"); }
:root {
  --font: "Noto Sans", "Noto Sans Arabic", "Twemoji", sans-serif !important;
  --mono: "Noto Sans Mono", "Twemoji", monospace !important;
}
*, *::before, *::after, ::backdrop {
  animation: none !important;
  transition: none !important;
  caret-color: transparent !important;
  /* Blurs are drawn a little differently each time; without them the app looks the same, only less frosted. */
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}`;

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".json": "application/json" };

// The UI with the invented data, built once.
const dist = mkdtempSync(join(tmpdir(), "oopz-shots-"));
process.env.VITE_BACKEND = "mock";
await build({ root: desktop, logLevel: "warn", build: { outDir: dist, emptyOutDir: true } });

/** Serves the built UI and the fonts to the page; nothing else is reachable. */
async function serve(context) {
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== ORIGIN) return route.abort();
    const path = decodeURIComponent(url.pathname);
    const file = path.startsWith("/__fonts/") ? join(modules, path.slice("/__fonts/".length)) : join(dist, path === "/" ? "index.html" : path);
    if (!(file.startsWith(dist) || file.startsWith(modules)) || !existsSync(file)) return route.fulfill({ status: 404, body: "" });
    return route.fulfill({ status: 200, contentType: TYPES[extname(file)] ?? "application/octet-stream", body: readFileSync(file) });
  });
}

/** Writes the picture unless the one there has the same pixels; returns whether it wrote. */
function writeIfChanged(path, png) {
  if (existsSync(path)) {
    const a = PNG.sync.read(readFileSync(path));
    const b = PNG.sync.read(png);
    if (a.width === b.width && a.height === b.height && a.data.equals(b.data)) return false;
  }
  writeFileSync(path, png);
  return true;
}

/** Makes one picture in one language. */
async function take(browser, lang, shot) {
  const vp = VIEWPORTS[shot.viewport ?? "desktop"];
  const theme = shot.theme ?? "light";
  // On a phone the app says "this phone" instead of "this computer".
  const t = vp.mobile ? { ...LOCALES[lang].strings, ...LOCALES[lang].phone } : LOCALES[lang].strings;
  const context = await browser.newContext({
    viewport: vp.size,
    deviceScaleFactor: vp.scale,
    isMobile: vp.mobile,
    hasTouch: vp.mobile,
    locale: LOCALES[lang].code,
    timezoneId: "Europe/Madrid",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  try {
    await serve(context);
    await context.addInitScript(
      ([lang, theme, welcome, css]) => {
        localStorage.setItem("oopz.lang", lang);
        localStorage.setItem("oopz.theme", theme);
        if (!welcome) localStorage.setItem("oopz.consent", "yes");
        document.addEventListener("DOMContentLoaded", () => {
          const style = document.createElement("style");
          style.textContent = css;
          document.head.appendChild(style);
        });
      },
      [lang, theme, !!shot.welcome, STYLE],
    );
    const page = await context.newPage();
    page.setDefaultTimeout(15_000);
    await page.clock.setFixedTime(new Date(CLOCK));
    const params = new URLSearchParams(shot.query ?? {});
    if (vp.mobile) params.set("platform", "android");
    await page.goto(`${ORIGIN}/?${params}`);
    await page.locator("#splash").waitFor({ state: "detached" });
    await shot.steps?.(helpers(page, t));
    // Done once the search index is built and nothing is opening.
    await page.locator(".building, .opening").first().waitFor({ state: "detached" });
    // The one number that depends on the machine: how long a search took ("· 21 ms"), shown as 3 ms.
    await page.evaluate(() => {
      const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n = walk.nextNode(); n; n = walk.nextNode()) n.data = n.data.replace(/· \d+ ms\b/, "· 3 ms");
    });
    await page.evaluate(() => document.fonts.ready);
    await page.mouse.move(-10, -10);
    const png = await page.screenshot({ animations: "disabled", caret: "hide" });
    return writeIfChanged(join(out, lang, `${shot.name}.png`), png);
  } finally {
    await context.close();
  }
}

let written = 0;
const failed = [];
try {
  for (const lang of Object.keys(LOCALES)) {
    mkdirSync(join(out, lang), { recursive: true });
    // One browser per language: its own language sets how date fields read (dd/mm/aaaa).
    // The other flags draw everything on the processor, the same way each time.
    const browser = await chromium.launch({
      args: [
        `--lang=${LOCALES[lang].code}`,
        "--disable-gpu",
        "--disable-gpu-rasterization",
        "--disable-partial-raster",
        "--disable-skia-runtime-opts",
        "--force-color-profile=srgb",
        "--font-render-hinting=none",
        "--disable-lcd-text",
      ],
    });
    try {
      const queue = shots.filter((s) => !only.length || only.includes(s.name));
      const worker = async () => {
        for (let shot = queue.shift(); shot; shot = queue.shift()) {
          try {
            if (await take(browser, lang, shot)) {
              written++;
              console.log(`wrote ${lang}/${shot.name}.png`);
            }
          } catch (e) {
            failed.push(`${lang}/${shot.name}`);
            console.error(`${lang}/${shot.name}: ${e.message.split("\n").slice(0, 3).join(" / ")}`);
          }
        }
      };
      await Promise.all(Array.from({ length: AT_ONCE }, worker));
    } finally {
      await browser.close();
    }
    // Pictures no entry makes any more.
    if (!only.length) {
      for (const f of readdirSync(join(out, lang))) {
        if (f.endsWith(".png") && !shots.some((s) => `${s.name}.png` === f)) {
          rmSync(join(out, lang, f));
          console.log(`removed ${lang}/${f}`);
        }
      }
    }
  }
} finally {
  rmSync(dist, { recursive: true, force: true });
}
console.log(`${written} picture(s) written, ${failed.length} failed${failed.length ? `: ${failed.join(", ")}` : ""}.`);
if (failed.length) process.exit(1);

/** What a shot's steps can use: the page, the language's words, and a few common moves. */
function helpers(page, t) {
  return {
    page,
    t,
    /** Clicks the sidebar, tab bar or phone menu item with this label (the first one shown). */
    nav: (label) =>
      page
        .locator(`.side button[title="${label}"], nav.tabbar button:has-text("${label}"), .content button.row:has-text("${label}")`)
        .filter({ visible: true })
        .first()
        .click(),
    /** Clicks the button shown with this text: exactly this, else the first that contains it. */
    click: async (text) => {
      const exact = page.getByRole("button", { name: text, exact: true }).filter({ visible: true });
      await ((await exact.count()) ? exact : page.getByRole("button", { name: text }).filter({ visible: true })).first().click();
    },
    /** Waits for this text on screen. */
    see: (text) => page.getByText(text).first().waitFor(),
  };
}
