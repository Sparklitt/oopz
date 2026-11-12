// The user guide's pictures (docs/guide/*.md), made by screenshots.mjs from the UI with its invented data (the mock
// backend, src/lib/backend/mock.ts). One entry per picture, made in every language:
//   name      the file, docs/guide/images/<lang>/<name>.png
//   viewport  "desktop" (the default) or "phone" (the Android app's layout)
//   theme     "light" (the default) or "dark"
//   welcome   true: the first start, with the welcome sheet
//   query     extra address parameters for the mock (see mock.ts), e.g. { library: "empty" }
//   steps     async ({ page, t, nav, click, see }) => …: what to do before the picture; `t` is the language's words
//             (src/lib/i18n), so the same steps work in every language
// To add a picture: add an entry here, run `pnpm screenshots <name>`, and show it in the guide.

/** The page's clock: the mock's data ends on 9 November 2026. */
export const CLOCK = "2026-11-10T10:30:00+01:00";

export const VIEWPORTS = {
  desktop: { size: { width: 1280, height: 800 }, scale: 1, mobile: false },
  phone: { size: { width: 412, height: 915 }, scale: 2, mobile: true },
};

/** The mock's key: it opens every invented backup. Not a real key. */
const MOCK_KEY = "0123456789abcdef".repeat(4); // gitleaks:allow: the invented mock key (comment above), not a real one

/** Opens the newest invented backup with the key, from the Add a backup screen. */
async function openBackup(s, file = "msgstore.db.crypt15") {
  const { page, t } = s;
  if (await page.locator("#unlock-key").count() === 0) {
    await s.nav(t.addBackup);
    await s.click(t.pickFolder);
    await page.locator("label", { hasText: file }).first().click();
    await s.click(t.choose);
  }
  await page.locator("#unlock-key").fill(MOCK_KEY);
  await s.click(t.keyOpen);
  await page.locator("#unlock-key").waitFor({ state: "detached" });
  await page.locator("dialog").waitFor({ state: "detached" });
}

/** Opens two backups: the newest and an older one, as for restoring a chat. */
async function openTwo(s) {
  await openBackup(s, "msgstore-2026-11-08.1.db.crypt15");
  await openBackup(s, "msgstore.db.crypt15");
}

export const shots = [
  // Starting
  { name: "welcome", welcome: true },
  { name: "add-backup" },
  {
    name: "found",
    steps: async (s) => {
      await s.click(s.t.pickFolder);
      await s.see("msgstore-2021-01-01.1.db.crypt14");
    },
  },
  {
    name: "unlock",
    steps: async (s) => {
      await s.click(s.t.pickFolder);
      await s.click(s.t.choose);
      await s.page.locator("#unlock-key").fill(MOCK_KEY.slice(0, 40));
      await s.page.locator("#unlock-key").blur();
    },
  },
  { name: "receive", steps: async (s) => s.nav(s.t.navReceive) },
  {
    name: "receive-qr",
    steps: async (s) => {
      await s.nav(s.t.navReceive);
      await s.click(s.t.startWifi);
      await s.page.locator(".qr svg").waitFor();
    },
  },
  {
    // The phone never opens the page (?wifi=silent): the tips come after 45 s, so the fixed clock moves on 46 s.
    name: "receive-tips",
    query: { wifi: "silent" },
    steps: async (s) => {
      await s.nav(s.t.navReceive);
      await s.click(s.t.startWifi);
      await s.page.locator(".qr svg").waitFor();
      await s.page.clock.setFixedTime(new Date(Date.parse(CLOCK) + 46_000));
      await s.page.getByText(s.t.wifiTipsTitle).waitFor();
    },
  },
  {
    name: "adb-copy",
    steps: async (s) => {
      await s.nav(s.t.addBackup);
      await s.click(s.t.fromAdb);
      await s.page.locator(".device").first().waitFor();
    },
  },
  {
    // No adb on this computer (?adb=none): Google's licence, shown before anything is downloaded.
    name: "adb-get",
    query: { adb: "none" },
    steps: async (s) => {
      await s.nav(s.t.addBackup);
      await s.click(s.t.fromAdb);
      await s.click(s.t.toolsShow);
      await s.page.locator("textarea.licence").waitFor();
    },
  },

  // One backup
  { name: "overview", steps: openBackup },
  {
    name: "chats",
    steps: async (s) => {
      await openBackup(s);
      await s.nav(s.t.navChats);
      await s.page.locator("button.chat").first().click();
      await s.page.locator(".scroller").waitFor();
    },
  },
  {
    name: "search",
    steps: async (s) => {
      await openBackup(s);
      await s.nav(s.t.navSearch);
      await s.page.locator(".field-search input").fill("playa");
      await s.page.keyboard.press("Enter");
      await s.page.waitForTimeout(1500);
    },
  },
  {
    name: "insights",
    steps: async (s) => {
      await openBackup(s);
      await s.nav(s.t.navInsights);
      await s.page.waitForTimeout(1000);
    },
  },
  {
    name: "export-backup",
    steps: async (s) => {
      await openBackup(s);
      await s.click(s.t.exportShort);
      await s.page.locator("dialog").waitFor();
    },
  },

  // The library
  { name: "library", steps: async (s) => s.nav(s.t.lib.navSources) },
  {
    name: "library-add",
    steps: async (s) => {
      await s.nav(s.t.lib.navSources);
      await s.click(s.t.lib.add);
      await s.page.locator("dialog").waitFor();
    },
  },
  {
    name: "library-chats",
    steps: async (s) => {
      await s.nav(s.t.lib.navSources);
      await s.click(s.t.lib.openChats);
      await s.page.waitForTimeout(800);
    },
  },
  {
    name: "timeline",
    steps: async (s) => {
      await s.nav(s.t.lib.timelineTitle);
      await s.page.waitForTimeout(800);
    },
  },
  {
    name: "contacts",
    steps: async (s) => {
      await s.nav(s.t.lib.contactsTitle);
      await s.page.waitForTimeout(800);
    },
  },
  {
    name: "select",
    steps: async (s) => {
      await s.nav(s.t.lib.selectTitle);
      await s.page.waitForTimeout(800);
    },
  },
  {
    name: "export-library",
    steps: async (s) => {
      await s.nav(s.t.lib.selectTitle);
      await s.click(s.t.lib.selectEverything);
      await s.page.waitForTimeout(500);
      await s.click(s.t.exportShort);
      await s.page.locator("dialog").waitFor();
      await s.page.waitForTimeout(500);
    },
  },

  // Restoring a chat
  {
    name: "restore",
    steps: async (s) => {
      await openTwo(s);
      await s.nav(s.t.navRestore);
      // Chat 4 lacks messages in the older backup (ids: 1 the older, 2 the newest).
      const [fromBackup, fromChat, toBackup, toChat] = await s.page.locator(".side-pick select").all();
      await fromBackup.selectOption("2");
      await fromChat.selectOption("4");
      await toBackup.selectOption("1");
      await toChat.selectOption("4");
      await s.click(s.t.compare);
      await s.page.waitForTimeout(800);
    },
  },

  // Settings
  { name: "settings", steps: async (s) => s.nav(s.t.navSettings) },
  { name: "settings-privacy", steps: async (s) => { await s.nav(s.t.navSettings); await s.click(s.t.setPrivacy); } },
  { name: "settings-updates", steps: async (s) => { await s.nav(s.t.navSettings); await s.click(s.t.updatesTitle); } },
  { name: "diagnostics", steps: async (s) => { await openBackup(s); await s.nav(s.t.navDiagnostics); } },

  // Dark theme, once
  { name: "chats-dark", theme: "dark", steps: async (s) => { await openBackup(s); await s.nav(s.t.navChats); await s.page.locator("button.chat").first().click(); await s.page.locator(".scroller").waitFor(); } },

  // The Android app
  { name: "phone-add", viewport: "phone" },
  { name: "phone-library", viewport: "phone", steps: async (s) => { await s.nav(s.t.lib.nav); await s.nav(s.t.lib.navSources); } },
  { name: "phone-library-chats", viewport: "phone", steps: async (s) => { await s.nav(s.t.lib.nav); await s.nav(s.t.navChats); await s.page.waitForTimeout(800); } },
  { name: "phone-tools", viewport: "phone", steps: async (s) => s.nav(s.t.navTools) },
];
