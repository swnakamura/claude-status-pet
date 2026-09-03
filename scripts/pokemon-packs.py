#!/usr/bin/env python3
"""Build claude-status-pet character packs from Pokémon Showdown sprites.

Personal use only: the sprites are © Nintendo / Game Freak / Creatures and are
downloaded straight from play.pokemonshowdown.com at run time, nothing is
redistributed. Each pack keeps its sprite still ("motion": false) and uses the
first frame of the front animation as the idle image, so idle and editing
show the same drawing at the same size.

    python3 scripts/pokemon-packs.py [--scale 0.6] leafeon glaceon sylveon eevee
    # -> ~/.claude/pet-data/characters/<name>/{character.json,idle.png,front.gif,back.gif,shiny.gif}

The argument position becomes the pack's "order", which is the order of the
character menu and of the per-session rotation (first pet = first name, once
that pack is the saved default).

"--scale" sets the pack's "scale" (fraction of the 140px box the sprite is drawn in);
the ~60px Showdown sprites look less blurry at 0.5–0.7 than stretched to the full box.

Requires Pillow (for the first-frame extraction).
"""
from __future__ import annotations

import io
import json
import sys
import urllib.request
from pathlib import Path

from PIL import Image

BASE = "https://play.pokemonshowdown.com/sprites"
CHARACTERS = Path.home() / ".claude" / "pet-data" / "characters"


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "claude-status-pet pokemon-packs"})
    with urllib.request.urlopen(req) as r:
        return r.read()


def build(name: str, order: int, scale: float) -> Path:
    d = CHARACTERS / name
    d.mkdir(parents=True, exist_ok=True)
    front = fetch(f"{BASE}/ani/{name}.gif")
    (d / "front.gif").write_bytes(front)
    (d / "back.gif").write_bytes(fetch(f"{BASE}/ani-back/{name}.gif"))
    (d / "shiny.gif").write_bytes(fetch(f"{BASE}/ani-shiny/{name}.gif"))
    g = Image.open(io.BytesIO(front))
    g.seek(0)
    g.convert("RGBA").save(d / "idle.png")
    still, back, fwd, shiny = [f"{name}/idle.png"], [f"{name}/back.gif"], [f"{name}/front.gif"], [f"{name}/shiny.gif"]
    cfg = {
        "name": f"{name.capitalize()} (Pokémon)",
        "type": "gif",
        "version": 1,
        "order": order,
        "motion": False,
        "scale": scale,
        "states": {
            "idle": still, "offline": still, "unknown": still,
            "thinking": back, "reading": back, "searching": back,
            "editing": fwd, "running": fwd, "delegating": fwd,
            "waiting": shiny, "error": shiny,
        },
    }
    (d / "character.json").write_text(json.dumps(cfg, indent=2, ensure_ascii=False) + "\n")
    return d


if __name__ == "__main__":
    args = sys.argv[1:]
    scale = 0.6
    if "--scale" in args:
        i = args.index("--scale")
        scale = float(args[i + 1])
        del args[i:i + 2]
    names = args or ["leafeon", "glaceon", "sylveon", "eevee"]
    for i, n in enumerate(names):
        print("built", build(n, i, scale))
    print("restart the pets (right-click → Exit, then /pet on) to pick the packs up")
