# Slice audio chain: stock event -> Wwise bank -> `.wem` bytes (2026-10-01)

AI-assisted (Claude Sonnet 5.5, directed by the maintainer). Identification and feasibility only: **no audio was
decoded or played, nothing was heard, and no original-game capture was taken.** Extraction (ours), oracle checks and
the decode benchmark are reported separately. Generated output is game-derived and lives under ignored `local/`.
Reproduce: `python tools/audio_census.py census`, `python tools/audio_slice_chain.py` (about 5 minutes; most of it is
`--object-dump` on `Sanctuary_Dynamic`), `python tests/audio_census_test.py`.

## 1. Where the audio is

Base game, `WillowGame/CookedPCConsole` (the only audio files for the base game; there is no loose `.bnk`/`.wem`):

| File | Bytes | Banks | Streaming sounds | Language |
|---|---|---|---|---|
| `Audio_Banks.pck` | 320,565,120 | 57 | 0 | sfx |
| `Audio_Streaming.pck` | 252,055,681 | 0 | 550 | sfx |
| `English(US)/Audio_Banks.pck` | 11,822,307 | 43 | 0 | english(us) |
| `English(US)/Audio_Streaming.pck` | 504,137,179 | 0 | 14,702 | english(us) |

Only English(US) VO is installed for the base game. `DLC/**`: 64 more `.pck` files, 2,576,049,228 bytes, 184 banks,
60,535 streaming sounds, languages english(us), french(france), german, italian, japanese, spanish(spain), sfx
(not mapped further; the slice does not need them).

### Container layout (fitted, with oracles)

`AKPK` header: `u32 header_size` (counted after the first 8 bytes), `u32 version` (=1), sizes of the language map,
bank table, streaming-sound table and an externals table (always the 4-byte empty count here). Language map: count,
`(offset, id)` pairs, UTF-16 names. Each table: `u32 count` then 20-byte entries
`{id, block_size(=1), size, start, language id}`. A bank is a Wwise bank (`BKHD`, `DIDX`, `DATA`, `HIRC`, `STID`;
bank version 62). `HIRC` is `u32 count` then `{u8 type, u32 size, body}` with `u32 id` first in the body.

Oracle results on the real install (`census`, all from `local/audio/census.json`):

- all 4 `.pck` (and all 64 DLC `.pck`): header size consistent, table sizes equal `4 + 20 * count`, **entries tile the file
  exactly** (0 gap bytes, 0 overlaps, end = file size);
- 100 of 100 banks: chunks tile the bank exactly, `BKHD` id equals the table id, `HIRC` objects tile the chunk exactly,
  `DIDX` is whole 12-byte entries;
- `STID` bank names: 99 of 99 satisfy FNV-1(name) = bank id (one bank has no `STID`);
- 28,564 events (8,013 distinct ids): body size = `8 + 4 * action count` for 28,564; 33,011 of 33,011 actions resolve in
  their bank; 30,351 play actions, 30,341 target an object in the same bank (10 do not: UNVERIFIED why);
  678 play actions are longer than the 17-byte layout the parser assumes (target still read correctly for the slice);
- 144,568 sounds: 22,010 in-bank sources, **22,010 of 22,010 Vorbis sources found in `DIDX`**; the other 276 in-bank
  sounds all use plugin `0x650002` (no media; presumably a generator, UNVERIFIED). 122,282 streamed sources: 122,281 found in
  the streaming tables (1 prefetch-type source, id 494997802 in bank `0xdb297178`, is not: UNVERIFIED). Source ids are
  unique across all streaming tables, so language does not disambiguate them.

### Hash hypothesis: confirmed

Wwise event/bank ids are the **32-bit FNV-1 (multiply, then xor) of the lower-cased name**. Checked: `AkEvent.ShortId`
equals FNV-1(`AkEvent.WwiseName`) for all 16 events below; `AkBank.ShortId` / bank ids equal FNV-1 of `STID` names 99/99.
Both properties are plain tagged properties on the UE3 object (`ow-package --properties <idx> --property-offset 4`);
`AkEvent` also carries `MinDuration`/`MaxDuration` and optionally `RequiredBank` (none of these events set it, so the
owning bank comes from the Wwise side).

## 2. Identity chain

UE3 side uses our reader only (`--mission-run`, `--object-dump`, `--properties`). Wwise side is `tools/audio_census.py`.
`bank` = bank holding the event (from the `HIRC` event id). All sounds are Wwise Vorbis (below).

### Mission dialog (`--mission-run` effects; group `GD_VOSQ_RockPaperGeno.Groups.DialogGroups_Side_RockPaperGeno`, Startup)

Chain: effect `a` (event tag) -> `GearboxDialogGroup.DialogEvents[Tag].OutputAction` -> `WillowDialogAct_Talk.TalkData[].TalkAkEvent`
-> `AkEvent` -> bank `Startup_Voice` (`0x549aa87b`, `English(US)/Audio_Banks.pck`) -> streamed source in `English(US)/Audio_Streaming.pck`.

| Event tag (last part) | Talker name tag in TalkData | AkEvent (`Ake_VOSQ_Sidequests.`) | Source id | Header duration | AkEvent Min..Max |
|---|---|---|---|---|---|
| `01_EchoX_Marcus` | `DialogName_Marcus` | `Ak_Play_VOSQ_RockPaperGeno_01_EchoX_Marcus` | 447937631 | 17.666 s | 16.50..20.22 |
| `02_EchoX_Marcus` | Marcus | `..._02_EchoX_Marcus` | 341448344 | 5.575 s | 4.86..6.50 |
| `03b_echoX_Marcus` | Marcus | `..._03b_echoX_Marcus` | 471710802 | 6.117 s | 5.81..6.19 |
| `03a_LiveHypEng` | `DialogName_HypEngineer` | `..._03a_Live_HypEngineer` | 603398354 | 1.999 s | 1.85..2.40 |
| `04_EchoX_Marcus` | Marcus | `..._04_echoX_Marcus` | 630406443 | 4.832 s | 4.46..5.67 |
| `05_EchoX_Marcus` | Marcus | `..._05_echoX_Marcus` | 586665932 | 7.368 s | 3.80..7.37 |

Notes: the record's earlier summary said "01, 02, 05 and a Hyperion line"; the run emits six dialog effects (also 03b and 04).
Each event has two play actions (two sound objects) pointing at the **same** source: whether that is a second bus/echo
routing is UNVERIFIED. `03a_LiveHypEng` is paired with its Talk act by a rule, not a reference: the group's events with a null
`OutputAction` pair in order with its inline `TalkActs` array (7 of 7 names agree, UNVERIFIED as the engine's rule). The
trigger's name tag for that line (`GD_Dialog_Enemy_Hyperion.VOBD_HYP_Engineer.DialogName_HYP_Engineer`) differs from the group's
`DialogName_HypEngineer`: unexplained, left as observed. Priority objects (`GD_Globals.Dialog.DialogPriority_*`), `bIsEchoEvent`
and `Emote` exist on tags/acts and are not modelled.

### Marcus's own group (`GD_Dialog_NPCImplementation.Groups.DialogGroup_NPC_Marcus`, Sanctuary_Dynamic) - not triggered by the Fire steps

Mission-giver barks: `DET_NPC_OnUse_AllMissionsInProgress` -> `Ak_Play_VOCT_Marcus_Quest_During`; `..._MissionsAvailable` -> `..._Quest_New`;
`..._NoMissions` -> `..._Quest_No_New`; `DET_NPC_PlayerLingeringInMenu` -> `..._Quest_UI_Idle`. AkEvents are in `Ake_VOCT_Contextual` (in
`Sanctuary_Dynamic`); banks `Sanctuary_P_Voice` / `SanctuaryAir_P_Voice` (random containers, several sources each; selection rule UNVERIFIED).

### Door (`Sanctuary_Dynamic`, see SANCTUARY_MOVER_PROTOTYPE.md)

| Track | Key time | AkEvent (`Ake_Obj_Movers_Temp.`) | Banks holding it | Source | Header duration |
|---|---|---|---|---|---|
| `...InterpData_2.InterpGroup_0.InterpTrackAkEvent_0` | 0 s | `Ak_Play_Door_Wood_Open` | `Sanctuary_P_SFX` (`0x425d3044`) plus Sanctuary_Air/Ice/Frost | 431676253 (in bank, 8,727 B) | 1.0027 s (AkEvent 1.00268) |
| `...InterpTrackAkEvent_1` | 0.2 s | `Ak_Play_Door_Wood_Close` | `Sanctuary_P_SFX`, SanctuaryAir | 926310066 (in bank, 12,699 B) | 1.390 s (AkEvent 1.39002) |

Which key plays on open versus close, and why "Close" sits at 0.2 s, is not established (UNVERIFIED).

### Lent pistol (`MW_RockPaper_Fire` -> `Pistol_Maliwan_2_Fire` -> `Pistol_Maliwan_2_Uncommon` -> `Pistol_Maliwan` -> `WeaponType_Maliwan_Pistol`)

| Role | Source in data | AkEvent | Bank | Source(s) |
|---|---|---|---|---|
| fire | `WeaponTypeDefinition.FireSounds[0].Event` | `Ake_Wep_Pistols.Ak_Play_Wep_Pistol_Maliwan_Shot` | `Startup_SFX` (`0xf3949abc`, `Audio_Banks.pck`) | switch container, 5 distinct sources (10 sounds), 0.899..0.900 s each (AkEvent 0.8991..0.9002), all in bank |
| equip | `PickupAndEquipSounds` | `Ake_Obj_Pickup.Obj_Pickup_Equip.Ak_Play_Obj_Pickup_Equip_Pistol` | Startup_SFX | 316603693, 0.395 s |
| reload clip out | `AnimNotify_AkEvent` in `Anim_1st_Person.Pistol` sequences `Reload_Maliwan` @0.371 s and `Reload_Maliwan_Fast` @0.209 s | `Ake_Wep_Pistols.Reloads.Ak_Play_Wep_Pistol_Maliwan_Clip_Out` | Startup_SFX | 19675346, 0.197 s |
| reload clip in | same sequences @1.306 s / @0.677 s | `...Clip_In` | Startup_SFX | 429576090, 0.517 s |

The fire event's switch container has two switch values (each source appears twice); which value the engine selects (probably
state/switch set by the weapon or element) is UNVERIFIED, so the host may pick any of the 5 sources until observed. The element part
`Pistol_Elemental_Fire` has its own behavior provider (not decoded for audio); element-specific sounds (burning, impact) are not
resolved. Reload sound timing comes from notifies on the 1st-person pistol animation set, which is the asset path
UModel/animation import already has to carry.

## 3. Decode feasibility

- **Codec:** every slice entry is a RIFF `fmt ` tag `0xFFFF` with a 66-byte `fmt` chunk: **Wwise-modified Vorbis**, mono or stereo, 32/36/44.1/48 kHz.
  Plugin id `0x00040001` is the same for 144,292 of 144,568 sounds (so the whole install is this codec bar the 276 media-less ones).
- **Duration oracle (supports the header reading, not a decode):** the sample count at `fmt+0x18` divided by the sample rate lies inside
  `AkEvent.MinDuration..MaxDuration` for **16 of 16** events, and equals it to 4 digits for the single-source SFX events. This corroborates the
  header-offset hypothesis and the event -> media mapping; it does not prove the audio content is the intended line.
- **Already installed:** no ffmpeg/vgmstream/ww2ogg/revorb on PATH. Found: Krita's `ffmpeg.exe` (8.0.1, SHA-256 `A0DA0A98...3FC4`),
  Resonite's `ffmpeg.exe` (n8.1.2, SHA-256 `2CD9C24C...DDBB`), a Z_CADSKI `ffprobe` (not tried), `libsndfile` 1.2.2 via Python `soundfile`.
  Python `torchaudio` has no backend available.
- **Benchmark (slice entries only, 16 unique `.wem`, 1.1 MB):** `ffmpeg -hide_banner -loglevel error -y -i <id>.wem <id>.wav`
  (`local/audio/bench_ffmpeg.ps1`, log `local/audio/bench_ffmpeg.log`). Krita ffmpeg: 0 ok / 16 fail in 2.3 s; Resonite ffmpeg: 0 ok / 16 fail in 1.2 s;
  error "no decoder found" (FFmpeg has no demuxer-level mapping for the `0xFFFF` Wwise tag even though both builds have the generic `vorbis`
  decoder). `soundfile`: "Malformed 'fmt ' chunk". **Result: nothing already installed can decode these files.** No new tool was downloaded or run.

### Candidates for the maintainer (not installed, not run; licences from upstream pages and memory, confirm before use)

| Candidate | Upstream | Licence | Covers | Risks |
|---|---|---|---|---|
| vgmstream (`vgmstream-cli`) | https://github.com/vgmstream/vgmstream | open source (`COPYING`; commonly stated as ISC for the core, bundled codec libraries differ) | decodes Wwise `.wem` directly to WAV, Windows CLI release | prebuilt binary bundles third-party libs; needs benchmark + provenance entry |
| ww2ogg + revorb | https://github.com/hcs64/ww2ogg (BSD-3-Clause) ; revorb (separate author, licence to confirm) | BSD-3-Clause / to confirm | rewraps Wwise Vorbis to standard Ogg (needs `packed_codebooks*.bin`; the 66-byte `fmt` variant must be tried) | two tools, a codebook data file; some `.wem` revisions fail |
| FFmpeg (already installed) | - | - | **ruled out above** | - |
| Own converter | - | ours | Wwise Vorbis header repack + `libsndfile` for the Ogg | substantial code and clean-room care; not worth it for the slice |

Recommendation (mine, UNVERIFIED): benchmark vgmstream-cli on exactly the 16 files above plus a handful of others; it is the shortest path and the
duration oracle gives a ready success check (decoded length vs header duration).

## 4. Host plan

Manifest `local/slice/audio.json` (schema `ow-audio-v1`, written by `tools/audio_slice_chain.py`, 18 entries now): per entry `key`, `kind`,
`trigger`, `ak_event {ue3_path, wwise_name, short_id}`, `banks`, `media[] {source, wem, bytes, codec, channels, sample_rate, duration_s, decoded}`,
`duration_range_from_akevent`, `checks {fnv1_matches_short_id, wem_header_duration_within_akevent_range}`, `state`. States: `unresolved |
extracted_undecoded | decoded_not_listened`; **all 18 are `extracted_undecoded`** and nothing is marked verified. Once a decoder is approved, drop
`<source>.wav` into `local/audio/decoded/` (or pass `--decoded-dir`) and re-run to fill `decoded`.

Keys the host can use directly: `dialog:<event tag from --mission-run effect.a>`, `door:open` / `door:close`, `weapon:fire`, `weapon:equip`,
`weapon:reload_anim_<sequence>_at_<time>s`, `marcus_group:<event tag>`. Multiple `media` entries mean a random/switch container: pick any (rule UNVERIFIED).

Existing hook points: the dialog effect (`kind: "dialog"` in `src/mission.*` effect list; currently logged by tag) -> look up `dialog:<a>`; the door's two
`InterpTrackAkEvent` keys are listed as omitted tracks in `local/doors/mover.json` (`omitted_tracks`) with their times in the manifest; weapon fire in the
host's Maya weapon path plays `weapon:fire`. For a first pass play the file at the host actor (no attenuation, bus, RTPC, occlusion or `bVoice`
ducking); `bVoice`, `FaceFXAnimSet` (lip sync) and `Priority` exist in the data and are out of scope.

Out of scope here: decoding, UE5 `USoundWave` import, Wwise RTPC/state/switch/bus logic, music (`Ak_Play_Music_Act1_Sanctuary` etc.), ambient emitters
(27 `AkAmbientSound` in `Sanctuary_Audio`), other languages, DLC banks, subtitles/echo UI.

## UNVERIFIED list

Container fields beyond the oracles above (the externals table, `block_size` use, the 678 longer play actions, 10 unresolved play targets, 1 unresolved
prefetch source); plugin `0x650002` meaning; container child extraction (longest run of in-bank ids, used for the pistol switch container and Marcus's random
containers); which child a switch/random container selects; Vorbis sample-count offset (only duration-oracle checked); event-with-null-`OutputAction` pairing
rule; why two actions share a source; door key semantics; the HypEngineer name-tag mismatch; nothing here was decoded, auditioned or compared to the original game.
