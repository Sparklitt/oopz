import type { Backend } from "./types";

/** Tauri inside the app; the invented data for `VITE_BACKEND=mock`, or a development page opened in a browser. The
 *  conditions are fixed at build time, so a production build has no mock in it. */
export async function loadBackend(): Promise<Backend> {
  const inTauri = "__TAURI_INTERNALS__" in window;
  if (import.meta.env.VITE_BACKEND === "mock" || (import.meta.env.DEV && !inTauri)) {
    return (await import("./mock")).createMock();
  }
  return (await import("./tauri")).tauri;
}
