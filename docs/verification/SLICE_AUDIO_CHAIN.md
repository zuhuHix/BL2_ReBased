# Slice audio chain: stock event -> Wwise bank -> `.wem` bytes (2026-10-01)

AI-assisted (Claude Sonnet 5.5, directed by the maintainer). Sections 1 and 2 are identification and feasibility, as of 2026-10-01; the decoder and host playback came later (sections 3 and 5). As first written: **no audio was
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

**2026-11-03: every line of the group, not one run's lines.** The table above is one `--mission-run` of the fixed step list.
At main 95e48fb the same steps emit only three dialog effects (01, 02, 03b), while the slice host reaches further lines (03a, 04 and
05; a regeneration run reported them varying between runs, two runs on 2026-11-03 both reached all three), so a manifest built from the run missed lines the host looked up (quest
check `dialog_hook_finds_every_line_and_plays_nothing`, 110/111 on a freshly regenerated machine).
`tools/audio_slice_chain.py` now resolves every `DialogEvents` entry of the mission's `MissionDialogGroup` (plus any group a
run's effect names): 35 events in this group, all with an AkEvent (28 through `OutputAction`, 7 through the inline-`TalkActs`
pairing above, whose 7 of 7 name agreement still holds), 47 manifest entries in all. Each dialog entry carries
`reached_in_scripted_run` so the scripted run's three lines stay distinguishable. The group is shared by the whole Rock, Paper,
Genocide series, so the manifest now also holds lines the Fire mission never plays; that is a superset by design.

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

### Decoder: vgmstream r2168 (maintainer decision 2026-11-07)

Provenance and licences are in `THIRD_PARTY.md`. ww2ogg with revorb and an own converter were the other options; FFmpeg was ruled out above.

Decode: `python tools/decode_slice_audio.py --vgmstream <path to vgmstream-cli.exe>` (or `OPENWILLOW_VGMSTREAM`), which runs `vgmstream-cli -i -o <source>.wav <source>.wem` per distinct source. `-i` ignores loop points: without it a
looped file would come out longer than its header says. It keeps a file only when the decoder exited 0, the WAV passes the host's parser rules, its sample count, channels and rate equal the `.wem` header's exactly, and it is not all zeros. Output goes to ignored `local/audio/decoded/`, and the script refuses a folder elsewhere in the repository.

**Benchmark (2026-11-07, zuhu PC, a few minutes of a 30-minute timebox):**

| Set | Command | Elapsed | OK / failed | In | Out |
|---|---|---|---|---|---|
| Slice manifest, 153 distinct sources (in bank and streamed, sfx and English VO) | as above | 8.5 s | 153 / 0 | 4.1 MB | 26.8 MB |
| 30 streamed sources beyond the slice, seeded sample, 15 from each base-game `Audio_Streaming.pck`; 424 s of audio; 24 to 48 kHz, mono and stereo | as above | 2.3 s | 30 / 0 | 7.0 MB | 67.0 MB |

No unsupported types or duplicates in these sets (the tool decodes each source once); the whole install was not tried. On the 153:

- every WAV's frame count equals the `.wem` header's sample count exactly, and channels and rate match; the tool now requires this. This is the first decode to confirm that header reading, previously only a duration oracle;
- none is silent, which the tool requires too, though only as "not all zeros". Observed by a one-off check, not by the tool: lowest peak 12,533 of 32,767, median 32,632. This shows signal, not the right sound;
- each holds only `fmt ` and `data`, so the WAV rules on padded chunks do not come into play for this output;
- a second decode of 10 was byte-identical;
- the CLI exits `0xC0000135` (DLL not found) with only `libvorbis.dll` beside it, or with none: the bundled DLLs stay with it. The sound path itself uses Vorbis; which other DLLs it really calls was not measured.

**Nobody has listened to any of this.** Length and signal do not show that a sound is right. Many files peak at full scale, and the host has no game mix (no bus, attenuation or ducking), so expect them to be loud. Keep the volume low at the first listen.

## 4. Host plan

Manifest `local/slice/audio.json` (schema `ow-audio-v1`, written by `tools/audio_slice_chain.py`; 49 entries on 2026-11-07): per entry `key`, `kind`,
`trigger`, `ak_event {ue3_path, wwise_name, short_id}`, `banks`, `media[] {source, wem, bytes, codec, channels, sample_rate, duration_s, decoded}`,
`duration_range_from_akevent`, `checks {fnv1_matches_short_id, wem_header_duration_within_akevent_range}`, `state`. In `checks`, `null` means not compared and `false` means a comparison failed. States: `unresolved |
extracted_undecoded | decoded_not_listened`; **all are `extracted_undecoded` that day** and nothing is marked verified. After `tools/decode_slice_audio.py` (section 3), re-run with
`local/audio/decoded/` in place (or `--decoded-dir`) to fill `decoded`; `tools/prepare_slice_audio.py` reads that folder either way.

Keys the host can use directly: `dialog:<event tag from --mission-run effect.a>`, `door:open` / `door:close`, `weapon:fire`, `weapon:equip`,
`weapon:reload_anim_<sequence>_at_<time>s`, `marcus_group:<event tag>`. Multiple `media` entries mean a random/switch container: pick any (rule UNVERIFIED).

Existing hook points: the dialog effect (`kind: "dialog"` in `src/mission.*` effect list; currently logged by tag) -> look up `dialog:<a>`; the door's two
`InterpTrackAkEvent` keys are listed as omitted tracks in `local/doors/mover.json` (`omitted_tracks`) with their times in the manifest; weapon fire in the
host's Maya weapon path plays `weapon:fire`. For a first pass play the file at the host actor (no attenuation, bus, RTPC, occlusion or `bVoice`
ducking); `bVoice`, `FaceFXAnimSet` (lip sync) and `Priority` exist in the data and are out of scope.

Out of scope here: decoding, UE5 `USoundWave` import, Wwise RTPC/state/switch/bus logic, music (`Ak_Play_Music_Act1_Sanctuary` etc.), ambient emitters
(27 `AkAmbientSound` in `Sanctuary_Audio`), other languages, DLC banks, subtitles/echo UI.

## 5. Host playback (2026-11-06, AI-assisted, Claude)

**State: the host path has run with a synthetic tone, and game sounds now decode (section 3). No game sound has been heard, by anyone.**

Path: the decoder (vgmstream, outside the repository; section 3) writes 16-bit PCM `<source>.wav` into ignored `local/audio/decoded/`. `tools/prepare_slice_audio.py` checks
each file's format and its length against the manifest's header duration, then writes ignored `local/slice/audio_play.json` (`ow-audio-play-v1`, role -> files, plus
`unresolved` with a reason per role). The host reads it with `-owaudioplay=` and plays through UE's own `USoundWaveProcedural` (`OpenWillowAudio.h/.cpp`; no new
module or dependency). Without the option nothing changes.

| Role | Hook | Sound | Evidence |
|---|---|---|---|
| `fire` | `AOpenWillowWalker::FireWeapon` | manifest `weapon:fire` | the lent pistol's `WeaponTypeDefinition.FireSounds` (section 2). The host plays it for every gun until other guns are resolved: a known gap |
| `hit` | `OpenWillowHit::OnHit` (direct hits with damage applied; at most one per 50 ms, host choice) | manifest `role:hit`, `Ak_Play_Imp_Bullets` | **UNVERIFIED**, one live session (below). A surface sound, not shown to be the enemy, and a switch container with 96 distinct sources: the host picks at random, so a hit can sound like any surface until the switch value is known |
| `phaselock_cast` | the successful lift in the Walker's Phaselock cast | manifest `role:phaselock_cast`, `Ak_Play_FX_Player_Phaselock_Activate` | **UNVERIFIED**, one live session (below): posted in the same millisecond as the skill's `PhaseLockTarget`, one cast. Its name also matches PHASELOCK_STOCK_DATA.md |
| `pickup` | `AOpenWillowInventoryPickup::TryPickup` when the item was added | none yet | candidates by name only, the `Ake_Obj_Pickup.Obj_Pickup` events; UNVERIFIED. Cash and ammo pickups take other paths and are not hooked |

The two live roles were seen in one real-game session (2026-11-07): one map (Ice_P), one target, one Phaselock cast and the save's shotgun (`SG_Jakobs_2_Uncommon`), not the lent pistol, with logging-only hooks on the game's sound functions. A call made from native code to native code passes no hook, and these hooks saw no event for the shot itself, so an absence means "not seen there". That install is modded: other SDK mods were
enabled, among them RTSE and Dialog Skipper with `Autoskip Dialog` on, which blocks dialog events from friendly NPCs, so heard lines and timing around lines from a live session are not stock. Neither role comes from a dialog event. They are not read
from a package reference: the Wwise name is the object name without `Ak_` (holds for all 47 entries read from packages on this machine), and its FNV-1 id resolves in the banks, so `checks.fnv1_matches_short_id` is `null`, not `true`, and with no AkEvent range to compare with, so is the duration check. Nothing here has been heard.

A random file is chosen when a role has several, because the container's rule is UNVERIFIED. No bus, RTPC, state, attenuation or ducking: every sound is 2D at full
volume. Dialog is not hooked; the quest suite's `dialog_hook_finds_every_line_and_plays_nothing` still describes the host.

Refused by both the host and the prepare tool, with the same rules, after walking the whole declared RIFF: a RIFF size past the file, a chunk or its pad past the RIFF size (also after
`data`), a partial chunk at its end, more than one `fmt` or `data`, partial sample frames, anything but 16-bit
PCM mono or stereo, rates outside 8..96 kHz, a file over 64 MB, and a map over 256 MB in all (host choices; a test keeps the two sides' limits equal). Without
`-owaudioplay` no delegate is bound, nothing is read, and a hook costs one flag test. When game sound arrives the flag is meant to become "a map was given or found at its
prepared path", so a prepared machine has sound by default and an unprepared one stays silent.

Test: `python tools/prepare_slice_audio.py --tone-only`, then `tools/test_audio.ps1` (`-owaudiotest`, a synthetic tone through the real path, 22 named rows; the host then exits itself).
`tools/check_audio.py` grades that run's own log: each expected row exactly once and PASS, the tone's play line, the mixer row's own numbers, a summary with matching
counts, and the editor's exit code, which must be 0 within a short wait, so a crash during teardown fails the suite. Its strongest check on sound is that the mixer pulled every queued byte (`mixer_pulled_all_bytes`). That still says nothing about what a person hears.

Runs: at 701af65 (runner1, 2026-11-06, before the exit and whole-walk checks existed) 16 of 16 rows, `mixer_pulled_all_bytes` queued=96000 left=0, exit 0. At d547e85 21 of 22, `editor=0`,
the same row failing with left=7936. Cause, read from the two logs: the row took one sample about 1.04 s of real time after the play, not the intended 1.2 s (the world timer's first step probably absorbs the frame in progress; inferred),
and the mixer starts roughly 0.12 s after the play, so the earlier pass, sampled 12 ms later, was close to luck. `Play`'s fixed-time stop had the same weakness and could cut off the end of a sound, so
it now stops once the queue has drained plus a margin, on real time, with a limit of the sound's length plus 2 s, and the row polls under that limit and logs how long it took. Nobody has reported hearing the tone. `OWAUDIO play` means
the host called play. Python: `tests/prepare_slice_audio_test.py`, `tests/check_audio_test.py` (synthetic).

## 6. Listening session (2026-11-08, AI-assisted, Claude)

**State: tooling for the maintainer to listen; the host's recording measured, nothing heard by the assistant.**

`tools/record_audio_listen.ps1` runs the slice gate (the sequence `tools/test_slice_gate.ps1` grades) with the host's sounds on (`-owaudioplay`), a quiet pause
before each step (`-owslicegatepause=<s>`, default 2 s in the driver, 0 = off in the host) and the host recording its own main output (`-owrecordaudio=<file.wav>`).
The recording is UE's own submix recorder (`UAudioMixerBlueprintLibrary::StartRecordingOutput` / `StopRecordingOutput` in the engine's `AudioMixer` module, added to
the module's private dependencies): no external recorder. Recording starts with the first game map and stops at the slice gate's summary; the gate waits for the
mixer's background writer to finish the file before it exits. Every `Play` and every gate step logs `OWAUDIOREC cue t=<s from the recording start>` and saves a frame
at most every 1.5 s. `tools/listen_report.py` then measures the WAV per cue (peak and RMS in the cue's window, how late the sound started after its cue, the quiet
gap before the next step) and writes `LISTEN.md` and `ANSWERS.md` beside it, under ignored `local/audio_listen/<date>/<time>/`. Both are game-derived (event names
come from the local manifest) and stay local.

What it can show: what the host sent to the mixer and how loud the mix was. What it cannot: whether a sound is the right one, or sounds like the game. Only the
roles in section 5 play; the guide lists everything else asked for (reload, equip, Phaselock hold and release, pickups, shield, damage, Fight For Your Life,
level-up, mission and dialog, doors, menus, footsteps) as not played. Test: `tests/listen_report_test.py` (synthetic tones and log).

## UNVERIFIED list

Container fields beyond the oracles above (the externals table, `block_size` use, the 678 longer play actions, 10 unresolved play targets, 1 unresolved
prefetch source); plugin `0x650002` meaning; container child extraction (longest run of in-bank ids, used for the pistol switch container and Marcus's random
containers); which child a switch/random container selects; Vorbis sample-count offset (only duration-oracle checked); event-with-null-`OutputAction` pairing
rule; why two actions share a source; door key semantics; the HypEngineer name-tag mismatch; nothing here was decoded, auditioned or compared to the original game.
