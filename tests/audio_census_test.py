"""Synthetic checks for tools/audio_census.py. No game files: every byte string is built here.

The fixtures mirror the layout the tool *assumes* (fitted to the installed files, see
docs/verification/SLICE_AUDIO_CHAIN.md), so these tests prove the code does what it says and that the
oracles can fail; they do not prove the layout. The oracle numbers on the real install are in that record.

Run: python tests/audio_census_test.py
"""
import io
import os
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tools"))
import audio_census as ac  # noqa: E402

P = struct.pack


def chunk(tag, body):
    return tag + P("<I", len(body)) + body


def hirc_obj(kind, ident, rest=b""):
    body = P("<I", ident) + rest
    return P("<BI", kind, len(body)) + body


def sound(ident, source, stream=0, plugin=0x00040001):
    return hirc_obj(2, ident, P("<3I", plugin, stream, source) + b"\0" * 8)


def action(ident, target, bank):
    return hirc_obj(3, ident, P("<HI", ac.PLAY_ACTION, target) + b"\0\0\x04" + P("<I", bank))


def event(ident, actions):
    return hirc_obj(4, ident, P("<I", len(actions)) + b"".join(P("<I", a) for a in actions))


def switch_container(ident, children):
    # arbitrary node parameters, then count + child ids, then trailing bytes (the parser must find the run)
    return hirc_obj(6, ident, b"\x01\x02\x03\x04\x05" * 3 + P("<I", len(children)) + b"".join(P("<I", c) for c in children) + b"\x07" * 6)


def wem(samples, rate=48000, channels=1):
    fmt = P("<HHIIHH", 0xFFFF, channels, rate, 1000, 0, 16) + b"\0" * 8 + P("<I", samples) + b"\0" * 40
    fmt = fmt[:0x42]
    body = b"WAVE" + chunk(b"fmt ", fmt) + chunk(b"data", b"\0" * 10)
    return b"RIFF" + P("<I", len(body)) + body


def make_bank(bank_id, objects, media, names=()):
    didx, data, off = b"", b"", 0
    for source, blob in media:
        didx += P("<3I", source, off, len(blob))
        data += blob
        off += len(blob)
    hirc = P("<I", len(objects)) + b"".join(objects)
    stid = P("<II", 1, len(names)) + b"".join(P("<IB", ac.fnv1_32(n), len(n)) + n.encode() for n in names)
    out = chunk(b"BKHD", P("<II", 62, bank_id) + b"\0" * 8)
    if media:
        out += chunk(b"DIDX", didx) + chunk(b"DATA", data)
    return out + chunk(b"HIRC", hirc) + chunk(b"STID", stid)


def make_pck(banks=(), sounds=(), langs=(("sfx", 0),)):
    """banks/sounds: lists of (id, bytes, lang id). Data is packed back to back after the header."""
    lang = P("<I", len(langs))
    strings = b""
    for i, (name, lid) in enumerate(langs):
        lang += P("<II", 4 + 8 * len(langs) + len(strings), lid)
        strings += name.encode("utf-16le") + b"\0\0"
    lang += strings
    tables = []
    for entries in (banks, sounds):
        tables.append([list(e) for e in entries])
    sizes = [4 + 20 * len(t) for t in tables]
    header_size = 4 * 5 + len(lang) + sum(sizes) + 4
    pos = 8 + header_size
    body = b""
    packed = []
    for t in tables:
        rows = b""
        for ident, blob, lid in t:
            rows += P("<5I", ident, 1, len(blob), pos, lid)
            body += blob
            pos += len(blob)
        packed.append(P("<I", len(t)) + rows)
    head = b"AKPK" + P("<6I", header_size, 1, len(lang), sizes[0], sizes[1], 4) + lang + packed[0] + packed[1] + P("<I", 0)
    return head + body


def write(path, data):
    with open(path, "wb") as f:
        f.write(data)


class Hash(unittest.TestCase):
    def test_fnv1_is_case_insensitive_and_matches_published_vectors(self):
        self.assertEqual(ac.fnv1_32(""), 0x811C9DC5)
        self.assertEqual(ac.fnv1_32("a"), 0x050C5D7E)  # FNV-1 (multiply, then xor), not FNV-1a
        self.assertEqual(ac.fnv1_32("Ab_C"), ac.fnv1_32("aB_c"))


class Pck(unittest.TestCase):
    def parse(self, blob):
        return ac.parse_pck_header(io.BytesIO(blob), len(blob))

    def test_tables_and_exact_tiling(self):
        blob = make_pck(banks=[(7, b"B" * 30, 0)], sounds=[(100, b"S" * 10, 1), (101, b"T" * 5, 1)], langs=[("english(us)", 1), ("sfx", 0)])
        h = self.parse(blob)
        self.assertEqual(h["languages"], {1: "english(us)", 0: "sfx"})
        self.assertEqual([e["id"] for e in h["banks"]], [7])
        self.assertEqual([(e["id"], e["size"]) for e in h["sounds"]], [(100, 10), (101, 5)])
        self.assertTrue(h["oracle"]["entries_tile_file"] and h["oracle"]["banks_count_matches_size"])

    def test_oracle_fails_on_truncated_or_padded_file(self):
        blob = make_pck(sounds=[(1, b"x" * 20, 0)])
        self.assertFalse(self.parse(blob[:-1])["oracle"]["entries_tile_file"])
        self.assertFalse(self.parse(blob + b"\0")["oracle"]["entries_tile_file"])

    def test_rejects_other_files(self):
        with self.assertRaises(ValueError):
            self.parse(b"RIFF" + b"\0" * 64)


class Bank(unittest.TestCase):
    def test_chunk_walk_hirc_and_decoders(self):
        bank = make_bank(5, [sound(10, 900), action(11, 10, 5), event(12, [11])], [(900, wem(100))], names=["Bank_A"])
        fh = io.BytesIO(bank)
        chunks, tiles = ac.read_bank(fh, 0, len(bank))
        self.assertTrue(tiles)
        self.assertEqual(sorted(chunks), ["BKHD", "DATA", "DIDX", "HIRC", "STID"])
        objects, hirc_tiles = ac.parse_hirc(ac.read_chunk(fh, chunks, "HIRC"))
        self.assertTrue(hirc_tiles)
        self.assertEqual([(k, i) for k, i, _ in objects], [(2, 10), (3, 11), (4, 12)])
        self.assertEqual(ac.decode_event(objects[2][2]), ([11], True))
        self.assertEqual(ac.decode_action(objects[1][2]), {"type": ac.PLAY_ACTION, "target": 10, "bank": 5, "play_layout_ok": True})
        self.assertEqual(ac.decode_sound(objects[0][2]), {"plugin": 0x40001, "stream": 0, "source": 900})

    def test_chunk_walk_detects_untiled_bank(self):
        bank = make_bank(5, [], [])
        self.assertFalse(ac.read_bank(io.BytesIO(bank + b"\0" * 3), 0, len(bank) + 3)[1])

    def test_container_children_finds_longest_run_of_known_ids(self):
        known = {1, 2, 3, 77}
        body = P("<I", 50) + switch_container(50, [1, 2, 3])[9:]
        self.assertEqual(ac.container_children(body, known), [1, 2, 3])
        self.assertEqual(ac.container_children(body, {9}), [])


class Riff(unittest.TestCase):
    def test_vorbis_header_and_duration(self):
        info = ac.riff_info(wem(96000, rate=48000))
        self.assertEqual((info["codec"], info["channels"], info["sample_rate"], info["vorbis_samples"]), ("wwise_vorbis", 1, 48000, 96000))
        self.assertEqual(info["duration_s"], 2.0)

    def test_not_riff(self):
        self.assertEqual(ac.riff_info(b"OggS" + b"\0" * 40), {"riff": False})


class EndToEnd(unittest.TestCase):
    """A fake game folder: one SFX pck (bank with in-bank media + a switch container) and a streaming pck."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        cooked = os.path.join(self.tmp.name, "WillowGame", "CookedPCConsole")
        os.makedirs(cooked)
        sfx = make_bank(0xAB, [sound(1, 901), sound(2, 902, stream=1), sound(3, 777, plugin=0x650002),
                               switch_container(4, [1, 2]), action(5, 4, 0xAB), event(6, [5]),
                               action(7, 2, 0xAB), event(8, [7])],
                        [(901, wem(44100, 44100))], names=["Bank_Test"])
        write(os.path.join(cooked, "Audio_Banks.pck"), make_pck(banks=[(0xAB, sfx, 0)]))
        self.streamed = wem(48000)
        write(os.path.join(cooked, "Audio_Streaming.pck"), make_pck(sounds=[(902, self.streamed, 0)]))
        self.inst = ac.Install(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_census_oracles_all_pass(self):
        res = ac.census(self.inst)
        o = res["oracles"]
        self.assertEqual((res["bank_count"], res["distinct_event_ids"]), (1, 2))
        self.assertEqual((o["events"], o["event_actions_resolve_in_bank"], o["play_target_resolves_in_bank"], o["play_bank_equals_owner"]), (2, 2, 2, 2))
        self.assertEqual((o["sounds"], o["sound_source_in_didx"], o["sound_source_in_stream_table"]), (3, 1, 1))
        self.assertEqual((o["stid_names"], o["stid_fnv_matches_bank_id"]), (1, 0))  # the fixture's bank id is not the hash of its name
        self.assertTrue(o["bank_chunks_tile"] == o["bkhd_id_equals_table_id"] == o["hirc_tiles"] == 1)

    def test_resolve_through_switch_container_reads_exact_bytes(self):
        rows = ac.resolve(self.inst, "0x6")["rows"]
        self.assertEqual(rows[0]["target_kind"], "switch")
        self.assertEqual(sorted(s["source"] for s in rows[0]["sounds"]), [901, 902])
        by = {s["source"]: s for s in rows[0]["sounds"]}
        self.assertEqual(by[902]["where"], "stream")
        self.assertEqual(ac.read_media(self.inst, by[902]), self.streamed)
        self.assertEqual(ac.riff_info(ac.read_media(self.inst, by[901])[:128])["sample_rate"], 44100)

    def test_unknown_event_and_name_hashing(self):
        self.assertEqual(ac.resolve(self.inst, "0x99")["rows"], [])
        self.assertEqual(ac._event_id("Play_X"), ac.fnv1_32("play_x"))


if __name__ == "__main__":
    unittest.main()
