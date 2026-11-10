// The app's languages. Each lives in its own file (en.ts, es.ts, …) with all its words; this file only lists them and
// picks one. The Rust side never sends words to show: errors come as codes, and import notes in English are said again
// by each language's `notes` (see types.ts to add a language).
import { locale as en, type Strings } from "./en.ts";
import { locale as es } from "./es.ts";
import type { Locale } from "./types.ts";

export type { Strings } from "./en.ts";
export type { Locale } from "./types.ts";

export const LOCALES = { en, es } satisfies Record<string, Locale>;
export type Lang = keyof typeof LOCALES;
const FALLBACK: Lang = "en";

/** The languages for a picker: code and own name. */
export const LANGS: [Lang, string][] = (Object.keys(LOCALES) as Lang[]).map((l) => [l, LOCALES[l].name]);

const isLang = (s: string | null | undefined): s is Lang => !!s && Object.prototype.hasOwnProperty.call(LOCALES, s);

let phoneWords = false;

/** Called once at start, before the first screen: the words for a phone instead of a computer. */
export function usePhoneWords(on: boolean) {
  phoneWords = on;
}

/** The saved choice, else the first of the system's languages the app speaks, else English. */
export function initialLang(): Lang {
  try {
    const saved = localStorage.getItem("oopz.lang");
    if (isLang(saved)) return saved;
  } catch {
    // Storage blocked: fall back to the system language.
  }
  const wanted = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const tag of wanted) {
    const base = tag.toLowerCase().split("-")[0];
    if (isLang(base)) return base;
  }
  return FALLBACK;
}

const withPhone = new Map<Lang, Strings>();

export function strings(lang: Lang): Strings {
  const l = LOCALES[lang] ?? LOCALES[FALLBACK];
  if (!phoneWords) return l.strings;
  let s = withPhone.get(lang);
  if (!s) withPhone.set(lang, (s = { ...l.strings, ...l.phone }));
  return s;
}

export function locale(lang: Lang): Locale {
  return LOCALES[lang] ?? LOCALES[FALLBACK];
}

/** An adapter's import note in this language; a note the language doesn't know is shown as written. */
export function noteText(note: string, lang: Lang): string {
  const { notes, noteWords } = locale(lang);
  for (const [re, say] of notes) {
    const m = re.exec(note);
    if (m) return say(...m.slice(1).map((part) => noteWords.reduce((s, [w, said]) => s.replace(w, said), part)));
  }
  return note;
}
