// What a language brings. To add one: copy es.ts to <code>.ts, say every word again (the type checks no key is missing),
// set its name and first day of the week, say the adapters' notes again, and add it to LOCALES in index.ts.
import type { PhoneStrings, Strings } from "./en.ts";

/** An adapter's import note (English) matched, and said again with its parts (numbers, names, `codes`). */
export type NoteRule = [RegExp, (...parts: string[]) => string];

export interface Locale {
  /** A BCP 47 code: the app's dates and numbers are formatted with it. */
  code: string;
  /** The language's own name, as the language picker shows it. */
  name: string;
  /** The first day of the week in weekday charts: 0 Sunday, 1 Monday. */
  firstDay: 0 | 1;
  strings: Strings;
  /** On a phone, the words that say "this phone" instead of "this computer". */
  phone: PhoneStrings;
  /** The adapters' import notes; empty when English is fine as written. */
  notes: NoteRule[];
  /** Units inside a note's "N × what" (`message type` → `tipo de mensaje`). */
  noteWords: [RegExp, string][];
}
