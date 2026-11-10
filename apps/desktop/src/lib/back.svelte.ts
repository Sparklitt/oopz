// The Back button (Android's, a mouse's back button, Alt+Left): it goes back inside oopz instead of leaving it. The
// web view's history holds one extra entry; Back takes it, oopz undoes one step (an open sheet, an open chat, the last
// screen) and puts the entry back. With nothing left to undo, Back goes on: Android closes the app as usual.
import { app } from "./state.svelte";
import type { View } from "./state.svelte";

const screens: View[] = [];
let returning = false;

export function trackBack() {
  // Screens visited, oldest first; going back doesn't add to it.
  let shown = app.view;
  $effect.root(() => {
    $effect(() => {
      const view = app.view;
      if (view === shown) return;
      if (!returning) {
        screens.push(shown);
        if (screens.length > 50) screens.shift();
      }
      returning = false;
      shown = view;
    });
  });
  history.pushState({ oopz: "back" }, "");
  window.addEventListener("popstate", () => {
    if (stepBack()) history.pushState({ oopz: "back" }, "");
    else history.back();
  });
}

/** Undoes one step; false when there's nothing to undo. */
function stepBack(): boolean {
  // The consent sheet stays until it's answered: Back leaves the app instead.
  if (app.sheet !== null && app.sheet !== "welcome") {
    app.sheet = null;
    return true;
  }
  if (app.sheet === "welcome") return false;
  if (app.view === "chats" && app.chat !== null) {
    app.chat = null;
    return true;
  }
  const previous = screens.pop();
  if (previous === undefined) return false;
  returning = true;
  app.view = previous;
  app.error = null;
  return true;
}
