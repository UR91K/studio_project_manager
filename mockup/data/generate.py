"""Seed a mock Seula database and snapshot the real HTTP API over it.

Two steps, both deterministic (fixed seed):

1. ``seed``: build ``seula-mock.db`` from ``src/database/schema.sql`` -- the same schema
   the program compiles in -- and fill it with plausible fake data written exactly the
   way the Rust code writes it (epoch-second datetimes, lowercase uid hex, PluginFormat
   display strings, Tonic/Scale FromStr names).

2. ``snapshot``: start the real ``seula --server`` against that database with a
   throwaway config, GET the read endpoints the frontend uses, and write the responses
   to ``mock-api.json`` plus ``mock-api.js`` (the same data as a global, so the mockups
   work from file:// without a server).

The JSON therefore has the exact shapes the real DTOs produce; nothing here restates
them. Run from anywhere:

    python mockup/data/generate.py            # seed + snapshot
    python mockup/data/generate.py seed       # database only
    python mockup/data/generate.py snapshot   # re-snapshot an existing database
    python mockup/data/generate.py config     # only write mock-config.toml
"""

import hashlib
import json
import os
import random
import re
import shutil
import sqlite3
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import uuid
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
SCHEMA = REPO / "src" / "database" / "schema.sql"
CORE_RS = REPO / "src" / "database" / "core.rs"
DB = HERE / "seula-mock.db"
MEDIA_DIR = HERE / "media"
CONFIG = HERE / "mock-config.toml"
OUT_JSON = HERE / "mock-api.json"
OUT_JS = HERE / "mock-api.js"
# For the snapshot of a real plugin scan: empty files named like plugins, scanned by a
# second server over a throwaway copy of the database.
FAKE_PLUGINS = HERE / "fake-plugins"
SCAN_DB = HERE / "seula-scan.db"
SCAN_CONFIG = HERE / "mock-scan-config.toml"

# Off the defaults (50051/50052) so a running real daemon does not collide.
HTTP_PORT = 50152
SCAN_HTTP_PORT = 50154

rng = random.Random(20260923)


def uid() -> str:
    return str(uuid.UUID(int=rng.getrandbits(128), version=4))


def epoch(y, m, d, hh=0, mm=0) -> int:
    return int(datetime(y, m, d, hh, mm, tzinfo=timezone.utc).timestamp())


def rand_time(start: int, end: int) -> int:
    return rng.randint(start, end)


def schema_version() -> int:
    m = re.search(r"pub const SCHEMA_VERSION: i32 = (\d+);", CORE_RS.read_text("utf-8"))
    if not m:
        sys.exit("could not read SCHEMA_VERSION from core.rs")
    return int(m.group(1))


# --------------------------------------------------------------------------- vocab

WORDS_A = """smile again sipper garage pec8 hellfire jazzchords casiopea dance game fart
bocbeat thaaa pickup junlge upbell dist metallic acid rat drum band abstract night
glass velvet static humid neon tape rust hollow copper lunar cheap soft broken slow
ferry orbit polar sugar paper basement rooftop cold warm late early dusty""".split()
WORDS_B = """vapor type deus bass chords rude buster music feedback house ambient techno
loop idea sketch groove jam demo riff break tekno dub garage swing funk drift heat
bounce shuffle tape vox keys pad lead wave pulse""".split()
SUFFIX = ["", "", "", " 2", " 3", " 4", " v2", " final", " (old)", " edit", " 1"]

TAGS = ["wip", "finished", "sketch", "mix ready", "needs master", "vocals", "collab",
        "lofi", "techno", "house", "ambient", "dnb", "game audio", "live set", "archive me"]

TASKS = ["fix kick and bass clash", "bounce stems", "automate filter in chorus",
         "replace placeholder vocal", "check mono compatibility", "export for mastering",
         "tighten drum timing", "rewrite bridge", "send to joel", "sidechain pads",
         "resample the lead", "cut intro by 8 bars", "fix clipping on master",
         "add ear candy in drop", "record real bass", "tidy up arrangement",
         "consolidate audio", "find better snare", "reference against mix",
         "reduce reverb tail"]

NOTES = ["Tempo feels slow, try +4 bpm.", "Mix is close. Vocals too loud in verse 2.",
         "Started on the train. Chords from the voice memo.",
         "Joel has the stems from 3 June.", "Keep the tape wobble, it is the whole point.",
         "Missing the Omnisphere patch on the laptop."]

COLLECTIONS = [
    ("night drives", "Slow tracks for the EP, sequenced for a single side.", "cover-01"),
    ("acid + tekno", "Everything with a 303 in it.", "cover-02"),
    ("band with joel", "Shared sessions and bounces.", "cover-03"),
    ("rat music 3", None, "cover-04"),
    ("abstract", "Weird ones. Not for release.", "cover-05"),
    ("fbm", "Tape-saturated beat tape.", "cover-06"),
    ("drum and bass stuff", None, "cover-07"),
    ("game music", "Loops and stingers for the jam entry.", "cover-08"),
    ("transcription", "Covers and learning pieces.", "cover-09-ns"),
    ("unsorted demos", None, None),  # deliberately no cover art
    ("live set 2027", "For the spring shows. Nothing chosen yet.", None),
]
# Collections seeded with no projects, for the empty tracklist. Last in the list and
# without a cover, so the random stream, and every other row, is unchanged.
EMPTY_COLLECTIONS = {"live set 2027"}

# (name, vendor, kind, is_instrument, installed) -- installed None = never scanned.
PLUGINS = [
    ("Pro-Q 3", "FabFilter", "VST3", False, True), ("Pro-C 2", "FabFilter", "VST3", False, True),
    ("Pro-L 2", "FabFilter", "VST3", False, True), ("Saturn 2", "FabFilter", "VST3", False, True),
    ("Pro-R 2", "FabFilter", "VST3", False, True), ("Serum", "Xfer Records", "VST2", True, True),
    ("OTT", "Xfer Records", "VST2", False, True), ("Vital", "Vital Audio", "VST3", True, True),
    ("Diva", "u-he", "VST3", True, True), ("Repro-1", "u-he", "VST3", True, False),
    ("Zebra2", "u-he", "VST2", True, True), ("Omnisphere", "Spectrasonics", "VST2", True, False),
    ("Kontakt 7", "Native Instruments", "VST3", True, True),
    ("Massive X", "Native Instruments", "VST3", True, True),
    ("Raum", "Native Instruments", "VST3", False, True),
    ("ValhallaVintageVerb", "Valhalla DSP", "VST3", False, True),
    ("ValhallaSupermassive", "Valhalla DSP", "VST3", False, True),
    ("ValhallaDelay", "Valhalla DSP", "VST2", False, True),
    ("Decapitator", "Soundtoys", "VST3", False, True), ("EchoBoy", "Soundtoys", "VST3", False, True),
    ("Little AlterBoy", "Soundtoys", "VST3", False, False),
    ("soothe2", "oeksound", "VST3", False, True), ("RC-20 Retro Color", "XLN Audio", "VST3", False, True),
    ("Addictive Drums 2", "XLN Audio", "VST2", True, None),
    ("Pigments", "Arturia", "VST3", True, True), ("Mini V4", "Arturia", "VST3", True, True),
    ("Jup-8 V4", "Arturia", "VST3", True, False),
    ("Ozone 11 Maximizer", "iZotope", "VST3", False, True), ("Neutron 4", "iZotope", "VST3", False, True),
    ("RX 10 De-click", "iZotope", "VST3", False, None),
    ("ShaperBox 3", "Cableguys", "VST3", False, True), ("Surge XT", "Surge Synth Team", "VST3", True, True),
    ("TAL-U-NO-LX", "TAL Software", "VST3", True, True), ("TAL-Sampler", "TAL Software", "VST2", True, True),
    ("Dexed", "Digital Suburban", "VST3", True, True), ("PhaseMistress", "Soundtoys", "VST2", False, False),
    ("Kickstart 2", "Nicky Romero", "VST3", False, True), ("LFOTool", "Xfer Records", "VST2", False, False),
    ("Portal", "Output", "VST3", False, True), ("Arcade", "Output", "VST3", True, None),
    ("Sausage Fattener", "Dada Life", "VST2", False, True), ("CamelCrusher", "Camel Audio", "VST2", False, False),
    ("Tube Screamer", "TSE Audio", "VST2", False, None), ("Youlean Loudness Meter 2", "Youlean", "VST3", False, True),
    ("SPAN", "Voxengo", "VST3", False, True), ("Sonible smart:EQ 4", "sonible", "VST3", False, True),
    ("Kilohearts Disperser", "Kilohearts", "VST3", False, True), ("Phase Plant", "Kilohearts", "VST3", True, True),
    ("Uhbik-T", "u-he", "VST2", False, True), ("Unknown Plugin", None, "VST2", False, None),
]

# What a scan reports as the category, by plugin. VST3 subcategory strings; vst-meta
# maps VST2's category enum onto the same shape.
CATEGORIES = {
    "Pro-Q 3": "Fx|EQ", "Pro-C 2": "Fx|Dynamics", "Pro-L 2": "Fx|Dynamics",
    "Saturn 2": "Fx|Distortion", "Pro-R 2": "Fx|Reverb", "OTT": "Fx",
    "ValhallaVintageVerb": "Fx|Reverb", "ValhallaSupermassive": "Fx|Reverb|Delay",
    "ValhallaDelay": "Fx", "Decapitator": "Fx|Distortion", "EchoBoy": "Fx|Delay",
    "Little AlterBoy": "Fx|Pitch Shift", "soothe2": "Fx|EQ", "RC-20 Retro Color": "Fx",
    "Ozone 11 Maximizer": "Fx|Mastering", "Neutron 4": "Fx|Mastering",
    "RX 10 De-click": "Fx|Restoration", "ShaperBox 3": "Fx|Modulation",
    "PhaseMistress": "Fx", "Kickstart 2": "Fx|Dynamics", "LFOTool": "Fx",
    "Portal": "Fx|Delay", "Sausage Fattener": "Fx", "CamelCrusher": "Fx",
    "Tube Screamer": "Fx", "Youlean Loudness Meter 2": "Fx|Analyzer",
    "SPAN": "Fx|Analyzer", "Sonible smart:EQ 4": "Fx|EQ",
    "Kilohearts Disperser": "Fx|Filter", "Raum": "Fx|Reverb", "Uhbik-T": "Fx",
    "Kontakt 7": "Instrument|Sampler", "TAL-Sampler": "Instrument",
    "Addictive Drums 2": "Instrument", "Arcade": "Instrument|Sampler",
}
# Vendors whose plugins report a URL; the rest report none, as many real ones do.
VENDOR_URLS = {
    "FabFilter": "https://www.fabfilter.com", "u-he": "https://u-he.com",
    "Valhalla DSP": "https://valhalladsp.com", "Native Instruments": "https://www.native-instruments.com",
    "Arturia": "https://www.arturia.com", "iZotope": "https://www.izotope.com",
    "Kilohearts": "https://kilohearts.com", "Soundtoys": "https://www.soundtoys.com",
}

SAMPLE_DIRS = [
    r"C:\Users\producer\Splice\sounds\packs\{pack}",
    r"C:\Users\producer\Music\Samples\{pack}",
    r"D:\Sample Library\{pack}",
    r"C:\Users\producer\Music\Ableton\User Library\Samples\Recorded",
]
PACKS = ["Lofi Dusty Drums", "Techno Tools Vol 2", "Vocal Chops", "303 Acid Lines",
         "Foley Textures", "Jungle Breaks", "Analog Pads", "One Shots Deluxe",
         "Field Recordings", "Modular Blips"]
SAMPLE_KINDS = ["kick", "snare", "hat", "clap", "perc", "break", "vox", "pad", "fx",
                "bass", "loop", "riser", "texture", "chord", "stab"]
EXTENSIONS = ["wav"] * 8 + ["aif"] * 2 + ["flac", "mp3"]

TONICS = ["C", "CSharp", "D", "DSharp", "E", "F", "FSharp", "G", "GSharp", "A", "ASharp", "B"]
SCALES = ["Major"] * 5 + ["Minor"] * 6 + ["Dorian", "Mixolydian", "Phrygian",
                                          "MinorPentatonic", "HarmonicMinor"]
VERSIONS = [(10, 1, 43), (11, 0, 12), (11, 2, 11), (11, 3, 13), (11, 3, 25),
            (12, 0, 5), (12, 0, 25), (12, 1, 5), (12, 1, 10), (12, 2, 5)]


# --------------------------------------------------------------------------- seed

def project_names(n: int) -> list[str]:
    seen, out = set(), []
    while len(out) < n:
        name = f"{rng.choice(WORDS_A)} {rng.choice(WORDS_B)}{rng.choice(SUFFIX)}"
        if name not in seen:
            seen.add(name)
            out.append(name)
    return out


def seed_scan_details(conn, pid, name, vendor, kind, instr, installed, uid_hex, folder, ext, now):
    """The rest of what a scan records: buses, MIDI, latency, format extras, VST3 classes.

    From a per-plugin stream, so the main rng -- and so every other row -- is the same
    as before this existed. A sweep keeps the scanner columns of a plugin it no longer
    finds (``sweep_unseen`` only clears ``installed``), so some missing plugins carry an
    old scan record: they were installed once, and have been removed since.
    """
    prng = random.Random(pid)
    was_found = installed is True or (installed is False and prng.random() < 0.6)
    if not was_found:
        return
    category = CATEGORIES.get(name, "Instrument|Synth" if instr else "Fx")
    if kind == "VST2" and category.startswith("Instrument"):
        category = "Instrument"
    sidechain = not instr and prng.random() < 0.4
    extra_outs = instr and prng.random() < 0.3
    fields = dict(
        path=f"{folder}\\{name}.{ext}",
        category=category,
        is_instrument=int(instr),
        audio_in_channels=4 if sidechain else (0 if instr else 2),
        audio_out_channels=16 if extra_outs else 2,
        has_midi_input=int(instr or prng.random() < 0.15),
        has_midi_output=int(prng.random() < 0.08),
        latency_samples=0 if instr else prng.choice([0, 0, 0, 0, 64, 256, 1024, 4096]),
        has_gui=1,
        vendor_url=VENDOR_URLS.get(vendor),
        is_shell=0,
    )
    if installed is False:
        # The scan that stopped finding it.
        fields["last_scanned_at"] = now - 86400 * 2
    if kind == "VST3":
        fields["audio_in_buses"] = 0 if instr else (2 if sidechain else 1)
        fields["audio_out_buses"] = 8 if extra_outs else 1
        fields["factory_flags"] = prng.choice([16, 16, 17, 24])
    else:
        fields.update(
            fourcc="".join(prng.choice("ABCDEFGHKLMNPRSTVXZabcdefghklmnprstuvxz0123456789") for _ in range(4)),
            preset_chunks=int(prng.random() < 0.8),
            f64_precision=int(prng.random() < 0.3),
            silent_when_stopped=int(prng.random() < 0.2),
            midi_in_channels=16 if instr else 0,
            midi_out_channels=0,
        )
    sets = ", ".join(f"{k} = ?" for k in fields)
    conn.execute(f"UPDATE plugins SET {sets} WHERE id = ?", (*fields.values(), pid))

    if kind != "VST3":
        return
    version = f"{prng.randint(1, 4)}.{prng.randint(0, 12)}.{prng.randint(0, 9)}"
    controller = f"{prng.getrandbits(128):032x}"
    conn.execute("INSERT INTO plugin_classes VALUES (?,?,?,?,?,?)",
                 (pid, name, "Audio Module Class", uid_hex, 2147483647, version))
    conn.execute("INSERT INTO plugin_classes VALUES (?,?,?,?,?,?)",
                 (pid, f"{name} Controller", "Component Controller Class", controller, 2147483647, version))
    buses = []
    if not instr:
        buses.append(("input", "audio", "Stereo In", 2, 0, 1))
        if sidechain:
            buses.append(("input", "audio", "Sidechain", 2, 1, 0))
    buses.append(("output", "audio", "Stereo Out", 2, 0, 1))
    if extra_outs:
        buses += [("output", "audio", f"Out {n}-{n + 1}", 2, 1, 0) for n in range(3, 17, 2)]
    if fields["has_midi_input"]:
        buses.append(("input", "event", "MIDI In", 16, 0, 1))
    for b in buses:
        conn.execute("INSERT INTO plugin_buses VALUES (?,?,?,?,?,?,?)", (pid, *b))


def seed_sample_file(conn, sid, fname, present, now):
    """What the last sample check measured (ADR-0041).

    From a per-sample stream, so every other row is unchanged. Recordings are long,
    loops and breaks middling, one-shots small; compressed formats are smaller. Most
    missing samples were found by an earlier check and keep the size they had; the rest
    were never found, and have no row.
    """
    srng = random.Random(sid)
    if not present and srng.random() < 0.3:
        return
    ext = fname.rsplit(".", 1)[-1].lower()
    if fname[0].isupper():  # "Audio 12 [2024-...].wav": a recording
        size = srng.randint(4_000_000, 90_000_000)
    elif any(k in fname for k in ("loop", "break", "pad", "texture", "riser")):
        size = srng.randint(600_000, 12_000_000)
    else:
        size = srng.randint(30_000, 1_800_000)
    size = int(size * {"flac": 0.6, "mp3": 0.12}.get(ext, 1.0))
    checked = now - 86400 if present else now - 86400 * srng.randint(20, 300)
    conn.execute("INSERT INTO sample_files VALUES (?,?,?,?)",
                 (sid, size, checked - srng.randint(86400, 86400 * 900), checked))


def seed() -> None:
    if DB.exists():
        DB.unlink()
    for suffix in ("-wal", "-shm"):
        p = Path(str(DB) + suffix)
        if p.exists():
            p.unlink()
    shutil.rmtree(MEDIA_DIR, ignore_errors=True)

    conn = sqlite3.connect(DB)
    conn.execute("PRAGMA foreign_keys = ON")
    conn.executescript(SCHEMA.read_text("utf-8"))
    # Without this the program sees an old-schema database and discards it (ADR-0011).
    conn.execute(f"PRAGMA user_version = {schema_version()}")

    now = epoch(2026, 9, 23, 12)
    t0 = epoch(2021, 3, 1)

    # media: covers and audition bounces
    cover_ids = {}
    for _, _, cover in COLLECTIONS:
        if cover is None:
            continue
        mid = uid()
        cover_ids[cover] = mid
        path = REPO / "mockup" / "images" / "thumbs" / f"{cover}.jpg"
        conn.execute(
            "INSERT INTO media_files VALUES (?,?,?,?,?,?,?,?)",
            (mid, f"{cover}.jpg", "jpg", "cover_art", path.stat().st_size, "image/jpeg",
             rand_time(t0, now), hashlib.sha256(path.read_bytes()).hexdigest()))

    # tags
    tag_ids = {}
    for t in TAGS:
        tag_ids[t] = uid()
        conn.execute("INSERT INTO tags VALUES (?,?,?)", (tag_ids[t], t, rand_time(t0, now)))

    # plugins
    plugins = []
    for name, vendor, kind, instr, installed in PLUGINS:
        pid = uid()
        if kind == "VST3":
            raw = rng.getrandbits(128).to_bytes(16, "big")
            uid_hex = raw.hex()
            dashed = str(uuid.UUID(bytes=raw))
            dev = f"device:vst3:{'instr' if instr else 'audiofx'}:{dashed}"
        else:
            n = rng.getrandbits(31)
            uid_hex = f"{n:08x}"
            dev = f"device:vst:{'instr' if instr else 'audiofx'}:{n}?n={name.replace(' ', '%20')}"
        fmt = f"{kind} {'Instrument' if instr else 'Effect'}"
        scanned = installed is True
        folder = r"C:\Program Files\Common Files\VST3" if kind == "VST3" else r"C:\Program Files\VSTPlugins"
        ext = "vst3" if kind == "VST3" else "dll"
        conn.execute(
            """INSERT INTO plugins (id, plugin_kind, uid, name, format, vendor, version,
                installed, last_scanned_at, dev_identifier, path, category, is_instrument,
                audio_in_channels, audio_out_channels, has_midi_input, presets, parameters,
                has_gui) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)""",
            (pid, kind, uid_hex, name, fmt, vendor,
             f"{rng.randint(1, 4)}.{rng.randint(0, 12)}.{rng.randint(0, 9)}" if vendor else None,
             None if installed is None else int(installed),
             now - 86400 * 2 if installed is not None else None, dev,
             f"{folder}\\{name}.{ext}" if scanned else None,
             ("Instrument|Synth" if instr else "Fx|EQ") if scanned else None,
             int(instr) if scanned else None,
             (0 if instr else 2) if scanned else None, 2 if scanned else None,
             int(instr) if scanned else None,
             rng.randint(0, 900) if scanned else None,
             rng.randint(8, 400) if scanned else None, 1 if scanned else None))
        conn.execute("INSERT INTO plugin_refs VALUES (?,?,?,?,?,?)",
                     (dev, pid, name, "instr" if instr else "audiofx",
                      "uid" if scanned else "created", rand_time(t0, now)))
        seed_scan_details(conn, pid, name, vendor, kind, instr, installed, uid_hex, folder, ext, now)
        plugins.append((pid, instr))
    instr_ids = [p for p, i in plugins if i]
    fx_ids = [p for p, i in plugins if not i]
    # a few plugins carry most of the usage, like real libraries
    fx_weights = [8 if i < 6 else 1 for i in range(len(fx_ids))]

    # samples
    samples = []
    used_paths = set()
    while len(samples) < 420:
        d = rng.choice(SAMPLE_DIRS).format(pack=rng.choice(PACKS))
        kind = rng.choice(SAMPLE_KINDS)
        ext = rng.choice(EXTENSIONS)
        if "Recorded" in d:
            fname = f"{rng.choice(['Audio', 'Vocal', 'Bass', 'Guitar'])} {rng.randint(1, 40)} [{rng.randint(2021, 2026)}-0{rng.randint(1, 9)}-1{rng.randint(0, 9)} {rng.randint(100000, 235959)}].wav"
        else:
            fname = f"{kind}_{rng.choice(['dry', 'wet', 'hard', 'soft', 'tape', 'long', 'short', 'clean'])}_{rng.randint(1, 60):02d}.{ext}"
        path = f"{d}\\{fname}"
        if path in used_paths:
            continue
        used_paths.add(path)
        sid = uid()
        present = rng.random() > 0.09
        conn.execute("INSERT INTO samples VALUES (?,?,?,?)", (sid, fname, path, int(present)))
        seed_sample_file(conn, sid, fname, present, now)
        samples.append(sid)

    # projects
    names = project_names(240)
    project_ids = []
    for i, name in enumerate(names):
        pid = uid()
        project_ids.append(pid)
        folder_name = name if rng.random() > 0.15 else rng.choice(WORDS_A) + " idea"
        file_name = name if rng.random() > 0.2 else f"{name} {rng.choice(['mixdown', 'v1', 'backup', 'old'])}"
        path = rf"C:\Users\producer\Music\Ableton\Projects\{folder_name} Project\{file_name}.als"
        # display name differs from the file name for some (the rename feature)
        display = name if rng.random() > 0.1 else name.title()
        created = rand_time(t0, now - 86400 * 3)
        # most projects stop being touched within weeks; a few are revisited much later
        modified = created + int((now - created) * rng.random() ** 3)
        major, minor, patch = rng.choice(VERSIONS)
        beta = rng.random() < 0.04
        key = rng.random() < 0.7
        ts = rng.choices([(4, 4), (3, 4), (6, 8), (7, 8), (5, 4)], weights=[85, 6, 4, 3, 2])[0]
        tempo = round(rng.choice([rng.uniform(70, 100), rng.uniform(118, 132), rng.uniform(160, 176)]), rng.choice([0, 0, 0, 2]))
        duration = rng.randint(45, 420) if rng.random() > 0.06 else None
        bar = round(duration * tempo / 60 / ts[0], 2) if duration else None
        audio = None
        if rng.random() < 0.35:
            audio = uid()
            conn.execute("INSERT INTO media_files VALUES (?,?,?,?,?,?,?,?)",
                         (audio, f"{file_name}.wav", "wav", "audio_file",
                          rng.randint(8, 70) * 1_000_000, "audio/wav", modified,
                          hashlib.sha256(pid.encode()).hexdigest()))
        conn.execute(
            """INSERT INTO projects (is_active, id, path, name, hash, notes, created_at,
                modified_at, last_parsed_at, tempo, time_signature_numerator,
                time_signature_denominator, key_signature_tonic, key_signature_scale,
                duration_seconds, furthest_bar, daw_type, daw_version_display, audio_file_id)
               VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)""",
            (int(rng.random() > 0.08), pid, path, display,
             hashlib.sha256(path.encode()).hexdigest(),
             rng.choice(NOTES) if rng.random() < 0.18 else None,
             created, modified, now - rng.randint(0, 86400 * 5), tempo, ts[0], ts[1],
             rng.choice(TONICS) if key else None, rng.choice(SCALES) if key else None,
             duration, bar, "Ableton Live",
             f"{major}.{minor}.{patch}" + (" beta" if beta else ""), audio))
        if audio:
            # ADR-0037: a list of audios, one of them primary. Extras come from their
            # own per-project stream so the main rng, and so the rest of the data, is
            # unchanged by them.
            arng = random.Random(pid)
            listed = [audio]
            for suffix in arng.choices([[], [" (master)"], [" (master)", " alt mix"]], weights=[55, 30, 15])[0]:
                extra = str(uuid.UUID(int=arng.getrandbits(128), version=4))
                conn.execute("INSERT INTO media_files VALUES (?,?,?,?,?,?,?,?)",
                             (extra, f"{file_name}{suffix}.wav", "wav", "audio_file",
                              arng.randint(8, 70) * 1_000_000, "audio/wav", modified,
                              hashlib.sha256((pid + suffix).encode()).hexdigest()))
                listed.append(extra)
            for pos, mid in enumerate(listed):
                conn.execute("INSERT INTO project_audio_files VALUES (?,?,?,?)", (pid, mid, pos, modified))
            if len(listed) > 1 and arng.random() < 0.5:
                # Sometimes the master, not the first bounce, is what the row plays.
                conn.execute("UPDATE projects SET audio_file_id = ? WHERE id = ?", (listed[1], pid))
            if len(listed) > 1 and arng.random() < 0.12:
                # Primary cleared: audios listed, nothing plays in the row (ADR-0037).
                conn.execute("UPDATE projects SET audio_file_id = NULL WHERE id = ?", (pid,))
        conn.execute("INSERT INTO project_ableton_metadata VALUES (?,?,?,?,?)",
                     (pid, major, minor, patch, int(beta)))

        used = set(rng.sample(instr_ids, rng.randint(0, 4)))
        used |= set(rng.choices(fx_ids, weights=fx_weights, k=rng.randint(0, 9)))
        for plug in used:
            conn.execute("INSERT INTO project_plugins VALUES (?,?)", (pid, plug))
        for s in rng.sample(samples, rng.choice([0, 1, 3, 5, 8, 12, 20, 34])):
            conn.execute("INSERT INTO project_samples VALUES (?,?)", (pid, s))
        for t in rng.sample(TAGS, rng.choices([0, 1, 2, 3], weights=[40, 35, 18, 7])[0]):
            conn.execute("INSERT INTO project_tags VALUES (?,?,?)", (pid, tag_ids[t], modified))
        if rng.random() < 0.3:
            for desc in rng.sample(TASKS, rng.randint(1, 6)):
                conn.execute("INSERT INTO project_tasks VALUES (?,?,?,?,?)",
                             (uid(), pid, desc, int(rng.random() < 0.4), rand_time(created, now)))

    # collections
    for name, desc, cover in COLLECTIONS:
        cid = uid()
        created = rand_time(t0, now - 86400 * 10)
        conn.execute("INSERT INTO collections VALUES (?,?,?,?,?,?,?)",
                     (cid, name, desc, None, created, rand_time(created, now),
                      cover_ids.get(cover)))
        members = [] if name in EMPTY_COLLECTIONS else rng.sample(project_ids, rng.randint(3, 14))
        for pos, p in enumerate(members):
            conn.execute("INSERT INTO collection_projects VALUES (?,?,?,?)",
                         (cid, p, pos, rand_time(created, now)))

    conn.commit()
    conn.close()
    print(f"seeded {DB.relative_to(REPO)}: {len(names)} projects, {len(PLUGINS)} plugins, "
          f"{len(samples)} samples, {len(COLLECTIONS)} collections")


# --------------------------------------------------------------------------- snapshot

def get(path: str, port: int = HTTP_PORT):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}{path}", timeout=30) as r:
        return json.loads(r.read())


def q(value: str) -> str:
    return urllib.parse.quote(value, safe="")


def write_config(path: Path, db: Path, http_port: int, vst_paths: list) -> None:
    toml_path = lambda p: str(p).replace("\\", "/")
    path.write_text(
        "paths = []\n"
        f"database_path = '{toml_path(db)}'\n"
        f"media_storage_dir = '{toml_path(MEDIA_DIR)}'\n"
        f"http_port = {http_port}\n"
        "log_level = 'error'\n"
        f"vst_search_paths = [{', '.join(repr(toml_path(v)) for v in vst_paths)}]\n", "utf-8")


def start_server(config: Path, port: int):
    env = {k: v for k, v in os.environ.items() if not k.startswith("SEULA_")}
    exe = REPO / "target" / "debug" / ("seula.exe" if os.name == "nt" else "seula")
    server = subprocess.Popen([str(exe), "--config", str(config), "--server"], cwd=REPO, env=env)
    for _ in range(100):
        try:
            get("/health", port)
            return server
        except OSError:
            if server.poll() is not None:
                sys.exit(f"seula --server exited with {server.returncode}")
            time.sleep(0.2)
    server.terminate()
    sys.exit("server did not come up")


def scan_streams() -> dict:
    """The event streams of a real ``POST /api/v1/plugins/scan`` (ADR-0038) and a real
    ``POST /api/v1/samples/check`` (ADR-0041), keyed like the other responses.

    The seeded plugins do not exist on disk, so the plugin scan looks at a folder of
    empty files named like some of them; every one fails to load, which a real scan
    reports the same way. The seeded sample paths do not exist either, so the check
    finds every sample missing. Both write their result, so they run against a copy of
    the database, one after the other (only one scan runs at a time).
    """
    shutil.rmtree(FAKE_PLUGINS, ignore_errors=True)
    FAKE_PLUGINS.mkdir()
    for name, _, kind, _, _ in PLUGINS[:14]:
        (FAKE_PLUGINS / f"{name}.{'vst3' if kind == 'VST3' else 'dll'}").write_bytes(b"")
    shutil.copyfile(DB, SCAN_DB)
    write_config(SCAN_CONFIG, SCAN_DB, SCAN_HTTP_PORT, [FAKE_PLUGINS])
    server = start_server(SCAN_CONFIG, SCAN_HTTP_PORT)
    streams = {}
    try:
        for route in ("/api/v1/plugins/scan", "/api/v1/samples/check"):
            req = urllib.request.Request(f"http://127.0.0.1:{SCAN_HTTP_PORT}{route}", method="POST")
            with urllib.request.urlopen(req, timeout=120) as r:
                streams[f"POST {route}"] = [json.loads(line[5:]) for line in r.read().decode().splitlines()
                                            if line.startswith("data:")]
    finally:
        server.terminate()
        server.wait(timeout=10)
        shutil.rmtree(FAKE_PLUGINS, ignore_errors=True)
        for suffix in ("", "-wal", "-shm"):
            Path(str(SCAN_DB) + suffix).unlink(missing_ok=True)
        SCAN_CONFIG.unlink(missing_ok=True)
    return streams


def dedupe_projects(api: dict) -> None:
    """Store a plugin's or sample's "used in" list, and a collection's tracklist, as
    project ids.

    They hold the same project DTOs as ``/api/v1/projects`` (and, under ``scope=all``,
    ``?scope=deleted``), and written out in full for every plugin they would be most of
    the file. Each is checked equal to the main list's copy before it is replaced, and
    the mockup puts them back (``apiGet`` in shell.js). Equal up to the order of a
    project's plugins and samples, which the API does not define and which differs
    between routes. A collection's tracklist is a bare list, so it is stored as
    ``{"project_ids": [...], "as_list": true}`` and given back as a list.
    """
    def canonical(p):
        return {**p, **{k: sorted(p[k], key=lambda x: x["id"]) for k in ("plugins", "samples")}}

    by_id = {p["id"]: canonical(p)
             for key in ("/api/v1/projects?limit=10000", "/api/v1/projects?scope=deleted&limit=10000")
             for p in api[key]["projects"]}
    same = lambda projects: all(by_id.get(p["id"]) == canonical(p) for p in projects)
    for path, body in api.items():
        if path.startswith("/api/v1/collections/") and path.split("?")[0].endswith("/projects") and isinstance(body, list):
            if not same(body):
                sys.exit(f"{path}: a project differs from the main list's copy")
            api[path] = {"project_ids": [p["id"] for p in body], "as_list": True}
            continue
        if not path.startswith(("/api/v1/plugins/", "/api/v1/samples/")) or not isinstance(body, dict) or "projects" not in body:
            continue
        if same(body["projects"]):
            body["project_ids"] = [p["id"] for p in body.pop("projects")]


def snapshot() -> None:
    if not DB.exists():
        sys.exit("no database; run `seed` first")
    MEDIA_DIR.mkdir(exist_ok=True)
    write_config(CONFIG, DB, HTTP_PORT, [])
    subprocess.run(["cargo", "build", "--quiet"], cwd=REPO, check=True)
    server = start_server(CONFIG, HTTP_PORT)
    try:
        big = "limit=10000"
        api = {}
        for path in [
            f"/api/v1/projects?{big}", f"/api/v1/projects?scope=deleted&{big}",
            "/api/v1/projects/statistics",
            f"/api/v1/tags?{big}", f"/api/v1/tags/with-usage?{big}", "/api/v1/tags/statistics",
            f"/api/v1/collections?{big}",
            f"/api/v1/plugins?{big}", "/api/v1/plugins/stats", f"/api/v1/plugins/vendors?{big}",
            "/api/v1/plugins/formats",
            f"/api/v1/samples?{big}", "/api/v1/samples/stats", "/api/v1/samples/analytics",
            "/api/v1/samples/formats",
            f"/api/v1/media?{big}", "/api/v1/tasks/statistics",
            "/api/v1/system/info", "/api/v1/system/scan-status",
            # The stats view, in both project scopes (ADR-0045).
            "/api/v1/system/statistics", "/api/v1/system/statistics?scope=all",
            "/api/v1/config/status",
            # A plain term and one of the search operators, for the projects board's
            # search frames.
            "/api/v1/search?query=techno&limit=200", "/api/v1/search?query=plugin:serum&limit=200",
        ]:
            api[path] = get(path)

        plugins = api[f"/api/v1/plugins?{big}"]["plugins"]
        for pl in plugins:
            api[f"/api/v1/plugins/{pl['id']}"] = get(f"/api/v1/plugins/{pl['id']}")
            api[f"/api/v1/plugins/{pl['id']}/projects?{big}"] = get(f"/api/v1/plugins/{pl['id']}/projects?{big}")
        # The status bar's counts under each single filter the toolbar can set, and the
        # combinations the board's frames use.
        stats_queries = [f"vendor_filter={q(v['vendor'])}" for v in api[f"/api/v1/plugins/vendors?{big}"]["vendors"]]
        stats_queries += [f"format_filter={q(f['format'])}" for f in api["/api/v1/plugins/formats"]["formats"]]
        stats_queries += [f"install_states={st}" for st in
                          ("installed", "absent", "unscanned", "installed,absent", "installed,unscanned", "absent,unscanned")]
        stats_queries += ["query=pro", "format_filter=VST3%20Effect&install_states=absent,unscanned"]
        for sq in stats_queries:
            api[f"/api/v1/plugins/stats?{sq}"] = get(f"/api/v1/plugins/stats?{sq}")
        api[f"/api/v1/plugins/search?query=pro&{big}"] = get(f"/api/v1/plugins/search?query=pro&{big}")

        # Samples: each one's detail and used-in list, the status bar under each filter
        # the toolbar can set and the frames' combinations, and a search.
        for s in api[f"/api/v1/samples?{big}"]["samples"]:
            api[f"/api/v1/samples/{s['id']}"] = get(f"/api/v1/samples/{s['id']}")
            api[f"/api/v1/samples/{s['id']}/projects?{big}"] = get(f"/api/v1/samples/{s['id']}/projects?{big}")
        sample_stats = [f"format_filter={f['format']}" for f in api["/api/v1/samples/formats"]["formats"]]
        sample_stats += ["present_only=true", "missing_only=true", "query=kick",
                         "format_filter=aiff&missing_only=true", "query=kick&missing_only=true"]
        for sq in sample_stats:
            api[f"/api/v1/samples/stats?{sq}"] = get(f"/api/v1/samples/stats?{sq}")
        api[f"/api/v1/samples/search?query=kick&{big}"] = get(f"/api/v1/samples/search?query=kick&{big}")

        # Collections (ADR-0043, ADR-0044): the list in each order the grid's Sort menu
        # and the table's headers ask for, in both scopes, and a search; then each
        # collection's detail, tracklist, tasks and statistics, in both scopes.
        api[f"/api/v1/collections?scope=all&{big}"] = get(f"/api/v1/collections?scope=all&{big}")
        for sort in ("name", "project_count", "total_duration", "created_at", "modified_at"):
            for desc in ("false", "true"):
                path = f"/api/v1/collections?sort_by={sort}&sort_desc={desc}&{big}"
                api[path] = get(path)
        api[f"/api/v1/collections/search?query=tape&{big}"] = get(f"/api/v1/collections/search?query=tape&{big}")
        for c in api[f"/api/v1/collections?{big}"]["collections"]:
            cid = c["id"]
            for sub in ("", "/projects", "/tasks", "/statistics"):
                api[f"/api/v1/collections/{cid}{sub}"] = get(f"/api/v1/collections/{cid}{sub}")
                api[f"/api/v1/collections/{cid}{sub}?scope=all"] = get(f"/api/v1/collections/{cid}{sub}?scope=all")
    finally:
        server.terminate()
        server.wait(timeout=10)

    api.update(scan_streams())
    dedupe_projects(api)

    OUT_JSON.write_text(json.dumps(api, indent=1), "utf-8")
    OUT_JS.write_text(
        "// Generated by mockup/data/generate.py from the real HTTP API over a seeded\n"
        "// database. Keys are request paths. Do not edit; regenerate.\n"
        f"window.SEULA_API = {json.dumps(api, separators=(',', ':'))};\n", "utf-8")
    print(f"snapshot: {len(api)} responses -> {OUT_JSON.relative_to(REPO)} "
          f"({OUT_JSON.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    step = sys.argv[1] if len(sys.argv) > 1 else "all"
    if step == "config":
        # Only the config the frontend's dev daemon runs from; `snapshot` writes it too.
        MEDIA_DIR.mkdir(exist_ok=True)
        write_config(CONFIG, DB, HTTP_PORT, [])
    if step in ("seed", "all"):
        seed()
    if step in ("snapshot", "all"):
        snapshot()
