# Guía de oopz

[English](en.md)

oopz abre tu propio historial de chats en tu ordenador o en tu teléfono: copias de seguridad de WhatsApp y las
exportaciones que hacen otras apps. Puedes leerlo, buscar en él, exportarlo y recuperar un chat de WhatsApp perdido.
Nada sale de tu dispositivo.

Todas las imágenes de esta guía muestran personas y números inventados (Ana Prueba, Ben Ejemplo, `+9999 555 0187`).
Se sacan de la propia app, así que cambian cuando cambia la app.

- [Qué abre oopz y qué no abrirá nunca](#qué-abre-oopz-y-qué-no-abrirá-nunca)
- [Sacar la copia del teléfono](#sacar-la-copia-del-teléfono)
- [Tu clave de 64 dígitos](#tu-clave-de-64-dígitos)
- [Abrir una copia](#abrir-una-copia)
- [Leer, buscar y estadísticas](#leer-buscar-y-estadísticas)
- [La biblioteca: todas tus apps juntas](#la-biblioteca-todas-tus-apps-juntas)
- [Exportar](#exportar)
- [Recuperar un chat perdido](#recuperar-un-chat-perdido)
- [Ajustes, privacidad y actualizaciones](#ajustes-privacidad-y-actualizaciones)
- [La línea de comandos](#la-línea-de-comandos)
- [Si algo falla](#si-algo-falla)

## Qué abre oopz y qué no abrirá nunca

| Tienes | Lo que necesita oopz |
|---|---|
| WhatsApp en Android | la copia cifrada de extremo a extremo, `msgstore.db.crypt15`, y su clave de 64 dígitos |
| WhatsApp en iPhone | la copia que hacen Finder, Dispositivos Apple o iTunes en tu ordenador, y su contraseña si está cifrada |
| WhatsApp en cualquier teléfono | el **Exportar chat** de un chat (`.txt` o `.zip`) |
| Telegram | la exportación de Telegram Desktop en JSON (`result.json`) |
| SMS y llamadas (Android) | los `.xml` de la app SMS Backup & Restore |
| LINE | **Exportar historial de chat** (`.txt`) |
| Viber | **Exportar historial de mensajes** (un zip de archivos CSV) |

oopz no abre nunca:

- **Copias de Google Drive o iCloud.** Solo WhatsApp puede leerlas, y oopz no lo intenta. Usa la copia que hay en el
  teléfono.
- **Copias antiguas de WhatsApp, `crypt12` y `crypt14`.** Su clave está en un archivo que solo se puede leer en un
  teléfono rooteado. oopz las muestra en la lista y te lo explica.
- **Una copia cuya clave se ha perdido.** Sin la clave nadie puede abrir una copia crypt15, y oopz nunca intentará
  adivinarla.
- **La copia de otra persona sin ella.** oopz es para tus propios datos. Para usar la copia de otra persona (para
  recuperar un chat), te la tiene que dar y escribir ella misma su clave.

## Sacar la copia del teléfono

En Android, WhatsApp guarda sus copias en `Android/media/com.whatsapp/WhatsApp/Databases/` (en teléfonos antiguos,
`WhatsApp/Databases/`). La más reciente es `msgstore.db.crypt15`; las otras, `msgstore-AAAA-MM-DD.1.db.crypt15`, son
de los últimos días. Si puedes, copia la carpeta `Databases` entera. Para ver nombres en lugar de números, copia
también `wa.db.crypt15` si está.

Antes, haz una copia nueva: WhatsApp › Ajustes › Chats › Copia de seguridad › **Guardar**.

Luego elige la forma que mejor te venga.

### Con un cable

Conecta el teléfono al ordenador y elige **Transferencia de archivos** en el teléfono. En Windows y Linux el teléfono
aparece como una unidad: abre la carpeta de arriba y copia `Databases` al ordenador. En un Mac, usa Android File
Transfer u OpenMTP. Si la carpeta sale vacía, desconecta, vuelve a conectar y desbloquea el teléfono.

### Tarjeta SD o memoria USB

Desde la app Archivos del teléfono, copia la carpeta `Databases` a una tarjeta SD o a una memoria USB (con un
adaptador OTG) y luego conéctala al ordenador.

### Quick Share o una carpeta en la nube

Comparte los archivos con Quick Share (Compartir con Nearby) desde la app Archivos, o déjalos en una carpeta en la
nube que uses (Google Drive como carpeta, Dropbox, OneDrive) y descárgalos en el ordenador. La copia va cifrada, así
que es bastante seguro; las fotos y los vídeos no van cifrados, así que para eso mejor el cable.

### Por Wi-Fi con un código QR

En oopz, en el ordenador: **Recibir del teléfono** (o **Añadir copia › Por Wi-Fi con un código QR**), elige dónde
dejar los archivos y pulsa **Mostrar el código QR**. Escanéalo con la cámara del teléfono: se abre una página en el
navegador del teléfono; elige allí los archivos de la copia. Llegan a una carpeta nueva y oopz te ofrece abrirlos. La
página solo existe en tu Wi-Fi, durante unos minutos y mientras esta pantalla siga abierta.

![Recibir del teléfono, antes del código QR](images/es/receive.png)

![El código QR, esperando al teléfono](images/es/receive-qr.png)

La página funciona sin internet: el ordenador y el teléfono solo tienen que compartir una red, aunque sea un router
sin internet. Sin Wi-Fi cerca, activa el punto de acceso del teléfono y conecta el ordenador a él (no gasta datos
móviles). Si el ordenador tiene varias tarjetas de red, elige la de Wi-Fi en **Dirección**; las marcadas «¿virtual?»
(WSL, Docker, VPN) casi nunca funcionan. Cuando el teléfono carga la página, oopz dice **El teléfono abrió la
página**.

Si el teléfono no consigue abrir la página:

- Los dos tienen que estar en **la misma Wi-Fi**. Quita un momento los datos móviles del teléfono para que no lo
  intente por ahí.
- Las **redes de invitados** y muchas redes públicas o de oficina aíslan los dispositivos entre sí. Usa la Wi-Fi de
  casa, o el punto de acceso del teléfono con el ordenador conectado a él.
- Puede bloquearlo un **cortafuegos**. En Windows, permite oopz en redes **privadas** cuando te lo pregunte (o en
  Seguridad de Windows › Firewall) y comprueba que tu red de casa está marcada como privada. En macOS, permite las
  conexiones entrantes para oopz.
- Una **VPN** en cualquiera de los dos puede esconder al otro. Páusala.
- Escribe a mano en el navegador del teléfono la dirección que aparece bajo el código, tal cual, con `http://`.
- ¿Sigue sin ir? Usa un cable, o adb (abajo).

Si en 45 segundos el teléfono no llega, los mismos consejos salen en la pantalla:

![Los consejos de Wi-Fi, cuando el teléfono no ha abierto la página](images/es/receive-tips.png)

### Copiar con adb (USB o depuración inalámbrica)

adb es la herramienta de Android para desarrolladores; copia archivos sin instalar nada en el teléfono. Sirve cuando
el teléfono no aparece como unidad o la página Wi-Fi no conecta.

1. En el teléfono: Ajustes › Información del teléfono, toca siete veces **Número de compilación**. Después, en
   Ajustes › Opciones para desarrolladores, activa **Depuración USB** (o **Depuración inalámbrica**, desde Android 11,
   en la misma Wi-Fi que el ordenador).
2. En oopz: **Añadir copia › Con depuración USB o inalámbrica (adb)**.
3. La primera vez, si este ordenador no tiene adb, oopz se ofrece a traerlo: **Ver la licencia de Google** muestra la
   versión, el tamaño y la licencia de Google; **Aceptar y descargar** baja las platform-tools de Android desde Google
   (una vez, unos 8–17 MB), comprueba el archivo con la suma de control de Google y lo guarda en la carpeta de oopz.
   No se descarga nada antes de aceptar. **Ajustes › General › adb de Android (platform-tools) › Quitar** lo borra.
4. Conecta el teléfono. Con cable: enchúfalo y toca **Permitir** en el teléfono. Sin cable: abre **Depuración
   inalámbrica** en oopz, toca **Vincular dispositivo con código de vinculación** en el teléfono, escribe la dirección,
   el puerto y el código que muestra, luego la dirección y el puerto de «Dirección IP y puerto», y **Conectar**.
5. Elige el teléfono, marca **Media** si quieres también fotos y vídeos, y **Elegir dónde copiar**. oopz copia la
   carpeta de WhatsApp a una carpeta nueva, comprueba el tamaño de cada archivo, no deja nada a medias y te ofrece
   **Usar los archivos copiados**.

![Sin adb todavía: la licencia de Google, antes de descargar nada](images/es/adb-get.png)

![Copiar con adb: los teléfonos que ve adb](images/es/adb-copy.png)

En la línea de comandos: `oopz pull --out-dir CARPETA` (añade `--media`, `--business`, o `--pair HOST:PUERTO --code
CÓDIGO` y `--connect HOST:PUERTO` para la depuración inalámbrica). Usa el adb que descargó oopz si no hay otro.

Sin oopz, la misma copia a mano:

```sh
adb pull /sdcard/Android/media/com.whatsapp/WhatsApp/Databases/ Databases/
```

Cuando acabes, vuelve a desactivar la depuración USB.

### En el propio teléfono: la app de Android

oopz también funciona en Android (7.0 o posterior). Allí, **Elegir un archivo de copia** abre el selector de archivos
del teléfono: ve a `Android/media/com.whatsapp/WhatsApp/Databases/` y elige `msgstore.db.crypt15`. oopz lo copia a su
espacio privado, abre esa copia y la borra al cerrarse. No pide permiso de almacenamiento. Los resultados (una
exportación, una copia recuperada) se guardan donde tú elijas.

<p>
  <img alt="La app de Android: Añadir una copia" src="images/es/phone-add.png" width="200">
  <img alt="La app de Android: la biblioteca" src="images/es/phone-library.png" width="200">
  <img alt="La app de Android: chats de todas las apps" src="images/es/phone-library-chats.png" width="200">
  <img alt="La app de Android: Herramientas" src="images/es/phone-tools.png" width="200">
</p>

### iPhone

oopz lee WhatsApp desde la copia que tu ordenador hace de tu iPhone (sin iCloud y sin jailbreak).

- **Mac**: Finder › tu iPhone › **Hacer una copia de seguridad de todos los datos del iPhone en este Mac** ›
  **Realizar copia ahora**.
- **Windows**: Dispositivos Apple (o iTunes) › **Realizar copia ahora**.

Si marcas **Encriptar copia de seguridad local**, eliges una contraseña, y oopz te la pedirá. Después añade la copia a
la biblioteca (**Biblioteca › Fuentes › Añadir una fuente**) y elige su carpeta:

- Mac: `~/Library/Application Support/MobileSync/Backup/<id>/`
- Windows: `%APPDATA%\Apple Computer\MobileSync\Backup\<id>\` o `%USERPROFILE%\Apple\MobileSync\Backup\<id>\`

### Exportaciones de otras apps

- **WhatsApp** (cualquier teléfono): abre un chat › Más › **Exportar chat**, con o sin archivos. Un chat cada vez.
- **Telegram**: en Telegram Desktop (Windows, macOS, Linux), Ajustes › Avanzado › **Exportar datos de Telegram**, o
  el menú de un chat › **Exportar historial del chat**; formato **JSON legible por máquinas**. Las apps del teléfono
  no exportan.
- **SMS y llamadas**: la app SMS Backup & Restore de Android crea `sms-….xml` y `calls-….xml`.
- **LINE**: un chat › Ajustes › **Exportar historial de chat**.
- **Viber**: Ajustes › Llamadas y mensajes › **Exportar historial de mensajes**.

Pasa el archivo al ordenador de cualquiera de las formas de arriba, o ábrelo en la app de Android.

## Tu clave de 64 dígitos

Una copia crypt15 solo se abre con su clave: 64 cifras y letras (0-9, a-f).

WhatsApp la enseña una sola vez, al activar las copias cifradas: WhatsApp › Ajustes › Chats › Copia de seguridad ›
**Copia de seguridad cifrada de extremo a extremo** › Activar › **Usar clave de cifrado de 64 dígitos**. Apúntala en
ese momento o guárdala en un gestor de contraseñas: WhatsApp no vuelve a mostrarla.

- Si elegiste una **contraseña** en vez de la clave, oopz no puede abrir la copia: la contraseña solo funciona a
  través de los servidores de WhatsApp. Para tener una clave, desactiva la copia cifrada y vuelve a activarla con la
  clave de 64 dígitos, y haz una copia nueva. Eso solo sirve de aquí en adelante.
- Si la clave se ha **perdido**, nada puede abrir las copias hechas con ella, tampoco oopz. Activa una clave nueva
  para las copias que vengan.
- oopz usa la clave una vez para abrir la copia y después la borra de la memoria. Nunca se guarda ni se envía.

## Abrir una copia

La primera vez, oopz te pide que confirmes que las copias son tuyas.

![La pantalla de bienvenida](images/es/welcome.png)

**Añadir copia** y luego **Elegir una carpeta** (la carpeta `Databases` que copiaste) o **Elegir un archivo de
copia**. También puedes arrastrar el archivo a la ventana.

![Añadir una copia](images/es/add-backup.png)

oopz enseña todas las copias de la carpeta. Los formatos antiguos aparecen, pero no se pueden abrir. Elige una.

![Las copias encontradas en una carpeta](images/es/found.png)

Escribe o pega la clave; los espacios dan igual. Marca **Añadirla también a mi biblioteca** para leerla junto con tus
otras apps.

![La clave](images/es/unlock.png)

Mientras la abre, oopz comprueba la copia: si se descifra y si su base de datos está sana. El resumen muestra lo que
contiene.

![El resumen de una copia](images/es/overview.png)

**Cerrar** la olvida y borra la copia descifrada. **Diagnóstico** enseña las tablas y comprobaciones de una copia,
solo con recuentos, para un informe de errores.

![Diagnóstico](images/es/diagnostics.png)

## Leer, buscar y estadísticas

**Chats** muestra todos los chats; abre uno para leerlo, con respuestas, reacciones, encuestas, llamadas y
ediciones. **Ir a una fecha** te mueve por los chats largos. Para ver las fotos, elige la carpeta `Media` que copiaste
del teléfono (**Ver fotos**); no se copia nada.

![Leyendo un chat](images/es/chats.png)

**Buscar** (o Ctrl K, ⌘K en un Mac) encuentra palabras en todos los chats. Dan igual las tildes y las mayúsculas:
`cafe` encuentra `café`. Puedes acotar por chat, tipo, quién lo envió o fechas.

![Buscar](images/es/search.png)

**Estadísticas** muestra los mensajes por mes, día de la semana y hora, y quién escribe más.

![Estadísticas](images/es/insights.png)

oopz sigue el modo oscuro del sistema, o eliges tú uno en Ajustes.

![Leyendo un chat con el tema oscuro](images/es/chats-dark.png)

## La biblioteca: todas tus apps juntas

La biblioteca guarda lo que añades de cada app: WhatsApp (Android e iPhone), Telegram, SMS, LINE y Viber. Puedes
buscar en todo a la vez, leer una línea de tiempo con todas las apps y unir a la misma persona de distintas apps.

**Biblioteca › Fuentes › Añadir una fuente** y elige un archivo o una carpeta; oopz reconoce la app. Un **Exportar
chat** de WhatsApp no dice cuál de las personas eres tú: oopz te lo pregunta una vez (**¿Cuál eres tú?**) para que tus
mensajes salgan como enviados.

![Las fuentes de la biblioteca](images/es/library.png)

![Añadir una fuente](images/es/library-add.png)

- **Chats**: todas las conversaciones de todas las apps, con filtro por app.
- **Buscar**: en todo.
- **Línea de tiempo**: los mensajes de un filtro de todas las apps, por orden de fecha.
- **Estadísticas**: los mismos gráficos que para una copia, sobre todo.
- **Contactos**: la misma persona en distintas apps, unida en un solo contacto. oopz propone uniones cuando coinciden
  números o correos, y nunca une nada por su cuenta. Puedes importar y exportar vCards.
- **Seleccionar**: lo que vas a exportar. Un filtro, más los mensajes que marques o desmarques a mano. Puedes
  guardarlo con un nombre.

![Chats de todas las apps](images/es/library-chats.png)

![La línea de tiempo](images/es/timeline.png)

![Contactos](images/es/contacts.png)

![Seleccionar](images/es/select.png)

La biblioteca se borra al cerrar oopz, salvo que actives **Guardar mi biblioteca en este dispositivo** en Ajustes.

## Exportar

Desde **Seleccionar** (o el **Exportar…** de una copia), elige un formato:

| Formato | Qué obtienes |
|---|---|
| Página web (HTML) | un solo archivo, se abre en cualquier navegador y se imprime bien |
| PDF | páginas para imprimir o guardar, con emoji y texto de derecha a izquierda; portada, índice y fotos pequeñas opcionales |
| CSV | una fila por mensaje, para hojas de cálculo |
| JSON | todo, para otros programas |
| Texto | texto sin formato |
| vCard | las personas que aparecen, como contactos |
| Archivos multimedia | las fotos, audios y archivos, en un zip |

![Exportar una selección](images/es/export-library.png)

![Exportar una copia](images/es/export-backup.png)

Cada exportación es un archivo nuevo; oopz nunca escribe encima de otro. **No va cifrada**: guárdala en un sitio
seguro.

## Recuperar un chat perdido

Dos personas comparten un chat individual. Una lo perdió (entero o en parte); la otra todavía lo tiene. oopz toma los
mensajes de la copia que los tiene y los añade a una **copia nueva** de la copia de la otra persona, puestos de su
lado.

1. Abre las dos copias (cada una con la clave de su dueño).
2. **Recuperar un chat**: elige la copia que todavía tiene el chat y la que lo perdió, y el chat en cada lado.
   **Comparar: ¿qué falta?** cuenta lo que se añadiría.
3. Escribe la clave de la copia que lo perdió, elige una carpeta para el resultado y, si quieres las fotos, la carpeta
   `Media` del otro teléfono.
4. **Unir en una copia nueva**. Antes de escribir nada, oopz vuelve a descifrar el resultado y lo comprueba
   (integridad de la base de datos, recuentos, orden, que ningún otro chat cambie). Si falla una comprobación, no se
   escribe nada.
5. **Ponerla en el teléfono** te guía: aparta la carpeta de WhatsApp, borra los datos de WhatsApp, deja el resultado
   en `Databases/` y restaura desde él en WhatsApp.

![Recuperar un chat](images/es/restore.png)

oopz se niega, y te dice por qué, cuando:

- el chat es un **grupo** (de momento, solo chats individuales);
- los dos lados son **la misma copia**, o el chat no existe en la copia de destino;
- los mensajes que tienen las dos copias no aparecen **como en un espejo** (es otro chat, o la copia de otra persona);
- el WhatsApp de destino tiene **tablas o columnas que oopz no puede rellenar** con la otra copia (un WhatsApp más
  nuevo): te dice cuáles;
- la clave de destino no abre su copia, o la carpeta del resultado ya existe.

Los originales nunca se tocan. Cambiar una copia de WhatsApp puede ir contra sus condiciones: lo decide, y asume el
riesgo, la persona dueña de la cuenta. Guarda la copia original y la carpeta apartada hasta estar seguro.

## Ajustes, privacidad y actualizaciones

![Ajustes](images/es/settings.png)

- **Idioma** y **tema**; **Ocultar nombres** muestra los chats por su tipo y el final de su número, para cuando
  compartes pantalla.
- **Privacidad**: **Guardar mi biblioteca en este dispositivo**, y **Borrar ahora**, que elimina de una vez todas las
  copias descifradas y la biblioteca.
- oopz no tiene cuenta, ni nube, ni telemetría. Las copias, las claves y los mensajes se quedan en tu dispositivo. Lo
  único que usa la red es la página Wi-Fi para tu teléfono (mientras está abierta) y la búsqueda de actualizaciones.
- Las **actualizaciones** están desactivadas hasta que actives **Buscar actualizaciones al abrir oopz** o pulses
  **Buscar actualizaciones**. Se le pregunta a GitHub por la última versión; GitHub ve tu dirección IP y nada más. Una
  actualización solo se instala cuando tú lo pides, y solo si su firma es correcta. En Android, instala el APK nuevo
  desde la [página de versiones](https://github.com/Sparklitt/oopz/releases).

![Ajustes: privacidad](images/es/settings-privacy.png)

![Ajustes: actualizaciones](images/es/settings-updates.png)

## La línea de comandos

El comando `oopz` hace lo mismo que la app, en una terminal; muestra recuentos, fechas e ids enmascarados, nunca el
texto de los mensajes. Los instaladores lo dejan en tu `PATH`; con la AppImage o en macOS, usa **Ajustes › Instalar
el comando oopz**.

![Ajustes › General, con la fila del comando oopz (AppImage)](images/es/settings.png)

```sh
oopz backups --dir Databases/          # ¿se abren todas y están sanas?
oopz chats --backup Databases/msgstore.db.crypt15
oopz lib import result.json --library mi-biblioteca/
```

`oopz --help` enseña todos los comandos; el [README](../../README.md#usage) tiene la tabla (en inglés).

## Si algo falla

| Lo que ves | Qué hacer |
|---|---|
| «Esta clave no abre esta copia» | Revisa cada carácter. Si está bien, puede que el archivo esté cortado: cópialo otra vez y compara su tamaño con el del teléfono. |
| «copia crypt14 antigua» | Su archivo de clave solo se puede leer en un teléfono rooteado. Activa la copia cifrada de extremo a extremo con la clave de 64 dígitos y haz una copia nueva. |
| «no es una copia crypt15 de WhatsApp, o está incompleto» | La copia se cortó, o es otro archivo. Vuelve a copiarlo. |
| «su base de datos está dañada» | Prueba una copia anterior, `msgstore-AAAA-MM-DD.1.db.crypt15`. |
| Salen números en vez de nombres | Copia también `wa.db.crypt15` de la misma carpeta, si tu teléfono lo tiene. |
| No hay ninguna copia en la carpeta | Puede que WhatsApp solo guarde la copia en Google Drive. Haz una local: Ajustes › Chats › Copia de seguridad › Guardar. |
| El teléfono no abre la página Wi-Fi | Mira [los consejos de Wi-Fi](#por-wi-fi-con-un-código-qr), o usa un cable o adb. |
| «No hay espacio libre suficiente» | Libera espacio o elige otra carpeta. No se ha escrito nada. |
| Windows dice «Windows protegió su PC» | El instalador aún no está firmado: **Más información › Ejecutar de todas formas**. |
| macOS dice que no puede comprobar oopz | Ajustes del Sistema › Privacidad y seguridad › **Abrir igualmente**. |

¿Otra cosa? [Abre una incidencia](https://github.com/Sparklitt/oopz/issues) contando qué hiciste y qué dijo oopz.
Nunca adjuntes una copia, una clave, mensajes ni números de teléfono.

<sub>Los emoji de las imágenes son de [Twemoji](https://github.com/twitter/twemoji) (CC-BY 4.0).</sub>
