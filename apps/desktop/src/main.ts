// The oopz desktop UI. It only shows results and sends commands; the work happens behind the Backend interface
// (src/lib/backend): Rust in the app, invented data with VITE_BACKEND=mock.
import "./polyfills";
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import { loadBackend } from "./lib/backend";
import { app, checkUpdates, setBackend } from "./lib/state.svelte";
import { trackBack } from "./lib/back.svelte";

// No top-level await: Chrome 67 (an old phone's WebView) doesn't have it.
void loadBackend()
  .then((backend) => {
    setBackend(backend);
    mount(App, { target: document.getElementById("app")! });
    hideSplash();
    trackBack();
    if (app.updates.auto) void checkUpdates(true);
  })
  .catch((e: unknown) => {
    // The app couldn't start: say so on the loading screen instead of spinning for ever.
    const splash = document.getElementById("splash");
    if (!splash) return;
    splash.querySelector(".dots")?.remove();
    const err = document.createElement("p");
    err.className = "err";
    err.textContent =
      (document.documentElement.lang === "es" ? "oopz no ha podido arrancar: " : "oopz couldn't start: ") + String(e);
    splash.appendChild(err);
  });

/** Fades the loading screen (index.html) out once the app is on screen, then removes it. */
function hideSplash() {
  const splash = document.getElementById("splash");
  if (!splash) return;
  requestAnimationFrame(() => splash.classList.add("gone"));
  setTimeout(() => splash.remove(), 400);
}
