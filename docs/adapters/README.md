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
| `whatsapp-ios`, `whatsapp-business-ios` | An iPhone's Finder/iTunes backup, encrypted ones with the owner's password | [whatsapp-ios.md](whatsapp-ios.md) |

WhatsApp Android's backup (`whatsapp-android`, in `oopz-app`) and the iPhone's read their type codes from msgkinds'
registry (decision [0020](../decisions/0020-the-shared-registry.md)); the other adapters keep their own mapping until
msgkinds has their source (each has a `TODO(msgkinds)` naming it).

Test data for each comes from `oopz-gen --export FORMAT OUT` (whatsapp-txt, telegram-json, sms-xml, line-txt,
viber-csv), and `oopz-gen --ios OUT [--ios-password PW]` for an iPhone backup: invented people, every odd line the format allows, and a `manifest.txt` of counts by kind.
