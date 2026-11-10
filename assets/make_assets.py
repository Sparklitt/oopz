#!/usr/bin/env python3
"""Writes the README artwork: banner-light.svg, banner-dark.svg, banner.svg (follows the viewer's theme) and demo.svg.

    python3 assets/make_assets.py
    pnpm dlx @resvg/resvg-js-cli --fit-width 1280 assets/banner-light.svg assets/social-preview.png

The demo replays a real session on the synthetic fixtures (the `fixtures` feature of the `oopz` crate: invented
people, test keys); its output is copied verbatim from that run, trimmed where marked with "…". Never paste output
from a real backup here.
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent

# The app's own colours (apps/desktop/src/app.css): forest green on warm paper, or on near-black.
THEMES = {
    "light": dict(bg="#F6F4F0", bg2="#EBE7E0", ink="#1D201D", soft="#62665F", card="#FFFFFF", line="#E3DFD7",
                  accent="#2F5D4F", accenttext="#2F5D4F", accentsoft="#DBE8E2", bar="#D6D2CA", chip="#2F5D4F", chiptext="#FFFFFF"),
    "dark": dict(bg="#1B1D1C", bg2="#232625", ink="#E9EBE7", soft="#A5AAA3", card="#2A2D2C", line="#333735",
                 accent="#3F7D68", accenttext="#7CBEA5", accentsoft="#24342E", bar="#3D4240", chip="#3A7562", chiptext="#FFFFFF"),
}

SANS = "Inter, 'Segoe UI', 'Helvetica Neue', Helvetica, Arial, 'DejaVu Sans', sans-serif"

# Where a person's history comes from: names only, as text (no app's logo or colours).
SOURCES = ["WhatsApp · Android", "WhatsApp · iPhone", "Telegram", "SMS", "LINE", "Viber"]


def mark(x: float, y: float, size: float) -> str:
    """The app's mark (app-icon.svg): a curled restore arrow on a rounded square."""
    k = size / 64
    return f"""<g transform="translate({x} {y}) scale({k})">
    <rect class="markbg" width="64" height="64" rx="15"/>
    <path d="M17.5 32a14.5 14.5 0 1 0 4.25-10.25" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round"/>
    <path d="M15.5 14.5v10.25h10.25" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round"
          stroke-linejoin="round"/>
    <circle cx="32" cy="32" r="3.6" fill="#fff"/>
  </g>"""


def banner_body() -> str:
    """The artwork, coloured through CSS classes so one drawing serves both themes."""
    chips = []
    y = 118
    for name in SOURCES:
        w = 34 + len(name) * 10.2
        right = 900
        chips.append(f'<rect class="card" x="{right - w:.0f}" y="{y}" width="{w:.0f}" height="44" rx="22"/>'
                     f'<text class="ink" x="{right - w / 2:.0f}" y="{y + 29}" text-anchor="middle" font-size="18" '
                     f'font-weight="600">{name}</text>'
                     f'<path class="flow" d="M{right + 6} {y + 22} C 960 {y + 22}, 950 320, 1010 320" '
                     f'fill="none" stroke-width="2.5" stroke-dasharray="2 7" stroke-linecap="round"/>')
        y += 66
    return f"""
  <rect class="bg" width="1280" height="640"/>
  <circle class="bg2" cx="1130" cy="320" r="230"/>

  {mark(132, 150, 112)}
  <text class="ink" x="268" y="246" font-family="{SANS}" font-size="104" font-weight="800"
        letter-spacing="-3">oopz</text>

  <text class="ink" x="132" y="356" font-family="{SANS}" font-size="36" font-weight="700">Your own chat history,</text>
  <text class="ink" x="132" y="402" font-family="{SANS}" font-size="36" font-weight="700">from every app, in one
    place.</text>
  <text class="soft" x="132" y="452" font-family="{SANS}" font-size="24">Search, select, export, restore a lost chat.
    <tspan class="accenttext" font-weight="700">Offline.</tspan></text>

  <g font-family="{SANS}" font-size="19" font-weight="600">
    <rect class="chip" x="132" y="510" width="88" height="36" rx="18"/>
    <text class="chiptext" x="176" y="534" text-anchor="middle">Rust</text>
    <rect class="chip" x="232" y="510" width="132" height="36" rx="18"/>
    <text class="chiptext" x="298" y="534" text-anchor="middle">AGPL-3.0</text>
    <rect class="chip" x="376" y="510" width="182" height="36" rx="18"/>
    <text class="chiptext" x="467" y="534" text-anchor="middle">no network, ever</text>
    <rect class="chip" x="570" y="510" width="214" height="36" rx="18"/>
    <text class="chiptext" x="677" y="534" text-anchor="middle">desktop · Android · CLI</text>
  </g>

  <!-- motif: each app's export or backup flows into one library -->
  <g font-family="{SANS}">
    {"".join(chips)}
  </g>
  <g>
    <rect class="card" x="1010" y="230" width="190" height="180" rx="30"/>
    <rect class="accentsoft" x="1032" y="254" width="146" height="34" rx="17"/>
    <rect class="bar" x="1032" y="304" width="120" height="10" rx="5"/>
    <rect class="bar" x="1032" y="324" width="90" height="10" rx="5"/>
    <rect class="bar" x="1032" y="352" width="130" height="10" rx="5"/>
    <rect class="bar" x="1032" y="372" width="70" height="10" rx="5"/>
    <g fill="none" class="accentstroke" stroke-width="5" stroke-linecap="round">
      <circle cx="1052" cy="271" r="8"/><path d="M1058 277 L1065 284"/>
    </g>
  </g>
"""


def css(t: dict) -> str:
    return (f".bg{{fill:{t['bg']}}}.bg2{{fill:{t['bg2']}}}.ink{{fill:{t['ink']}}}.soft{{fill:{t['soft']}}}"
            f".markbg{{fill:{t['accent']}}}.accenttext{{fill:{t['accenttext']}}}.accentsoft{{fill:{t['accentsoft']}}}"
            f".accentstroke{{stroke:{t['accent']}}}.flow{{stroke:{t['accenttext']}}}"
            f".card{{fill:{t['card']};stroke:{t['line']};stroke-width:2}}"
            f".bar{{fill:{t['bar']}}}.chip{{fill:{t['chip']}}}.chiptext{{fill:{t['chiptext']}}}")


def banner(style: str) -> str:
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640" '
            f'role="img" aria-label="oopz: your own chat history, from every app, in one place. Search, select, '
            f'export, restore a lost chat. Offline.">\n'
            f"  <style>{style}</style>{banner_body()}</svg>\n")


# --- terminal demo -------------------------------------------------------------------------------------------------

K = "64-digit {} key (not shown): "
# (command lines, output lines) per screen; output copied from a run on the synthetic fixtures.
SCREENS = [
    (["oopz backups --dir Databases/"],
     [("dim", K.format("backup")),
      ("ok", "msgstore.db.crypt15  0 MB  ok: 2 chats, 19 messages, last 2023-11-14")]),
    (["oopz chats --backup Databases/msgstore.db.crypt15"],
     [("dim", K.format("backup")),
      ("head", " chat  who                   messages  last message"),
      ("", "    1  contact ••••001             18  2023-11-14"),
      ("", "    2  group ••••001                1  2023-11-14")]),
    (["oopz restore --source ana.crypt15 --source-chat 1 \\",
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
      ("", "next: oopz chat-facts --backup ben-restored.crypt15 --chat 2")]),
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

    return f"""<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="A terminal session: oopz checks a folder of backups, lists the chats in one (masked ids), and restores a lost chat into a new, checked backup. Synthetic data.">
<style>{"".join(style)}</style>
<rect width="{W}" height="{H}" rx="14" fill="#15162E"/>
<circle cx="26" cy="22" r="6" fill="#FF7565"/><circle cx="46" cy="22" r="6" fill="#FFB940"/><circle cx="66" cy="22" r="6" fill="#8FD9A8"/>
<text x="{W / 2}" y="27" text-anchor="middle" class="dim" style="font-size:13px">oopz — synthetic backups, test keys</text>
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
