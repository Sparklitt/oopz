# Adapters

One page per format an adapter reads (decision [0013](../decisions/0013-one-model-for-every-app.md)): what's read, how
it maps to the model, what's refused, what was tested and what still needs a real export to confirm.

| Adapter | Format | Page |
|---|---|---|
| `whatsapp-export` | WhatsApp "Export chat" (txt, zip, folder) | [whatsapp-export.md](whatsapp-export.md) |
| `telegram-json` | Telegram Desktop JSON export (`result.json`) | [telegram-json.md](telegram-json.md) |
| `sms-xml` | SMS Backup & Restore XML (messages and calls) | [sms-xml.md](sms-xml.md) |
| `line-export` | LINE "Export chat history" (txt) | [line-export.md](line-export.md) |
| `viber-csv` | Viber "Export message history" (zip of CSV) | [viber-csv.md](viber-csv.md) |

Test data for each comes from `oopz-gen --export FORMAT OUT` (whatsapp-txt, telegram-json, sms-xml, line-txt,
viber-csv): invented people, every odd line the format allows, and a `manifest.txt` of counts by kind.
