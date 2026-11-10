// The few newer browser functions that Svelte's runtime calls, for old phones' WebViews (Chrome 67, Android 8).
// Imported first in main.ts. Each is filled only where it is missing; our own code avoids the rest.

const root = (typeof self !== "undefined" ? self : window) as unknown as Record<string, unknown>;

// Chrome 71.
if (typeof root.globalThis === "undefined") root.globalThis = root;

// Chrome 71.
if (typeof root.queueMicrotask !== "function") {
  root.queueMicrotask = (task: () => void) => {
    Promise.resolve()
      .then(task)
      .catch((e) => setTimeout(() => {
        throw e;
      }));
  };
}

// Chrome 85. Svelte calls it with a plain string; a global RegExp goes to replace() as the standard says.
if (typeof String.prototype.replaceAll !== "function") {
  Object.defineProperty(String.prototype, "replaceAll", {
    configurable: true,
    writable: true,
    value(this: string, pattern: string | RegExp, replacement: string) {
      if (pattern instanceof RegExp) return this.replace(pattern, replacement);
      return this.split(pattern).join(replacement);
    },
  });
}

// Not a function but a layout feature: flex `gap` came in Chrome 84. Without it, app.css spaces things with
// margins under .no-flex-gap.
{
  const probe = document.createElement("div");
  probe.style.cssText = "display:flex;flex-direction:column;row-gap:1px;position:absolute;visibility:hidden";
  probe.appendChild(document.createElement("div"));
  probe.appendChild(document.createElement("div"));
  document.documentElement.appendChild(probe);
  if (probe.scrollHeight !== 1) document.documentElement.classList.add("no-flex-gap");
  probe.remove();
}

export {};
