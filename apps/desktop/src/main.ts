// The oopz desktop UI. It only shows results and sends commands; the work happens behind the Backend interface
// (src/lib/backend): Rust in the app, invented data with VITE_BACKEND=mock.
import "./polyfills";
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import { loadBackend } from "./lib/backend";
import { app, checkUpdates, setBackend } from "./lib/state.svelte";

// No top-level await: Chrome 67 (an old phone's WebView) doesn't have it.
void loadBackend().then((backend) => {
  setBackend(backend);
  mount(App, { target: document.getElementById("app")! });
  if (app.updates.auto) void checkUpdates(true);
});
