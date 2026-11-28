#!/usr/bin/env python3
"""Builds the list of classic player-made tracks (made 2000 to 2005).

    python3 tools/classics/build.py tools/classics/classics.json

Lists every track MDX Racing hosts, then reads just the end of each zip
(its table of contents, a few kilobytes, by HTTP range request) to see
when its .env track file was made. Tracks whose .env dates from 2000 to
2005 are the classics: the ones players made while MM2 was new. Copies
of the same file uploaded twice are listed once.

The launcher's Tracks page reads the list (built in), and downloads a
track from MDX Racing only when a player picks it. MDX Racing gave its
permission for that; each track still belongs to the player who made it.
"""

import concurrent.futures
import html
import json
import re
import struct
import sys
import time
import urllib.parse
import urllib.request

AGENT = "OpenMCM2-classics/1"
MDX = "https://mdxracing.com/"
CATEGORIES = {
    "National_Tracks": "NATIONAL",
    "SuperCross_Tracks": "SX",
    "BAJA_Maps": "BAJA",
    "ENDURO": "ENDURO",
    "Stunt_Maps": "QUARRIES",
}
YEARS = range(2000, 2006)
# The game's own tracks, uploaded again by players: not classics.
STOCK = re.compile(r"^(nat|sx|baja|enduro|end|quarry|qua|stunt)\d*\.env$|^(airfield|farm|openpit|skilodge)\.env$", re.I)
# The game's own track files are dated 29 and 30 April 2000: a track file
# with those dates is one of them renamed.
STOCK_DAYS = ("2000-04-29", "2000-04-30")

# Mike McCue's Harmonic Cycle tracks (2000 to 2003), by .env name, with
# the names his site gives them; his readmes name him as their maker.
HARMONIC = "Mike McCue (Harmonic Cycle)"
HARMONIC_NAMES = {
    "apalachee.env": "Apalachee Sunshine",
    "chattahoocheenat.env": "Chattahoochee Bluff National Challenge",
    "kolomokicsnat.env": "Kolomoki Carousel",
    "kolomokimxnat.env": "Kolomoki Icky Klay",
    "kolomokinat.env": "Kolomoki Quick",
    "micousoukee.env": "Micousoukee Maze",
    "sansilica.env": "Mission San Silica",
    "hcd03.env": "Starwatch Staircase Ramp Folly",
    "rocky03.env": "Starwatch Staircase (rocky)",
    "starwatchbeta.env": "Starwatch Staircase (smooth)",
    "suwannee_sand_dune_wiggle.env": "Suwannee Sand Dune Wiggle",
    "td_sko_blindmule.env": "TD Sketch Out: Blind Mule Pass",
    "td_sko_brycecanyon.env": "TD Sketch Out: Bryce Canyon",
    "td_farmyardscramble.env": "TD Farmyard Scramble",
    "td_foulpheasant.env": "TD Foul Pheasant Twist",
    "td_raunchyturkey.env": "TD Raunchy Turkey Twist",
    "td_q60_karnivaltwist.env": "TD Quick 60: Karnival Twist",
    "td_q60_alpinatwist.env": "TD Quick 60: Alpina Twist",
    "td_q60_shadowtwist.env": "TD Quick 60: Shadow Twist",
    "tdbackyardstwist.env": "TD Backyards Twist",
    "tdmutantnewton01.env": "TD Mutant Newton Twist",
}
# The sizes of the zips on his own site: MDX's copy of the same size is
# the one listed when MDX has several uploads of a track.
HARMONIC_SIZES = {5249846, 6165053, 6975867, 7169501, 7534376, 7366684, 7355142, 8765590, 8199305, 8141044, 7309903, 7982127}
MAKERS = {HARMONIC: list(HARMONIC_NAMES)}
MAKER = {env: who for who, envs in MAKERS.items() for env in envs}


def get(url, headers=None, tries=5):
    for i in range(tries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": AGENT, **(headers or {})})
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read(), r.headers
        except urllib.error.HTTPError as e:
            if e.code in (400, 404):
                raise
            last = e
        except Exception as e:  # the site drops connections now and then
            last = e
        time.sleep(2 + 3 * i)
    raise RuntimeError(f"{url}: {last}")


def listing():
    """Every track MDX Racing lists: name, maker, event and address."""
    tracks, seen = [], set()
    for cat, event in CATEGORIES.items():
        page = 1
        while True:
            q = f"q={cat}" if page == 1 else f"page={page}&c=&sc=&q={cat}"
            text = get(f"{MDX}new/index3.php?{q}")[0].decode("latin-1")
            found, start = 0, 0
            for m in re.finditer(r"href='\.\./(mcm2/Tracks/[^']+)'", text):
                rel = m.group(1)
                before, start = text[start:m.start()], m.end()
                if not re.search(r"\.(zip|rar|7z)$", rel, re.I) or rel.split("/")[2].lower() != cat.lower() or rel in seen:
                    continue
                seen.add(rel)
                found += 1
                b = before.rfind("<b>")
                title = html.unescape(re.sub("<[^>]+>", "", before[b + 3:before.find("</b>", b)])) if b >= 0 else ""
                k = title.lower().rfind(" track by")
                name, author = (title[:k].strip(), title[k + 9:].strip()) if k >= 0 else (title.strip(), "")
                stem = rel.rsplit("/", 1)[1].rsplit(".", 1)[0]
                tracks.append({"name": name or stem, "author": author, "event": event, "path": rel})
            print(cat, page, found, file=sys.stderr, flush=True)
            if found == 0:
                break
            page += 1
    return tracks


def contents(t):
    """Adds the zip's size, .env names and the .env's date (from the zip's
    table of contents, read with a range request)."""
    if not t["path"].lower().endswith(".zip"):
        return t
    url = MDX + urllib.parse.quote(t["path"])
    try:
        tail, h = get(url, {"Range": "bytes=-65536"})
        size = int(h.get("Content-Range", "/0").rsplit("/", 1)[1] or len(tail))
        e = tail.rfind(b"PK\x05\x06")
        if e < 0:
            return t
        _, cd_size, cd_at = struct.unpack("<HII", tail[e + 10:e + 20])
        first = size - len(tail)
        cd = get(url, {"Range": f"bytes={cd_at}-{cd_at + cd_size - 1}"})[0] if cd_at < first else tail[cd_at - first:cd_at - first + cd_size]
        envs, p = [], 0
        while p + 46 <= len(cd) and cd[p:p + 4] == b"PK\x01\x02":
            tm, dt = struct.unpack("<HH", cd[p + 12:p + 16])
            n, x, c = struct.unpack("<HHH", cd[p + 28:p + 34])
            name = cd[p + 46:p + 46 + n].decode("latin-1")
            if name.lower().endswith(".env"):
                envs.append((name.replace("\\", "/").rsplit("/", 1)[-1], f"{1980 + (dt >> 9):04d}-{(dt >> 5) & 15:02d}-{dt & 31:02d}"))
            p += 46 + n + x + c
        if envs:
            t.update(size=size, envs=[e[0] for e in envs], made=min(e[1] for e in envs))
    except Exception as ex:
        print("skipped", t["path"], ex, file=sys.stderr)
    return t


def main(out):
    tracks = listing()
    with concurrent.futures.ThreadPoolExecutor(6) as ex:
        tracks = list(ex.map(contents, tracks))
    classics, seen = [], set()
    for t in tracks:
        if not t.get("made") or int(t["made"][:4]) not in YEARS or t["made"] in STOCK_DAYS or all(STOCK.match(e) for e in t["envs"]):
            continue
        key = (t["event"], tuple(sorted(e.lower() for e in t["envs"])), t["size"])
        if key in seen:
            continue
        seen.add(key)
        env = next((e.lower() for e in t["envs"] if e.lower() in MAKER), None)
        name = HARMONIC_NAMES.get(env, t["name"]) if env else t["name"]
        classics.append({"name": name, "author": MAKER[env] if env else t["author"], "event": t["event"], "path": t["path"], "made": t["made"], "size": t["size"], "env": env})
    # One copy of each Harmonic Cycle track: the one matching his own zip.
    best = {}
    for t in classics:
        if t["env"]:
            b = best.get(t["env"])
            if b is None or (t["size"] in HARMONIC_SIZES and b["size"] not in HARMONIC_SIZES):
                best[t["env"]] = t
    classics = [{k: v for k, v in t.items() if k != "env"} for t in classics if not t["env"] or best[t["env"]] is t]
    classics.sort(key=lambda t: (t["made"], t["name"].lower()))
    about = (
        "Classic player-made tracks: every track on MDX Racing whose track file was made from 2000 to 2005, "
        "dated from the zip's own table of contents. Each belongs to the player who made it; the launcher "
        "downloads it from MDX Racing (with its permission) only when a player picks it. Build with build.py."
    )
    with open(out, "w") as f:
        f.write('{"about": ' + json.dumps(about) + ',\n"tracks": [\n')
        f.write(",\n".join(json.dumps(t, ensure_ascii=False, separators=(",", ":")) for t in classics))
        f.write("\n]}\n")
    print(len(classics), "classics", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1])
