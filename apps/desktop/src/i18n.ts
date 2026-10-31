// Every word the app shows, in English and Spanish. Plain words: the person may never have seen a backup file.

const en = {
  steps: ["Welcome", "Get backup", "Key", "Check", "Explore", "Merge", "Phone 2", "Done"],
  next: "Continue",
  back: "Back",
  cancel: "Cancel",

  welcomeTitle: "Your chats, back where they belong",
  welcomeLead:
    "oopz opens your own WhatsApp backups with your own 64-digit key, tells you what is inside, and can put a lost chat back. Everything stays on this computer.",
  rulesTitle: "Before we start",
  rules: [
    "Only your own backups, opened with your own key. To use someone else's backup, they give it to you and type their own key, knowing what you will do.",
    "oopz never gets around WhatsApp's encryption. Without the key, nothing can be opened, by oopz or anyone.",
    "Your original files are never changed. oopz only reads them; every result is a new folder.",
    "Nothing leaves this computer. oopz has no account, no cloud and no tracking.",
  ],
  consent: "These are my own backups, or their owner agreed and will type their own key.",

  getTitle: "Get your backup onto this computer",
  getLead: "WhatsApp keeps its backups on the phone. Pick how you want to bring them here.",
  fromFolder: "From a folder or file",
  fromFolderText:
    "Copied with a USB cable, an SD card or USB stick, Quick Share, or a cloud folder you already use. On the phone the folder is Android/media/com.whatsapp/WhatsApp/Databases.",
  pickFolder: "Choose a folder",
  pickFile: "Choose a backup file",
  fromWifi: "Over Wi-Fi with a QR code",
  fromWifiText:
    "This computer opens a private page on your Wi-Fi for a few minutes. Scan the code with the phone, pick the files, done. Nothing goes through the internet.",
  startWifi: "Show the QR code",
  wifiWhere: "First choose where the received files go (a new folder is made inside it).",
  wifiScan: "Scan this with the phone's camera, open the page, and pick the files from Databases.",
  wifiOr: "Or type this address in the phone's browser:",
  wifiWarn:
    "Use your own home Wi-Fi. Backups are encrypted, but photos and videos are not: for Media, a USB cable is safer.",
  wifiReceiving: "Receiving",
  wifiReceived: "Received (size checked)",
  wifiStop: "Stop receiving",
  wifiStopped: {
    done: "The phone said it's done.",
    stopped: "Stopped.",
    idle: "Stopped after 10 minutes without anything arriving.",
    refused: "Stopped: too many wrong requests from the network.",
  } as Record<string, string>,
  wifiUseThese: "Use the received files",
  driveNote:
    "Google Drive backups can't be opened by anyone but WhatsApp, and oopz doesn't try. Use the backup on the phone itself.",
  found: "Backups found",
  newest: "newest",
  kind: {
    crypt15: "can be opened",
    crypt14: "older format (crypt14): needs a key file only a rooted phone can read; not supported",
    crypt12: "older format (crypt12): not supported",
    not_crypt15: "named like a backup but doesn't look like one (incomplete copy?)",
  } as Record<string, string>,
  choose: "Open this one",

  keyTitle: "Your 64-digit key",
  keyLead:
    "When you turned on end-to-end encrypted backups, WhatsApp showed you 64 digits and letters (0-9, a-f). Type or paste them. Spaces don't matter.",
  keyLabel: "Key for",
  keyDigits: "characters",
  keyNever: "The key is used once to open the backup and then wiped from memory. It is never saved.",
  keyOpen: "Open the backup",
  keyOpening: "Opening and checking… a large backup can take a minute.",
  passwordQ: "Did you choose a password instead of a 64-digit key?",
  passwordTitle: "Backups protected with a password can't be opened offline",
  passwordText:
    "With a password, the real key is kept by WhatsApp on its servers, and only the WhatsApp app can fetch it. oopz will not imitate WhatsApp to get it: that would get around WhatsApp's security and could put your account at risk. You can switch to a 64-digit key inside WhatsApp in two minutes:",
  passwordSteps: [
    "On the phone, open WhatsApp › Settings › Chats › Chat backup › End-to-end encrypted backup.",
    "Choose to use a 64-digit encryption key instead (WhatsApp asks for your current password).",
    "Write the 64 digits down on paper and keep them safe. Nobody can recover them for you.",
    "Back on the Chat backup screen, tap Back up to make a fresh backup with the new key.",
    "Copy the new msgstore.db.crypt15 to this computer and continue here.",
  ],
  passwordEdge:
    "Forgot the password? Then nobody can open those backups, not even WhatsApp support. Older backups made with the password stay unreadable; only new ones use the key. Make the new backup before you uninstall or reset anything.",

  checkTitle: "What's inside",
  sound: "The backup opened and its database is sound.",
  unsound: "The backup opened, but its database has problems:",
  chats: "chats",
  messages: "messages",
  oneToOne: "one-to-one",
  groups: "groups",
  from: "from",
  to: "to",
  chatTable: ["#", "Who", "Messages", "Last message"],
  who: {
    contact: "contact",
    contact_lid: "contact (private id)",
    group: "group",
    broadcast: "broadcast list",
    other: "other",
  } as Record<string, string>,
  masked: "Ids are masked: only their last 3 characters are shown.",
  showMore: "Show more",
  fingerprint: "Fingerprint of the original file (SHA-256):",
  checkAnother: "Check another backup",
  toExplore: "Explore and search",
  toMerge: "Merge a lost chat",
  mergeNeedsTwo: "To merge, check two backups: the one that has the chat, and the one that lost it.",
  forget: "Close",

  exploreTitle: "Explore",
  exploreBuilding: "Building the search index… once per backup, a few seconds.",
  exploreBuilt: "Search index ready",
  exploreScope: "Show",
  wholeBackup: "the whole backup",
  searchPlaceholder: "Search your messages (accents don't matter)",
  searchHits: "results, newest first",
  searchMore: "(showing the first 200)",
  you: "You",
  them: "Them",
  fromMe: "sent by you",
  fromOthers: "received",
  audioMinutes: "minutes of voice notes and audio",
  byMonth: "Messages per month",
  byHour: "Messages by hour of the day (this computer's time zone)",
  byKind: "By type",
  kindTable: ["Type", "Messages", "Media size"],
  senders: "Who writes most (ids masked)",
  viewer: "Chat viewer",
  openViewer: "Open in the viewer",
  kinds: { 0: "text", 1: "photo", 2: "audio / voice", 3: "video", 4: "contact", 5: "location", 7: "system", 9: "document", 13: "GIF", 15: "deleted", 16: "live location", 20: "sticker", 42: "view once", 46: "poll", 81: "video note", 90: "call" } as Record<number, string>,
  screenOnly: "Message text is shown here on your screen only. oopz never writes it to a log or sends it anywhere.",

  mergeTitle: "Merge a lost chat",
  mergeLead:
    "Messages of one chat that the other backup lacks are added to a new copy of it, checked before anything is written. Only one-to-one chats for now.",
  source: "The backup that still has the chat",
  target: "The backup that lost it (a new copy is made)",
  chat: "Chat",
  targetKey: "Key of the backup that lost it (asked again: oopz never keeps keys)",
  outFolder: "Where to put the result",
  mediaFolder: "Optional: the Media folder of the phone that has the chat",
  chooseFolder: "Choose…",
  none: "none",
  runMerge: "Merge into a new copy",
  merging: "Merging and checking…",
  mergeDone: "Done. The new backup passed every check.",
  added: "added",
  alreadyThere: "already there",
  untouched: "Original backups unchanged (fingerprint checked again).",
  mediaCopied: "media files copied",
  mediaMissing: "missing",
  toPhone2: "Put it on the phone",

  phoneTitle: "Put the result on the phone",
  phoneLead: "Do these in order. Tick each one when it's done.",
  phoneSteps: [
    ["Mind the nightly backup", "Every night around 2 a.m. WhatsApp makes a local backup by itself (there is no switch for it): it replaces msgstore.db.crypt15 and deletes dated backups older than about a week. Copy the Databases folder off the phone now, set Back up to Google to Never on both phones while you work, and finish the same day."],
    ["Keep a copy of everything", "Copy the phone's whole WhatsApp folder to this computer first. It's your way back."],
    ["Same phone number", "The phone that gets the chat must use the same number as the backup's owner."],
    ["Don't open WhatsApp yet", "Install WhatsApp but don't open it. If it was already set up, move its WhatsApp folder aside and clear WhatsApp's storage in Android settings."],
    ["Copy the result", "Copy Databases/msgstore.db.crypt15 from the result folder into Android/media/com.whatsapp/WhatsApp/Databases on the phone (create the folders if needed), and Media into …/WhatsApp/Media."],
    ["Open WhatsApp and restore", "Verify the number. If WhatsApp offers a Google Drive backup, that's not the merged one: the surest way is to remove the Google account from the phone for now (Settings › Accounts) before opening WhatsApp, so it finds only the local backup. Tap Restore, type the 64-digit key, then add the account back."],
    ["Check", "Open the chat and see the messages. If WhatsApp refuses the backup, nothing is lost: see the plan's 'restore refused' steps and put your copy back."],
  ],
  phoneDone: "All done",

  doneTitle: "All done",
  doneLead: "Your originals were never changed. Keep the result folder and your copies until you're sure everything is there.",
  startOver: "Start again",

  errors: {
    key_format: "The key must be 64 characters: digits 0-9 and letters a-f.",
    wrong_key:
      "This key doesn't open this backup. Check each character. If you're sure it's right, the file may be incomplete: copy it again and compare its size with the phone's.",
    not_crypt15: "This file isn't a WhatsApp crypt15 backup, or it's incomplete.",
    crypt14: "This is an older crypt14 backup. Its key file is only readable on a rooted phone, so oopz can't open it.",
    damaged: "The backup opened but its database is damaged. Try an older backup (msgstore-YYYY-MM-DD.1.db.crypt15).",
    unsupported: "oopz can't do this with this backup:",
    merge_check: "The merged backup failed a check, so nothing was written:",
    changed: "The backup file changed since it was checked. Check it again.",
    exists: "That folder already exists; choose another place.",
    io: "A file couldn't be read or written (disk full, cable unplugged, no permission?):",
    not_found: "Nothing found:",
    network: "The Wi-Fi page couldn't start:",
  } as Record<string, string>,
};

export type Strings = typeof en;

const es: Strings = {
  steps: ["Inicio", "Copia", "Clave", "Revisar", "Explorar", "Unir", "Teléfono 2", "Listo"],
  next: "Continuar",
  back: "Atrás",
  cancel: "Cancelar",

  welcomeTitle: "Tus chats, de vuelta en su sitio",
  welcomeLead:
    "oopz abre tus propias copias de seguridad de WhatsApp con tu propia clave de 64 dígitos, te dice qué contienen y puede devolver un chat perdido. Todo se queda en este ordenador.",
  rulesTitle: "Antes de empezar",
  rules: [
    "Solo tus propias copias, abiertas con tu propia clave. Para usar la copia de otra persona, te la da ella y escribe su propia clave, sabiendo lo que vas a hacer.",
    "oopz nunca se salta el cifrado de WhatsApp. Sin la clave no se puede abrir nada, ni oopz ni nadie.",
    "Tus archivos originales nunca se modifican. oopz solo los lee; cada resultado es una carpeta nueva.",
    "Nada sale de este ordenador. oopz no tiene cuentas, ni nube, ni seguimiento.",
  ],
  consent: "Son mis propias copias, o su dueño está de acuerdo y escribirá su propia clave.",

  getTitle: "Trae tu copia a este ordenador",
  getLead: "WhatsApp guarda sus copias en el teléfono. Elige cómo traerlas.",
  fromFolder: "Desde una carpeta o archivo",
  fromFolderText:
    "Copiada con un cable USB, una tarjeta SD o memoria USB, Quick Share o una carpeta en la nube que ya uses. En el teléfono la carpeta es Android/media/com.whatsapp/WhatsApp/Databases.",
  pickFolder: "Elegir una carpeta",
  pickFile: "Elegir un archivo de copia",
  fromWifi: "Por Wi-Fi con un código QR",
  fromWifiText:
    "Este ordenador abre una página privada en tu Wi-Fi durante unos minutos. Escanea el código con el teléfono, elige los archivos y listo. Nada pasa por internet.",
  startWifi: "Mostrar el código QR",
  wifiWhere: "Primero elige dónde guardar lo que llegue (se crea una carpeta nueva dentro).",
  wifiScan: "Escanéalo con la cámara del teléfono, abre la página y elige los archivos de Databases.",
  wifiOr: "O escribe esta dirección en el navegador del teléfono:",
  wifiWarn:
    "Usa el Wi-Fi de tu casa. Las copias van cifradas, pero las fotos y vídeos no: para Media, el cable USB es más seguro.",
  wifiReceiving: "Recibiendo",
  wifiReceived: "Recibido (tamaño comprobado)",
  wifiStop: "Dejar de recibir",
  wifiStopped: {
    done: "El teléfono dijo que ha terminado.",
    stopped: "Detenido.",
    idle: "Detenido tras 10 minutos sin recibir nada.",
    refused: "Detenido: demasiadas peticiones erróneas desde la red.",
  },
  wifiUseThese: "Usar los archivos recibidos",
  driveNote:
    "Las copias de Google Drive solo las puede abrir WhatsApp, y oopz no lo intenta. Usa la copia que está en el propio teléfono.",
  found: "Copias encontradas",
  newest: "la más reciente",
  kind: {
    crypt15: "se puede abrir",
    crypt14: "formato antiguo (crypt14): necesita un archivo de clave que solo un teléfono rooteado puede leer; no compatible",
    crypt12: "formato antiguo (crypt12): no compatible",
    not_crypt15: "se llama como una copia pero no lo parece (¿copia incompleta?)",
  },
  choose: "Abrir esta",

  keyTitle: "Tu clave de 64 dígitos",
  keyLead:
    "Al activar las copias cifradas de extremo a extremo, WhatsApp te mostró 64 dígitos y letras (0-9, a-f). Escríbelos o pégalos. Los espacios dan igual.",
  keyLabel: "Clave de",
  keyDigits: "caracteres",
  keyNever: "La clave se usa una vez para abrir la copia y luego se borra de la memoria. Nunca se guarda.",
  keyOpen: "Abrir la copia",
  keyOpening: "Abriendo y revisando… una copia grande puede tardar un minuto.",
  passwordQ: "¿Elegiste una contraseña en lugar de una clave de 64 dígitos?",
  passwordTitle: "Las copias con contraseña no se pueden abrir sin conexión",
  passwordText:
    "Con contraseña, la clave real la guarda WhatsApp en sus servidores, y solo la app de WhatsApp puede pedirla. oopz no se hará pasar por WhatsApp para obtenerla: eso sería saltarse la seguridad de WhatsApp y podría poner tu cuenta en riesgo. Puedes cambiar a una clave de 64 dígitos dentro de WhatsApp en dos minutos:",
  passwordSteps: [
    "En el teléfono, abre WhatsApp › Ajustes › Chats › Copia de seguridad › Copia de seguridad cifrada de extremo a extremo.",
    "Elige usar una clave de cifrado de 64 dígitos en su lugar (WhatsApp te pide tu contraseña actual).",
    "Apunta los 64 dígitos en papel y guárdalos bien. Nadie puede recuperarlos por ti.",
    "De vuelta en Copia de seguridad, toca Guardar para hacer una copia nueva con la clave nueva.",
    "Copia el nuevo msgstore.db.crypt15 a este ordenador y sigue aquí.",
  ],
  passwordEdge:
    "¿Olvidaste la contraseña? Entonces nadie puede abrir esas copias, ni el soporte de WhatsApp. Las copias antiguas hechas con contraseña siguen sin poder leerse; solo las nuevas usan la clave. Haz la copia nueva antes de desinstalar o restablecer nada.",

  checkTitle: "Qué contiene",
  sound: "La copia se abrió y su base de datos está sana.",
  unsound: "La copia se abrió, pero su base de datos tiene problemas:",
  chats: "chats",
  messages: "mensajes",
  oneToOne: "individuales",
  groups: "grupos",
  from: "desde",
  to: "hasta",
  chatTable: ["#", "Quién", "Mensajes", "Último mensaje"],
  who: {
    contact: "contacto",
    contact_lid: "contacto (id privado)",
    group: "grupo",
    broadcast: "lista de difusión",
    other: "otro",
  },
  masked: "Los ids van ocultos: solo se ven sus 3 últimos caracteres.",
  showMore: "Ver más",
  fingerprint: "Huella del archivo original (SHA-256):",
  checkAnother: "Revisar otra copia",
  toExplore: "Explorar y buscar",
  toMerge: "Unir un chat perdido",
  mergeNeedsTwo: "Para unir, revisa dos copias: la que tiene el chat y la que lo perdió.",
  forget: "Cerrar",

  exploreTitle: "Explorar",
  exploreBuilding: "Creando el índice de búsqueda… una vez por copia, unos segundos.",
  exploreBuilt: "Índice de búsqueda listo",
  exploreScope: "Mostrar",
  wholeBackup: "toda la copia",
  searchPlaceholder: "Busca en tus mensajes (las tildes dan igual)",
  searchHits: "resultados, los más recientes primero",
  searchMore: "(se muestran los 200 primeros)",
  you: "Tú",
  them: "Otros",
  fromMe: "enviados por ti",
  fromOthers: "recibidos",
  audioMinutes: "minutos de notas de voz y audio",
  byMonth: "Mensajes por mes",
  byHour: "Mensajes por hora del día (zona horaria de este ordenador)",
  byKind: "Por tipo",
  kindTable: ["Tipo", "Mensajes", "Tamaño multimedia"],
  senders: "Quién escribe más (ids ocultos)",
  viewer: "Visor del chat",
  openViewer: "Abrir en el visor",
  kinds: { 0: "texto", 1: "foto", 2: "audio / voz", 3: "vídeo", 4: "contacto", 5: "ubicación", 7: "sistema", 9: "documento", 13: "GIF", 15: "eliminado", 16: "ubicación en tiempo real", 20: "sticker", 42: "ver una vez", 46: "encuesta", 81: "videonota", 90: "llamada" },
  screenOnly: "El texto de los mensajes se muestra solo aquí, en tu pantalla. oopz nunca lo escribe en un registro ni lo envía a ningún sitio.",

  mergeTitle: "Unir un chat perdido",
  mergeLead:
    "Los mensajes de un chat que le faltan a la otra copia se añaden a una copia nueva de esta, comprobada antes de escribir nada. Por ahora solo chats individuales.",
  source: "La copia que todavía tiene el chat",
  target: "La copia que lo perdió (se hace una copia nueva)",
  chat: "Chat",
  targetKey: "Clave de la copia que lo perdió (se pide otra vez: oopz nunca guarda claves)",
  outFolder: "Dónde poner el resultado",
  mediaFolder: "Opcional: la carpeta Media del teléfono que tiene el chat",
  chooseFolder: "Elegir…",
  none: "ninguna",
  runMerge: "Unir en una copia nueva",
  merging: "Uniendo y comprobando…",
  mergeDone: "Hecho. La copia nueva pasó todas las comprobaciones.",
  added: "añadidos",
  alreadyThere: "ya estaban",
  untouched: "Copias originales sin cambios (huella comprobada otra vez).",
  mediaCopied: "archivos multimedia copiados",
  mediaMissing: "faltan",
  toPhone2: "Ponerla en el teléfono",

  phoneTitle: "Pon el resultado en el teléfono",
  phoneLead: "Hazlo en este orden. Marca cada paso al terminarlo.",
  phoneSteps: [
    ["Cuidado con la copia nocturna", "Cada noche hacia las 2 de la mañana WhatsApp hace una copia local por su cuenta (no hay opción para quitarla): reemplaza msgstore.db.crypt15 y borra las copias con fecha de más de una semana. Saca ya la carpeta Databases del teléfono, pon Guardar en Google en Nunca en los dos teléfonos mientras trabajas, y termina el mismo día."],
    ["Guarda una copia de todo", "Copia antes toda la carpeta WhatsApp del teléfono a este ordenador. Es tu camino de vuelta."],
    ["El mismo número", "El teléfono que recibe el chat debe usar el mismo número que el dueño de la copia."],
    ["No abras WhatsApp todavía", "Instala WhatsApp pero no lo abras. Si ya estaba configurado, aparta su carpeta WhatsApp y borra el almacenamiento de WhatsApp en los ajustes de Android."],
    ["Copia el resultado", "Copia Databases/msgstore.db.crypt15 de la carpeta del resultado a Android/media/com.whatsapp/WhatsApp/Databases en el teléfono (crea las carpetas si hace falta), y Media a …/WhatsApp/Media."],
    ["Abre WhatsApp y restaura", "Verifica el número. Si WhatsApp ofrece una copia de Google Drive, no es la unida: lo más seguro es quitar la cuenta de Google del teléfono por ahora (Ajustes › Cuentas) antes de abrir WhatsApp, para que solo encuentre la copia local. Toca Restaurar, escribe la clave de 64 dígitos y luego vuelve a añadir la cuenta."],
    ["Comprueba", "Abre el chat y mira los mensajes. Si WhatsApp rechaza la copia, no se pierde nada: mira los pasos de 'restauración rechazada' del plan y vuelve a poner tu copia."],
  ],
  phoneDone: "Todo hecho",

  doneTitle: "Todo listo",
  doneLead: "Tus originales nunca se modificaron. Guarda la carpeta del resultado y tus copias hasta estar seguro de que todo está.",
  startOver: "Empezar de nuevo",

  errors: {
    key_format: "La clave debe tener 64 caracteres: dígitos 0-9 y letras a-f.",
    wrong_key:
      "Esta clave no abre esta copia. Revisa cada carácter. Si estás seguro de que es correcta, el archivo puede estar incompleto: cópialo otra vez y compara su tamaño con el del teléfono.",
    not_crypt15: "Este archivo no es una copia crypt15 de WhatsApp, o está incompleto.",
    crypt14: "Es una copia crypt14 antigua. Su archivo de clave solo se puede leer en un teléfono rooteado, así que oopz no puede abrirla.",
    damaged: "La copia se abrió pero su base de datos está dañada. Prueba una copia anterior (msgstore-AAAA-MM-DD.1.db.crypt15).",
    unsupported: "oopz no puede hacer esto con esta copia:",
    merge_check: "La copia unida no pasó una comprobación, así que no se escribió nada:",
    changed: "El archivo de la copia cambió desde que se revisó. Revísalo otra vez.",
    exists: "Esa carpeta ya existe; elige otro sitio.",
    io: "No se pudo leer o escribir un archivo (¿disco lleno, cable desconectado, sin permiso?):",
    not_found: "No se encontró nada:",
    network: "La página Wi-Fi no pudo arrancar:",
  },
};

export type Lang = "en" | "es";

export function initialLang(): Lang {
  try {
    const saved = localStorage.getItem("oopz.lang");
    if (saved === "en" || saved === "es") return saved;
  } catch {
    // Storage blocked: fall back to the system language.
  }
  return navigator.language.toLowerCase().startsWith("es") ? "es" : "en";
}

export function strings(lang: Lang): Strings {
  return lang === "es" ? es : en;
}
