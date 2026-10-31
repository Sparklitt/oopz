#!/usr/bin/env python3
"""Writes the README artwork: banner-light.svg, banner-dark.svg, banner.svg (follows the viewer's theme) and demo.svg.

    python3 assets/make_assets.py
    pnpm dlx @resvg/resvg-js-cli --fit-width 1280 assets/banner-light.svg assets/social-preview.png

The demo replays a real session on the synthetic fixtures (the `fixtures` feature of the `oops` crate: invented
people, test keys); its output is copied verbatim from that run, trimmed where marked with "…". Never paste output
from a real backup here.
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent

# Not WhatsApp's colours: a warm coral and amber on ink or paper.
THEMES = {
    "light": dict(bg="#FFF7EE", bg2="#FFE9D6", ink="#23244F", soft="#6B6C8F", card="#FFFFFF", line="#E4D9CC",
                  coral="#F2604F", amber="#F7A928", bar="#D9D2E6", chip="#23244F", chiptext="#FFF7EE"),
    "dark": dict(bg="#15162E", bg2="#1F2042", ink="#F6F0E7", soft="#A9AACB", card="#26284F", line="#34366A",
                 coral="#FF7565", amber="#FFB940", bar="#4A4C80", chip="#F6F0E7", chiptext="#15162E"),
}

SANS = "Inter, 'Segoe UI', 'Helvetica Neue', Helvetica, Arial, 'DejaVu Sans', sans-serif"


def banner_body() -> str:
    """The artwork, coloured through CSS classes so one drawing serves both themes."""
    return f"""
  <rect class="bg" width="1280" height="640"/>
  <circle class="bg2" cx="1040" cy="300" r="250"/>
  <circle class="bg2" cx="150" cy="590" r="120"/>

  <!-- wordmark: drawn with strokes, so it looks the same without any font -->
  <g class="mark" fill="none" stroke-width="30" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="186" cy="262" r="54"/>
    <circle cx="326" cy="262" r="54"/>
    <circle cx="466" cy="262" r="54"/>
    <path d="M412 262 V392"/>
    <path d="M638 222 C626 204 600 198 580 202 C556 207 546 226 556 243 C566 260 602 262 622 274
             C644 288 642 312 622 322 C600 333 566 330 550 308"/>
  </g>
  <circle class="coral" cx="696" cy="304" r="17"/>

  <text class="ink" x="132" y="470" font-family="{SANS}" font-size="38" font-weight="700">Your own chat backups —</text>
  <text class="ink" x="132" y="518" font-family="{SANS}" font-size="38" font-weight="700">open, check, recover. <tspan class="coraltext">Offline.</tspan></text>

  <g font-family="{SANS}" font-size="19" font-weight="600">
    <rect class="chip" x="132" y="552" width="88" height="36" rx="18"/>
    <text class="chiptext" x="176" y="576" text-anchor="middle">Rust</text>
    <rect class="chip" x="232" y="552" width="132" height="36" rx="18"/>
    <text class="chiptext" x="298" y="576" text-anchor="middle">AGPL-3.0</text>
    <rect class="chip" x="376" y="552" width="182" height="36" rx="18"/>
    <text class="chiptext" x="467" y="576" text-anchor="middle">no network, ever</text>
  </g>

  <!-- motif: a chat whose lost message comes back -->
  <g>
    <rect class="card" x="830" y="150" width="270" height="70" rx="26"/>
    <rect class="bar" x="858" y="172" width="170" height="10" rx="5"/>
    <rect class="bar" x="858" y="192" width="110" height="10" rx="5"/>

    <rect class="ghost" x="900" y="250" width="290" height="74" rx="26" fill="none" stroke-width="3" stroke-dasharray="10 9"/>
    <rect class="coral" x="880" y="262" width="290" height="74" rx="26"/>
    <rect fill="#FFFFFF" opacity=".9" x="910" y="286" width="190" height="10" rx="5"/>
    <rect fill="#FFFFFF" opacity=".7" x="910" y="306" width="120" height="10" rx="5"/>

    <rect class="card" x="830" y="370" width="230" height="70" rx="26"/>
    <rect class="bar" x="858" y="392" width="140" height="10" rx="5"/>
    <rect class="bar" x="858" y="412" width="90" height="10" rx="5"/>

    <circle class="amber" cx="1170" cy="262" r="34"/>
    <g fill="none" stroke="#FFFFFF" stroke-width="7" stroke-linecap="round" stroke-linejoin="round">
      <path d="M1157 256 H1176 C1186 256 1192 263 1192 271 C1192 279 1186 285 1176 285 H1164"/>
      <path d="M1166 246 L1156 256 L1166 266"/>
    </g>
    <g class="amberstroke" fill="none" stroke-width="5" stroke-linecap="round">
      <path d="M1222 218 L1234 206"/><path d="M1228 240 H1246"/><path d="M1212 202 V186"/>
    </g>
  </g>
"""


def css(t: dict) -> str:
    return (f".bg{{fill:{t['bg']}}}.bg2{{fill:{t['bg2']}}}.mark{{stroke:{t['ink']}}}.ink{{fill:{t['ink']}}}"
            f".coral{{fill:{t['coral']}}}.coraltext{{fill:{t['coral']}}}.amber{{fill:{t['amber']}}}"
            f".amberstroke{{stroke:{t['amber']}}}.card{{fill:{t['card']};stroke:{t['line']};stroke-width:2}}"
            f".bar{{fill:{t['bar']}}}.ghost{{stroke:{t['soft']}}}.chip{{fill:{t['chip']}}}"
            f".chiptext{{fill:{t['chiptext']}}}")


def banner(style: str) -> str:
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640" '
            f'role="img" aria-label="oops: your own chat backups, open, check, recover. Offline.">\n'
            f"  <style>{style}</style>{banner_body()}</svg>\n")


# --- terminal demo -------------------------------------------------------------------------------------------------

K = "64-digit {} key (not shown): "
# (command lines, output lines) per screen; output copied from a run on the synthetic fixtures.
SCREENS = [
    (["oops backups --dir Databases/"],
     [("dim", K.format("backup")),
      ("ok", "msgstore.db.crypt15  0 MB  ok: 2 chats, 19 messages, last 2023-11-14")]),
    (["oops chats --backup Databases/msgstore.db.crypt15"],
     [("dim", K.format("backup")),
      ("head", " chat  who                   messages  last message"),
      ("", "    1  contact ••••001             18  2023-11-14"),
      ("", "    2  group ••••001                1  2023-11-14")]),
    (["oops restore --source ana.crypt15 --source-chat 1 \\",
      "    --target ben.crypt15 --target-chat 2 --out ben-restored.crypt15"],
     [("dim", K.format("source")),
      ("dim", K.format("target")),
      ("", "target chat: 4 messages before"),
      ("", "source chat: 2 already there, 8 added"),
      ("", "media: 2 new paths, 1 reuse a file the target has, 0 unsafe paths dropped; calls reused: 1"),
      ("dim", "  copied message: 8"),
      ("dim", "  …"),
      ("ok", "check decrypts with the target's key: yes"),
      ("ok", "check integrity_check: ok"),
      ("ok", "check chat messages: 4 + 8 = 12"),
      ("ok", "check other chats and the chat's own messages: identical, in order"),
      ("dim", "  …"),
      ("accent", "written: ben-restored.crypt15 (5331 bytes)"),
      ("", "next: oops chat-facts --backup ben-restored.crypt15 --chat 2")]),
]

FS, LH, CW = 15, 22, 9.0  # font size, line height, forced character width (textLength) for commands
W, H, PAD, TOP = 960, 470, 24, 64
TYPE_CPS, PAUSE, HOLD = 28, 0.5, 3.2


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def demo() -> str:
    events = []  # (kind, start, end of typing or None, screen end set later, payload)
    t = 0.6
    screens = []
    for cmds, outs in SCREENS:
        items = []
        y = TOP
        for i, c in enumerate(cmds):
            prompt = "$ " if i == 0 else "> "
            dur = len(c) / TYPE_CPS
            items.append(("cmd", t, t + dur, y, prompt, c))
            t += dur + 0.15
            y += LH
        t += PAUSE
        for cls, line in outs:
            items.append(("out", t, None, y, cls, line))
            t += 0.55 if cls == "dim" and "key" in line else 0.12
            y += LH
        t += HOLD
        screens.append((items, t))
        t += 0.1
    total = t + 0.4

    def pct(x: float) -> str:
        return f"{100 * x / total:.3f}%"

    style = [
        f"text{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,'DejaVu Sans Mono','Liberation Mono',monospace;"
        f"font-size:{FS}px;white-space:pre}}",
        ".p{fill:#FF7565}.c{fill:#F6F0E7}.o{fill:#D9D6EA}.dim{fill:#8587B0}.ok{fill:#8FD9A8}.head{fill:#FFB940}"
        ".accent{fill:#FFB940;font-weight:700}.cur{fill:#FF7565}",
    ]
    body = []
    n = 0
    for items, end in screens:
        for kind, start, typed, y, a, b in items:
            n += 1
            name = f"k{n}"
            # Visible from start to the end of its screen.
            style.append(f"@keyframes {name}{{0%,{pct(start)}{{opacity:0}}{pct(start + 0.001)},{pct(end)}{{opacity:1}}"
                         f"{pct(end + 0.001)},100%{{opacity:0}}}}.{name}{{animation:{name} {total:.2f}s linear infinite}}")
            if kind == "out":
                body.append(f'<text class="{a or "o"} {name}" x="{PAD}" y="{y}">{esc(b)}</text>')
                continue
            px = PAD + 2 * CW
            width = len(b) * CW
            body.append(f'<g class="{name}"><text class="p" x="{PAD}" y="{y}">{a.strip()}</text>'
                        f'<text class="c" x="{px}" y="{y}" textLength="{width:.1f}" lengthAdjust="spacing">'
                        f"{esc(b)}</text>")
            # A cover the colour of the terminal slides right one character at a time, with the cursor on its edge.
            cover = f"v{n}"
            steps = len(b)
            style.append(
                f"@keyframes {cover}{{0%,{pct(start)}{{transform:translateX(0);animation-timing-function:steps({steps},end)}}"
                f"{pct(typed)},100%{{transform:translateX({width:.1f}px)}}}}"
                f".{cover}{{animation:{cover} {total:.2f}s linear infinite}}")
            blink = f"b{n}"
            style.append(f"@keyframes {blink}{{0%,{pct(typed + 0.2)}{{opacity:1}}{pct(typed + 0.21)},100%{{opacity:0}}}}"
                         f".{blink}{{animation:{blink} {total:.2f}s linear infinite}}")
            body.append(f'<g class="{cover}"><rect x="{px}" y="{y - FS}" width="{W}" height="{LH}" fill="#15162E"/>'
                        f'<rect class="cur {blink}" x="{px}" y="{y - FS + 1}" width="{CW:.0f}" height="{FS + 3}"/></g></g>')

    return f"""<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="A terminal session: oops checks a folder of backups, lists the chats in one (masked ids), and restores a lost chat into a new, checked backup. Synthetic data.">
<style>{"".join(style)}</style>
<rect width="{W}" height="{H}" rx="14" fill="#15162E"/>
<circle cx="26" cy="22" r="6" fill="#FF7565"/><circle cx="46" cy="22" r="6" fill="#FFB940"/><circle cx="66" cy="22" r="6" fill="#8FD9A8"/>
<text x="{W / 2}" y="27" text-anchor="middle" class="dim" style="font-size:13px">oops — synthetic backups, test keys</text>
{chr(10).join(body)}
<rect x="0.5" y="0.5" width="{W - 1}" height="{H - 1}" rx="14" fill="none" stroke="#34366A"/>
</svg>
"""


def main() -> None:
    light, dark = css(THEMES["light"]), css(THEMES["dark"])
    (HERE / "banner-light.svg").write_text(banner(light))
    (HERE / "banner-dark.svg").write_text(banner(dark))
    (HERE / "banner.svg").write_text(banner(f"{light}@media (prefers-color-scheme: dark){{{dark}}}"))
    (HERE / "demo.svg").write_text(demo())


if __name__ == "__main__":
    main()
