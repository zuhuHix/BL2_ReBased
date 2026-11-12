# Tooling reference

The working detail behind the BL2_ReBased [README](../README.md): how to
build, what each tool does, what has been checked, and what each check does and does not prove.
Everything here reads the player's own installed game and writes only under
the ignored `local/` directory.

Requirements: CMake, Visual Studio 2022 C++ build tools, Python 3. The UE5
host additionally needs Unreal Engine 5.8 installed.

## Main tools at a glance (maintainer, 2026-10-22)

Reach for these first, roughly in this order, before writing a one-off decoder. Every rule below
(outputs under ignored `local/`, nothing game-derived in git, UNVERIFIED until confirmed in the game)
still applies.

| Need | Tool | Where / how |
|---|---|---|
| A value or behaviour the running game shows | **bl-sdk (unrealsdk + pyunrealsdk) live reads and hooks**, the way the maintainer's [RTSE](https://github.com/zuhuHix/Bl2_RTSE) save editor works (`find_object`/`find_all`, hooks, a game-thread job queue) | `tools/real_game/` (`realgame.ps1`, `scripts/*.py`, `openwillow_realgame` SDK mod); see "Driving the real game" below |
| Decoding anything: native functions, binary formats, movies, runtime call tracing | **REA** ("reverse engineer anything", npm `rea-agents` 6.0.0, used through its `rea` MCP server; it drives the local Ghidra) | MCP tools `open_binary`, `procedure_pseudo_code`, `search_strings`, `observe_native_calls`, `trace_native_values`, ...; raw output only under `local/analysis/` (docs/NATIVE_ANALYSIS.md) |
| Headless executable analysis behind REA | Ghidra 12.1.4 + Temurin JDK 21 | `%USERPROFILE%\Tools\`, `tools/ghidra_import.ps1` |
| Meshes, textures, animations, sounds | UModel / UE Viewer build 1590 | "UModel / UE Viewer" below |
| Package identity, properties, scene records | `ow-package` (ours) | "`ow-package`: the package reader" below |
| UnrealScript bytecode | `research/script_disasm.py` (ours) | "Reading the game's UnrealScript" below |
| The game's object dumps as an oracle | OpenBLCMM datapack | `tools/blcmm_dumps.py`, THIRD_PARTY.md |
| BL2's Scaleform UI movies | Ruffle (desktop and web builds) | "BL2's HUD movie over UE5" below |
| Save-file layout and save-editing behaviour (SDU tables, item serials, character fields) | Gibbed's Borderlands 2 save editor, as a reference to check against | Behaviour reference only. Not installed in `%USERPROFILE%\Tools` and not yet in THIRD_PARTY.md. Record its version and licence there before relying on it, and copy no code. |

REA is recorded in THIRD_PARTY.md ("REA (Reverse Engineer Anything) MCP server": 4.1.0 on 2026-10-07, updated to 6.0.0 on
2026-10-08). Gibbed has no THIRD_PARTY.md entry yet. That is a maintainer decision and is still open (2026-10-22).

## External tools

External tools are optional local dependencies. Keep their binaries outside the
repository and write every game-derived export under the ignored `local/`
directory. Do not add a binary, an extracted asset or a generated manifest to
git.

### UModel / UE Viewer

UModel is the primary external visual inspection and extraction candidate. The
current verified local executable is build 1590 from the official
[`gildor2/UEViewer` repository](https://github.com/gildor2/UEViewer). The
[official project page](https://www.gildor.org/en/projects/umodel) documents
the exporter. It recognizes this Borderlands 2 install as the
`border` game tag and package version `832/46`. The source repository is used
as provenance; no source was copied into this project.

The official project page documents package listing and export of static and
skeletal meshes, animations, textures and sounds. It also documents important
limits: some material types are unsupported, exported material files are
heuristic, and exported data does not prove UE5 material or gameplay parity.

The current maintainer checkout is outside this repository at
`%USERPROFILE%\Documents\BL2_Tools\UEViewer-src`. Other machines must pass
their own path explicitly. The reusable wrapper is
`tools/export_with_umodel.ps1`:

```powershell
./tools/export_with_umodel.ps1 `
  -UModel 'C:/path/to/BL2_Tools/UEViewer-src/umodel.exe' `
  -Cooked 'C:/path/to/Borderlands 2/WillowGame/CookedPCConsole' `
  -Package Ash_P `
  -Object Ash_Road01
```

The wrapper writes a timestamped directory under
`local/external/umodel/`, preserves the run log and reports exit code,
elapsed time, file count and output bytes. It intentionally exports one named
package/object at a time; a whole-install run must wait for the benchmark gate.

For direct inspection, these commands were verified locally:

```powershell
$umodel = 'C:/path/to/BL2_Tools/UEViewer-src/umodel.exe'
$cooked = 'C:/path/to/Borderlands 2/WillowGame/CookedPCConsole'
$out = 'C:/path/to/BL2_ReBased/local/external/umodel-smoke'
& $umodel "-path=$cooked" '-game=border' '-list' 'Ash_P'
& $umodel "-path=$cooked" '-game=border' '-export' '-gltf' '-lods' '-dds' `
  '-nooverwrite' "-out=$out" 'Ash_P' 'Ash_Road01' 'StaticMesh'
```

After representative packages have been tested and an output-size policy is
recorded, a cautious full-install command is:

```powershell
& $umodel "-path=$cooked" '-game=border' '-export' '-gltf' '-lods' '-dds' `
  '-sounds' '-nooverwrite' '-uncook' "-out=$out" '*.upk'
```

This is deliberately not marked as a guaranteed all-asset export. The command
must be accompanied by an inventory of attempted packages, exported classes,
failures, unsupported objects, duplicates, warnings, elapsed time and output
size. See [EXTERNAL_TOOL_BENCHMARK.md](verification/EXTERNAL_TOOL_BENCHMARK.md).

### Secondary tools

| Tool | Use | Current policy |
|---|---|---|
| [UPK Explorer](https://www.nexusmods.com/site/mods/587) | UE2/UE3 GUI inspection, texture/TFC work, package exploration and optional FBX/audio workflows | Optional fallback; current distribution is on Nexus Mods and requires an authenticated download; not required for the first UModel spike |
| `ow-package` | Package identity, census, properties, bounded payloads, scene records and verification | Project-owned and retained even if UModel becomes the visual backend |
| `pyunrealsdk` and community data tools | Future runtime observation and behavioral golden data | Not an asset-extraction replacement; use for observation only, after license review |
| UE Explorer / UPKUtils | Format and behavior references | GPL-licensed references; do not copy code into this MIT project |

The external-tool acquisition and first smoke results are recorded in the
[benchmark record](verification/EXTERNAL_TOOL_BENCHMARK.md). A successful
UModel export is not independent proof that the corresponding custom decoder,
material graph or original-game behavior is correct.

## Decisions log: monthly archive

[DECISIONS.md](../DECISIONS.md) keeps the current months; older months move verbatim to `docs/decisions/YYYY-MM.md`.
When a month ends:

1. Move every `## YYYY-MM-DD:` entry of that month, byte for byte and in its original order, into
   `docs/decisions/YYYY-MM.md` under the one-line header `# Decisions and evidence, YYYY-MM (moved verbatim from DECISIONS.md)`.
2. Add the line `Older entries: [YYYY-MM](docs/decisions/YYYY-MM.md).` under the title of DECISIONS.md.
3. Run `python tools/decisions_index.py --check` before committing the rotation. Its default baseline is HEAD, which
   still holds the pre-rotation log then. The check fails if an archived month still has entries in DECISIONS.md, if an
   archive holds another month's entry, if a link line is missing, or if a baseline entry is lost or changed.

Read the log with `python tools/decisions_index.py --tail N` (full text of the last N entries), `--grep WORD` (titles),
or with no argument (every title with its file and line), rather than opening the whole file. The NATIVE_INDEX rows
for a native name or a note file: `python tools/native_index_find.py <fragment> [...] [--note NAME]`.
The synthetic tests are `tests/decisions_index_test.py` and `tests/native_index_find_test.py`.

## Leak gate

`tools/leak_gate.py` (plain Python, no dependency, no network) refuses some of what this repository must never
publish. Where it stands, as the maintainer decided on 2026-11-11: the publishing ban stays in every brief, and every
PR keeps an independent review. Only the coordinator (the hub) publishes branches, through this gate. The
`pre-push` hook can be bypassed (`--no-verify`, a clone without it), so it is a backstop, not a guarantee. The
`Leak gate` CI job runs after publication, so it detects; it does not prevent. When the gate cannot establish
something, it refuses: it never guesses and never downgrades.

```powershell
python tools/leak_gate.py --range origin/main..HEAD     # every commit a branch adds, its blobs, messages, authors
python tools/leak_gate.py --staged                      # the staged changes
python tools/leak_gate.py --all HEAD                    # the whole tree, against the baseline
python tools/leak_gate.py --text pr.md                  # free text, e.g. a PR description (- reads stdin)
```

Exit 0 is clean (warnings may be printed), 1 means at least one error, 2 means it could not run (bad arguments, no
trusted lists, a broken config file, a git failure, an object it cannot read, input it cannot parse); 2 is never
clean. With `--strict` (the coordinator's publish path), a run with no error but any warning other than
`stale-baseline` exits 3; the summary line always names the warnings' rules. The hook and CI run without it. A finding prints its rule, `path:line`, the commit it came from, and a masked excerpt (at most the first
four characters and the length; none at all for e-mail addresses and private names). A path that is itself the
finding, or that holds an identifier or a control character, is masked too; `--show-paths` prints it for local use.
Messages never quote the scanned input, an argument or a policy line.

| Rule | Level | What it refuses |
|---|---|---|
| `forbidden-type` | error | game files (packages including script packages, texture caches, audio banks, movies, UI movies, saves, `.ini`), analysis databases, Unreal assets, UModel and other mesh, skeleton and animation exports, images, decoded audio and video. A name is normalised first (trailing dots and spaces, an alternate-stream suffix) |
| `game-config` | error | a section in the game's UE3 form (`[Package.Class]`, `[IniVersion]`, a BOM or a trailing comment allowed): one in a `.ini`, two in any other text file. UE5's own `[/Script/...]` and `[Core.*]` sections pass |
| `binary`, `oversize` | error | a binary file (a NUL byte in the first 8000 bytes, or decoded text holding a NUL) or a file over 1 MiB, outside the allow-list. Messages, names and PR text with NUL bytes are `binary` too |
| `undecodable` | error | text that is neither UTF-8 (with or without a BOM) nor BOM-marked UTF-16 or UTF-32; it is still scanned with replacement characters |
| `gitlink`, `lfs-pointer` | error | a submodule entry or a Git LFS pointer: content stored outside the tree, which the gate cannot see |
| `local-path` | error | anything under `local/`, `build/` or the UE output folders (any letter case), and `Bl2Screens/` or `PromptUpgrade.md` anywhere |
| `decompiler-mark` | error | names decompilers generate (Ghidra and IDA auto-names with an address suffix, their variable and register names, their sized types and helper calls), in every text file |
| `decompiler-note-mark` | error | parameter and local names in the decompiler's numbered form and member access through `this`, in prose only (Markdown, text, HTML, commit and tag messages, PR text); `this` access inside a fenced block or an inline code span passes, since records quote the project's own code |
| `dump` | error | rows of two-digit hex bytes (`4d` or `0x4d`; 8 or more lines, 48 or more bytes, some a-f digits), hex dump rows with offsets, base85 lines, and hex or base64 text however it is wrapped or quoted: each line is reduced to a fragment (quotes, commas, semicolons, brackets, `+` and `\x` removed) and a run of hex or base64 fragments joined to 160 hex digits or 256 base64 characters is refused, as is a base64 `data:` URI of 64 or more characters (also when split after its `base64,`), escaped bytes, and a numeric table about ten times larger than anything tracked (64 lines or 512 numbers in prose, 128 lines or 1024 numbers in code and data, 512 numbers on one line), per run and summed over the file, so a table cut into chunks adds up; a baselined sum covers that exact content (a hash of the numeric lines), so a changed value, or the file removed and added again, is judged anew. Base64 runs are judged joined (wrapped encodings share one line width). SVG skips only the numeric checks |
| `email` | error | an address that is not GitHub's per-user noreply form (`<id>+<login>@users.noreply.github.com`), an exact `allowed-address` entry of the allow-list, a reserved example domain, or an image named for a pixel density (`icon@2x.png`). One function decides for file content and for the author, committer and tagger of every new commit and tag |
| `user-path` | error | an absolute path inside a user profile: a drive, UNC or Git Bash root, then `Users`, `home` or `Documents and Settings`, with one to four slashes or backslashes (escaped JSON). Placeholders, variables and URL paths pass |
| `account-id`, `credential` | error | Steam account ids; well-known token prefixes, JWTs, Slack and Discord webhook URLs and private-key blocks (encrypted ones too) |
| `private-name` | error | a token from the private names list `local/leak_gate_names.txt` (ignored, never tracked; `OPENWILLOW_LEAK_NAMES` overrides the path). Its baseline sits next to it, not in the tracked baseline. The summary line says whether a list was found; without one the rule is not checked, as in CI |
| `object-format` | error | a commit or tag header the gate does not know, an object id or type header of the wrong shape, or a signature block that is not armoured base64: what the gate cannot read is refused |
| `dump-like` | warning | a numeric table larger than any the tree holds (8 lines and 48 numbers, 24 lines, or 64 numbers on one line), or 8 or more lines of hash-length hex (a hash list): reported, refused only at the `dump` sizes. Measured once on 2026-11-11: the longest numeric run is 6 lines with 12 numbers (`docs/verification/WEAPON_VISUALS.md`), the most numbers on one line 28 (`host/ue5/import_weapon_paint.py`), 24 in `tests/` (`tests/collision_test.py`); the only file whose numeric lines add up past the error size is `research/native_by_class.json`, held by the baseline as it is |
| `unknown-type` | warning | a new file whose type the tree has never held |
| `address-hex` | warning | a hex value in prose shaped like an address in the executable; constants share that range, so it is not refused |
| `local-copy` | warning | a changed file byte-identical to a file under `local/` (64 bytes or more; copies of the repository itself under `local/` are skipped) |
| `inspection-incomplete` | warning | the `local/` copy check could not list or read part of `local/`: it says which part, it is never silent |
| `gate-change` | warning | the change edits or deletes the gate, its lists, `.githooks/`, `.github/workflows/` or a `.gitattributes` or `.gitmodules` (a deletion is named first, as `DELETES`): it needs the maintainer's yes before landing |
| `stale-baseline` | warning | (`--all`) a baseline entry whose line is gone |

The identifier rules (`email`, `user-path`, `account-id`, `credential`, `private-name`) also run over file names and
over signature bodies (which are otherwise exempt from the blob shapes, being base64 by nature). Author, committer
and tagger names and ref names get every content rule (decompiler marks and blob shapes too) and a limit of 160
characters; tag names, encodings and armour headers get the prose rules. A shallow clone or a `.git/info/grafts`
file can hide commits: then every outgoing commit must have its recorded parents present and listed as recorded,
or nothing is judged.

**How it judges a change.** Every commit that the pushed head reaches and that neither the destination's old value
nor `refs/remotes/origin/main` reaches is compared with its first parent, and every new blob in that set is scanned
whole, so content added in one commit and removed in the next is still caught. Nothing else is excluded: a graph no
trusted ref reaches (an orphan branch, say) is judged whole, whatever other remotes have cached. A merge takes
nothing from another parent as new: a file identical to another parent's is skipped, and a line counts as added
only when it occurs more often than in every parent. Added lines come from comparing the blobs, not from `git diff`,
so git attributes and diff drivers cannot hide them; a line is added when its content (whitespace aside) occurs more
often than before. Moving a line adds nothing; a second copy does, and an added line is never covered by the
baseline. A renamed file is a new path, so a baselined line in it is judged again and refused until its content is
fixed. Every commit and tag header is read (unknown ones refused), annotated tags and merged tags included, and each
ref of a push is judged on its own; the local source of a push may be a ref, `HEAD` or a raw commit id. The gate's git
calls ignore replace refs and run without system or global configuration, pager, external diff or text conversion;
the clone's own configuration still takes part. The hook's fetch of `origin/main` keeps the user's configuration,
because it needs their credentials.

**Whose gate and whose lists.** The hook runs the copy of `tools/leak_gate.py` that `origin`'s `main` holds, fetched
into `refs/remotes/origin/main` before every run, whatever the push target, and reads the allow-list and baseline
from the same commit. It refuses the push when that fetch fails or that commit holds no gate. There is no fallback to
the working tree or the branch. CI runs the pull request base's copy (for a stacked PR, the parent branch it is
based on) with that commit's lists; on a push to `main`, the copy `main` held before the push, over the range from
that commit, with the trusted-ref exclusion off (`--no-trusted-exclude`, since the checkout's `origin/main` already
sits at the pushed head). A push whose previous `main` cannot be fetched fails the job; a manual run judges every
commit with `main`'s gate; a first push judges every commit with the head's own gate, and says so. The gate itself refuses in
range, staged, pre-push and text mode when the trusted commit cannot be found or holds no lists.

**Bootstrap: landing a gate that `main` does not hold, or a gate fix the old gate cannot judge.** This is the one
way out, temporary, and only the hub runs it, after the maintainer's yes on that exact change:
1. The fix is reviewed like any PR and carries the `gate-change` warning; the maintainer says yes to it.
2. The hub pushes with `OPENWILLOW_LEAK_GATE_BOOTSTRAP=<the full id of the commit whose gate and lists judge the
   push>`, normally the fix's own head. The hook prints that id; it judges with that commit's gate and lists.
3. CI does the same by itself only while the trusted commit holds no gate at all (`WARN BOOTSTRAP` in the log); once
   a gate is on `main`, CI judges with `main`'s copy, so a fix that `main`'s gate refuses shows red until it lands,
   and the hub lands it on the maintainer's yes.
4. After the first landing of this tool, step 3 no longer happens; steps 1 and 2 stay for later gate fixes.

Two files configure it. `tools/leak_gate_allow.txt` belongs to the maintainer: `<rule> <path glob> [sha256=<hex>] --
<reason>` per line, one rule per entry, with a hash for a pinned binary (the README banner), and `allowed-address
<address> -- <reason>` for the exact addresses allowed (today the Co-Authored-By trailer and GitHub's web committer).
`tools/leak_gate_baseline.txt` lists the findings that were on `main` when the gate arrived, by rule, path and a hash
of the line, never the line; `--all` warns about entries whose line is gone. Regenerate it only for content that
already sits on `main`: `python tools/leak_gate.py --all origin/main --worktree-config --write-baseline
tools/leak_gate_baseline.txt` (private-name entries go to the ignored file next to the names list).

**On a finding:** fix the content (move game-derived output back under `local/`, write the note in your own
words, use a placeholder for a path), or ask the maintainer for an allow-list entry. Do not regenerate the baseline
to let a new finding through, and do not skip the hook.

**The hook.** `.githooks/pre-push` is installed the way `.githooks/post-checkout` is: copied into the clone's shared
hooks folder (`git rev-parse --git-path hooks`, which every worktree of the clone uses). An existing clone picks it
up by rerunning `tools/install_worktree_assets_hook.ps1`, or, without the worktree seed (on Linux, say), with
`python tools/leak_gate.py --install-hook`. Neither overwrites a `pre-push` hook that is not ours. It uses an
interpreter that lies outside the checkout (one inside is skipped, never run), runs it isolated and without the site
module (`-I -S`: no `PYTHON*` variables, no user site, no `sitecustomize` or `.pth` file of a virtual environment;
the gate needs only the standard library), and runs the trusted copy from a temporary folder outside the checkout.
It prints nothing from the push target. Its fetch of `origin/main` runs without fsmonitor but keeps the clone's own
git configuration (ssh command, credential helper): that configuration is trusted, because every push from the clone
uses it anyway, and the fetch needs the user's credentials.

**What this gate does not do.** It is partial detection of marks, file types and identifier shapes. A clean scan
establishes neither originality nor permission to publish. A frozen copy of the gate still depends on what it is
given: the trusted policy (the allow-list and baseline on `main`), the interpreter and git settings of the machine,
the private names list, and complete history; with any of those wrong, a clean result means less. The hook can be
bypassed, CI comes after publication, and other ways of publishing are outside it. It does not establish
provenance, originality or licence: code translated from a decompiler listing by hand and renamed, game
data rewritten as prose or kept under the dump sizes, GPL code with its notices removed, and personal names missing
from the private list all pass. It sees only what goes through a `git push` from a checkout with the hook, or
through CI after publication: API writes, PR text edited after the last push (the job reruns on the next push),
issue and review comments, release assets and gists are not checked. The workflow that runs it is part of the pull
request it checks, so a PR can keep the job's name and replace its commands; only a repository setting closes that
(a ruleset on `main` that requires the `Leak gate` check and the maintainer's review for `.github/workflows/`,
`.githooks/` and the gate's files), and `gate-change` is the visible half. Baseline hashes are deterministic, so a
short line can be confirmed by guessing; the lines they hash are in the public tree already. A numeric table kept
under the per-file sum, even cut into chunks, passes with at most a `dump-like` warning. The clone's own git
configuration takes part in every git call. A checkout on a drive owned by another account may be refused, since
the global `safe.directory` entries are not read.

## Build and test

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
python tests/level_test.py
python tools/verify_packages.py --reader build/Release/ow-package.exe
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

The CTest suites and `tests/level_test.py` use synthetic fixtures only and are
what CI runs. `tools/verify_packages.py` needs an installed game; for a
non-default install add
`--cooked "D:/path/Borderlands 2/WillowGame/CookedPCConsole"`. It only reads
installed packages; temporary decompressed files are removed when the check
exits.

For a decoder-free build add `-DOPENWILLOW_LZO=OFF`; that build accepts only
decompressed packages and skips the container, asset and compressed tests. The
LZO decoder is the vendored MIT-licensed lzokay; see
[THIRD_PARTY.md](../THIRD_PARTY.md).

### Playing the slice by hand (`tools/play_slice.ps1`, 2026-11-11)

One command for the vertical slice with no test driver: Sanctuary_P with Marcus's Fire mission, the gun range's enemies
(`-owrangestart`), Phaselock's data, the sounds (`-owaudioplay`), BL2's HUD movie and status pages from
`tools/hud_overlay/serve.py`, and the start shield. It refuses to start, naming each missing file and the tool that makes it,
and holds the machine-wide UE run lock (below) while the window is open. Heavy: it opens the editor in `-game`.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/play_slice.ps1 -Fresh              # play; -Level N, -Save <file>, -Port N
powershell -NoProfile -ExecutionPolicy Bypass -File tools/play_slice.ps1 -CheckSeconds 180    # unattended smoke: PASS/FAIL per item
powershell -NoProfile -ExecutionPolicy Bypass -File tools/play_slice.ps1 -PrintCommandLine    # the line only; starts nothing
powershell -NoProfile -ExecutionPolicy Bypass -File tools/play_slice.ps1 -NoRange             # no range: Maya is not moved, no enemies
```

Exit codes: 0 played or passed, 1 a smoke item failed, 2 refused (missing data), 3 never launched (lock timeout, page server
refused, editor would not start).

The smoke passes only when the log shows the mission ready, the range built, Phaselock's data, the audio play map, the start
shield, and the HUD, inventory, skills, missions and mission-screen pages ready (`-GradeLog <log>` grades a log again). The
route and what stays UNVERIFIED: [SANCTUARY_RPG_MISSION.md](verification/SANCTUARY_RPG_MISSION.md), "Hand play (updated)".
Tests: `tests/play_slice_test.py` (CTest `play-slice-launcher`; synthetic data; the launch runs a stub editor and a stand-in lock, never Unreal).

### One Unreal run per machine

Every script that launches `UnrealEditor.exe` or `UnrealEditor-Cmd.exe` (the suites, the seed and import scripts,
the shot runners, the `run_ue_*` launchers, `test_ue_viewer.ps1`) and the real-game helpers (`tools/real_game/realgame.ps1`)
hold one lock file per machine, `%LOCALAPPDATA%\OpenWillow\ue_run.lock`, from before the launch until the process has
exited; interactive launchers (`run_ue_level.ps1 -ViewOnly`, `run_ue_flash_hud.ps1`, `run_ue_character.ps1 -Play`,
`run_quest.ps1`, `play_slice.ps1`) therefore keep their PowerShell window busy until the editor is closed. All of it lives in
`tools/ue_lock.ps1`, which the scripts dot-source:

- `Enter-OwUeLock -Owner <script>` waits while any `UnrealEditor`/`UnrealEditor-Cmd` process runs and the lock is held,
  then creates the file atomically and writes its owner: the PowerShell process id, that process's start time and the
  script name. `Exit-OwUeLock` deletes it only when the caller owns it. `Invoke-OwUeLocked -Owner <script> -Body { ... }`
  does both around a block; `Invoke-OwUeEditor` starts the editor and waits for it (`& $editor` does not wait for a GUI program).
- A lock is stale only when its owner process is gone (no process with that id, or one with another start time); `Enter`
  then removes it and takes its place. The check, the removal and the atomic create happen together behind a machine-wide
  named mutex (`Global\OpenWillowUeLockGate`; if that name cannot be opened, the scripts stop with an error rather than
  use a separate gate), so two waiters cannot both replace the
  same stale lock. After creating the lock, `Enter` looks for running editors once more and gives the lock back if one
  appeared (started by a script that bypasses the lock). Age alone never makes a lock stale, and a lock without owner fields (written by
  a branch older than 2026-11-03) is never removed automatically: check that no editor or game it guarded runs, then
  delete it by hand.
- The real-game helpers (`Enter-RunLock` in `tools/real_game/realgame.ps1`) and `run_phaselock_shots.ps1`,
  `test_ambient.ps1` and `seed_ambient_npc_assets.ps1` also wait while `Borderlands2` runs, so real-game probes and
  Unreal runs never overlap.
- Nobody launches the game while a capture runs: a game started and closed between its checks goes unnoticed.
- **Bar lifted (2026-11-09):** `action_skill_hud_capture`, `hud_integrity_capture`, `hud_layout_capture`, `hud_parity_capture`, `weapon_part_session`, `projectiles_capture` and `weapon_parity2_capture` are no longer barred. The first five rest on run Z41 on zuhu, which ran `tools/real_game/hud_layout_capture.ps1 -Resolutions 1280x720` on main with PR #182 at commit 6da8d34. It exited 0: Sanctuary_P loaded, the HUD was read, the saves were identical after two restores, sdk_settings were restored, and the pending record was removed once the tree hash matched. The same day `tools/real_game/phaselock_integrity_capture.ps1 -SkipDown` also finished with a clean restore. Z41 exercised the shared `Complete-BackupRun` cleanup, which these five use. The two finalizers (`projectiles_capture`, `weapon_parity2_capture`) were barred because they restored before re-checking the backup; since PR #200 both restore through `Restore-VerifiedRoots`, one hash table, and a missing or changed backup is refused with the record and both locks kept. Run Z44 on zuhu (2026-11-09, PR #200 head, `hud_layout_capture.ps1 -Resolutions 1280x720`) exited 0 with saves identical after three restores, three changed files restored, the pending record removed after the tree-hash check, and no BACKUP CHANGED, MISSING or REFUSED rows. Z44 exercised the shared restore through `hud_layout_capture`, not the two finalizers of `projectiles_capture` and `weapon_parity2_capture`. Neither of those two has run against the game since the fix. Each of the seven stays UNVERIFIED for its own capture until it has run once against the game. The synthetic tests (`tests/real_game_pending_test.py`, `tests/real_game_migration_test.py`, `tests/real_game_finalizer_recheck_test.py`) still cover the shared cleanup.
- `OPENWILLOW_UE_LOCK` overrides the path; it must be absolute (a relative path would name a different lock in every
  worktree and is rejected). The lock is per Windows user; two different accounts on one machine are not serialized. Set `OPENWILLOW_UE_LOCK` to the same absolute path for both if that's ever needed.

Until 2026-11-03 the lock was `local/ue_run.lock` inside each checkout, so editors started from different worktrees ran
side by side; a branch older than that still uses the per-checkout file, so rebase it before running suites next to newer
worktrees. CTest `ue-lock-machine-wide` fails when a script that names `UnrealEditor*.exe` does not use the helper, and
checks the owner and staleness rules against temporary lock files.

### Before a suite: engine path, preflight and the suite-map lint (2026-11-12)

- **Engine path.** Every script that launches the editor and has an `-Engine` default takes it from `OPENWILLOW_UE` when it
  is set, and `C:\Program Files\Epic Games\UE_5.8` otherwise; set the variable on a PC with Unreal elsewhere. Five scripts
  still require `-Engine` with no default (`run_ue_level.ps1`, `run_ue_character.ps1`, `run_ue_flash_hud.ps1`,
  `run_ue_probe.ps1`, `import_maya_combat_anims.ps1`). CTest `py-engine_default`.
- **`tools/preflight.ps1`** looks before a run and changes nothing: it deletes no lock, stops no process and writes nothing
  but its report (it may create the lock's folder, never the lock; git runs with `--no-optional-locks`, so the
  worktree's index is not refreshed), and it reads a lock with delete sharing so the owner's release is never delayed. Each check is off until
  its parameter is passed, so a driver asks for what applies to one suite: `-Data` (a file, or a folder holding one),
  `-Generators <data>=<generator>` (data newer than the generator's last commit), `-CheckLinks` (no link under `local\`
  resolves outside the worktree), `-Port`, `-NoEditor` (the two editor names `ue_lock.ps1` always waits for, not a caller's
  extra names), `-CheckLock` (the run lock above) and `-AlsoLock <file>`, `-ReportHold <file>` (reported, never a failure),
  `-CheckBuild`, `-CheckEngine`. `-CheckBuild` judges the module DLL by file times against what it is built from (files
  under `host/ue5/OpenWillow/Source`, headers under `src`, the two CMake libraries), and `build\Release\ow-core.lib` by
  content: every CMake build, also one that rebuilds nothing, writes `ow-core.lib.inputs` next to it
  (target `ow-build-stamp` in `CMakeLists.txt`: git's entries for `src`, `third_party/lzokay` and the two `CMakeLists.txt`), and the
  preflight compares that with the entries now. It says `UNKNOWN`, never ok, when it cannot tell: no stamp yet (run
  `cmake --build build --config Release` once), a stamp from a build with uncommitted inputs, a newer `*.Build.cs`,
  `*.Target.cs` or `.uproject`. It prints one `PREFLIGHT ok|FAIL|NOTE|UNKNOWN` line per check and a summary, also when
  git or a check fails, and exits 1 when a check failed (`UNKNOWN` is not a failure), 2 for a wrong call. `-Generators`
  compares file times with committer dates, which cuts both ways: a rebase after the generator ran makes the data look
  stale, and a checkout of a commit dated before the run makes it look fresh. Uncommitted changes to the generator
  fail on their own line. CTest `py-preflight`, run under Windows PowerShell 5.1 and pwsh where each is present.

  ```powershell
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/preflight.ps1 -NoEditor -CheckLock -Port 8795 -CheckBuild -CheckEngine
  ```
- **`tools/suite_map_lint.py --map <suites.txt> [--repo <checkout>] [--commit <rev>] [--smoke-only <file>] [--switch-only <file>]`**
  checks a suite map in the hub's format with the hub helper's path matching. It refuses what `suites-due.sh` refuses (a
  byte-order mark, a bad or repeated name, a suite with no paths, a misplaced `*`, `./`, a leading `/` or a backslash, a
  suite that covers no file), and reports a path that matches no tracked file; a command whose `-File` script is not
  tracked; a file the suite's script loads that the suite does not cover, where a load is `Join-Path $PSScriptRoot '<file>'`
  or `Join-Path $repo '<file>'` (repository root; helpers followed; case ignored as PowerShell does, and a path built from a
  variable is not read); and a tracked file under `host/ue5/OpenWillow/Source/` or `src/` that only a whole-tree suite (`.`)
  covers, unless `--smoke-only` lists it. The lint does not read switches, so a load behind a switch the suite never passes
  is reported too; `--switch-only` (lines of `<suite> <path>`) accepts it. Exit 1 on any error. `--map` defaults to the
  tracked `tools/suites.txt`. CTest `py-suite_map_lint`, and `suite-map-tracked`, which lints the tracked map with
  `tools/suites_switch_only.txt` (the loads that sit behind a switch the suite never passes), so a PR that adds a tool to a
  suite script without adding it to the map fails CI.

### Heavy suites: the suite map (2026-11-12)

The in-engine suites are too slow to run on every commit, so each one runs only when something it covers changed.
`tools/suites.txt` lists them, one line per suite: `name | minutes | paths | command`, then a
`#pass <name> | <must match> | <must not match>` line read by the driver. Paths are folders (`src/`), files, suffixes
(`*.py`) or `.` (everything), split on spaces and tabs and matched case-sensitively; the `*ignore*` line lists paths
no suite depends on. The map and `tools/suites_switch_only.txt` are on `unit`'s line (its CTest entry reads them), so
editing either makes `unit` due, and `slice_gate` too, since it covers `.`. A covered path must not be a symbolic link.
The map holds no machine paths: the Unreal folder comes from `OPENWILLOW_UE` and the game from `OPENWILLOW_BL2`. Its minutes
were measured on the main desktop.

- **What is due:** `python tools/suites_due.py due <commit>` prints each suite with no recorded pass on the content it covers now (name, minutes, command), and a total. A pass is keyed by the git hash of the suite's
  covered files plus its command, so a rebase, a merge or a docs change makes nothing due, and changing a suite's paths
  (so that they cover other files) or its command makes that suite due once. `why <commit> <suite>` lists the covered
  files changed since its last pass. `due`, `record` and `why` read the map as committed in `<commit>` (a commit with no
  `tools/suites.txt` is refused unless `--map <file>` names one), and refuse to run when the helper's own
  `tools/suites_due.py` or `tools/suite_map_lint.py` differs from that commit's: run them from a checkout of the commit
  being judged. `list` prints the working-tree map. Exit 1 when a suite covers no file at that commit.
- **Run them:** `powershell -NoProfile -File tools/run_suites.ps1 -Worktree <test worktree> -Commit <40-char sha>
  -OutDir <log folder> [-Suites a,b] [-Repeat <suite>:<n>]`. It refuses a worktree that is not clean or not at the
  commit, reads the map inside that worktree, runs the due suites (or `-Suites`) one after another, gives a failed suite exactly one rerun, writes one log per
  attempt and `results.tsv`, records each pass at once (not when the checkout moved or a tracked file changed during
  the run: that suite counts as failed), and exits with the number of suites that ended `FAIL` (100 when it
  refused to start). It starts no build, kills no process and deletes no lock; build first, as the map's header says.
- **Record a pass by hand:** `python tools/suites_due.py record <commit> <suite>...`, only after a complete pass of that
  suite on that commit. Passes go to `local/hub/test-ledger.txt` in the main checkout (`HUB_DIR` overrides the folder),
  one `suite|key|commit|time` line each. The ledger is machine state and stays untracked; a line means the same on any
  machine, so lines can be copied between them. After regenerating ignored data a suite reads, delete that suite's lines.

The helper is a port of the hub's earlier `suites-due.sh` and computes the same keys, so passes recorded before the move
stay valid. CTest `py-suites_due` (synthetic repository and maps; the driver cases need Windows PowerShell).

### Landing PRs in a batch

`tools/land_batch.sh` lands several reviewed PRs with one push to `main` (Git Bash, with `gh` logged in):

```
git worktree add --detach ../BL2_ReBased-landing origin/main      # once; the script works only in this worktree
DRY=1 bash tools/land_batch.sh -w ../BL2_ReBased-landing -t <tested-commit> "<pr>:<head> <pr>:<head>"   # checks only
bash tools/land_batch.sh -w ../BL2_ReBased-landing -t <tested-commit> "<pr>:<head> <pr>:<head>"
bash tools/land_batch.sh -w ../BL2_ReBased-landing -D "<pr>:<head>"    # Markdown-only PRs, no tested tree
```

`<head>` is the full 40-character commit that was reviewed and tested; `<tested-commit>` is the integration commit
(current `main` plus exactly these PRs, in this order) that the in-engine suites have passed on, in the sense of "What
counts as passed" below. The script rebases the
listed PRs into one chain on top of `origin/main`. One atomic push moves the PR branches to their commits in the chain
(all or none); once GitHub shows the new heads, `main` fast-forwards in a second push, leased on the `main` the chain
was built on. (Moving the branches and `main` in one push makes GitHub close the PRs instead of merging them; if the
second push is refused, the PRs stay open on their rebased heads and the run can be repeated with those.) GitHub finds
each PR's head inside `main` and marks the PR Merged; the script waits for that, then deletes each landed remote branch unless it moved after the push (GitHub
retargets any open PR that was based on one). Before it pushes anything, it checks:

- each PR is open, targets `main` (stacked PRs: land the parent first), comes from a branch of this repository (not a
  fork), and its head is still the reviewed commit; this is checked again right before the push;
- each rebased PR shows the same diff as when reviewed, context lines and binary content included, ignoring only
  where the hunks now sit (an earlier PR in the batch that edits lines next to a later one's makes this stop; land them
  separately); `DECISIONS.md`, which merges as a union of entries through `tools/merge-union.attributes`, may only gain
  lines, the same ones;
- with `-t`, the chain's final tree is byte-identical to the tested commit's tree; with `-D`, every changed path is a
  regular, non-executable Markdown text file.

`git rerere` is switched on for the repository, so a conflict resolved once, for example while building the integration
branch, is resolved the same way when the batch lands; the tree check catches a recorded fix that differs from the
tested one. A conflict rerere has not seen leaves the rebase open in the landing worktree with instructions. The script
refuses to start while a rebase is in progress there. CI runs on the new `main`; PR workflows may also run once per
moved PR branch. Read the run on `main` before landing the next batch. The log is `local/hub/land_batch.log`.

**Standing approval (maintainer, 2026-11-08; "passed" amended 2026-11-11).** The coordinating assistant may land a PR
without asking first when its independent review is clean and its in-engine suites have passed on the tested commit
given to `-t` (or, for a Markdown-only PR, its review is clean), and reports what landed afterwards. These still wait
for the maintainer's explicit OK: changes to `THIRD_PARTY.md` or `LICENSE`, a new dependency or tool, `ROADMAP.md`,
history rewrites, releases, and anything public outside the repository.

**What counts as passed.** A suite has passed on the tested commit when it ran green on that commit, or ran green on
an earlier commit in which every file the suite covers is byte-identical to the tested commit's. What each suite
covers is the path list in `tools/suites.txt` at the tested commit; a suite that is not listed there has no recorded
coverage and runs on every tested commit as before. `python tools/suites_due.py due <tested-commit>` prints the
listed suites that have no such pass, and it must print none before the batch lands. Nothing else about the checks above
changes: the chain's tree must still be byte-identical to the tested commit's.

The earlier rule was every suite on every tested commit. Between 2026-11-08 and 2026-11-10 that meant rerunning
suites each time `main` moved under a PR that had already passed, roughly three hours in all (an estimate from the
hub's message timestamps), and in the cases on record none of those reruns found a failure the first run had not. The amended rule keeps a rerun whenever
a covered file changed, which is the case where one can find something. It rests on the path lists being right: a
list that is too narrow skips a run that was needed. The full regression the hub runs on `main` once per hub run is
the check on that, and a failure there means widening the list.

## `ow-package`: the package reader

A standalone x64 C++20 tool that reads version 832/46 packages:

- name/import/export tables, fully and partially LZO-compressed containers;
- object records with package-local outer paths (`--exports`, `--imports`);
- a reusable `PackageStore` that indexes installed `.upk`, `.umap` and `.u`
  files lazily and resolves negative imports across packages (`--resolve`);
- a per-class export count for one package (`--census`), driven over a whole
  installation by `tools/census.py`;
- tagged properties (`--properties`): scalars, object references, nested
  structs, common fixed-layout structs, and arrays whose element type is
  supplied by a schema file;
- resident `Texture2D` mips to PNG (`--texture`, DXT1/DXT5, inline or
  TFC-streamed, with `--mip` and `--all-mips`);
- all render LODs of a `StaticMesh` in memory and a selected LOD to OBJ
  (`--mesh`, 16/32-bit indices, all UV sets retained by the importer);
- bulk scene metadata (`--scene-records <schema>`) and bounded raw payload
  bytes (`--payload <index>`) used by the level preparer.

General class/default inheritance and runtime package streaming are not
implemented. Python is used for scene preparation, tests and as an independent
execution path for comparison; the executable itself does not require Python.

### Inspect object records and properties

```powershell
$willowPackage = "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/WillowGame.upk"
$objects = & ./build/Release/ow-package.exe $willowPackage --exports | ConvertFrom-Json
$partDefault = $objects | Where-Object name -eq "Default__WeaponPartDefinition"
& ./build/Release/ow-package.exe $willowPackage --properties $partDefault.index --property-offset 4
```

Export indices are one-based; negative references identify imports, and zero
means null. `--resolve <reference> --cooked <directory>` follows an import to
the owning package and export path. Paths from `--properties` still describe
the current package's outer chain; resolution is an explicit separate step.

The property offset is relative to the export payload and must be supplied.
Offset 4 has been observed for class defaults, material instances, weapon
parts, textures and static meshes so far. It is not a universal object-prefix
rule and the four bytes' meaning is UNVERIFIED. Wrong offsets, missing
terminators, malformed values and payload overruns fail with an error and no
partial JSON result.

Supported values: int, finite float, bool, name, string, byte/enum, object,
class and component references, and structs. `Vector`, `Vector2D`, `Rotator`,
`Guid`, `LinearColor`, `Color` and `Quat` decode as fixed fields; other struct
types decode as a nested tagged stream. Nesting is capped at 32 levels.

Array tags do not serialize their element type, so arrays need
`--array-schema <file>` with `PropertyName=ElementType` lines
(`tools/phase0-arrays.schema` covers the material and weapon-part probes).
Element types may be `IntProperty`, `FloatProperty`, `NameProperty`,
`StrProperty`, `ObjectProperty`, `ByteProperty` or `StructProperty:<Type>`.
Arrays without a schema entry keep their tag metadata with
`status: "unsupported"` and `value: null`. Resolving element types from the
class's reflection data instead of a hand-written schema is future work.

Local check on 2026-09-10: `GD_Gladiolus_Weapons.AssaultRifle.AR_Barrel_Jakobs_Sawbar`
decodes all 13 top-level tags with the probe schema, consuming 1,551 bytes and
leaving zero trailing bytes. The decoded names and values match the BLCMM
Object Explorer dump, with one generated-subobject presentation discrepancy
recorded in [DECISIONS.md](../DECISIONS.md).

### Census and asset extraction

```powershell
$game = "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2"
python tools/census.py --reader build/Release/ow-package.exe --game $game --output local/census.json
python tools/prepare_probe.py --reader build/Release/ow-package.exe --game $game
```

Both write only under `local/`, which is ignored. The census exits non-zero
if any package fails to read and lists the error per package. The probe
writes `mesh.obj`, `texture.png` and a `probe.json` manifest with SHA-256
hashes of the package and outputs.

Texture extraction decodes every available resident mip and supports only
`PF_DXT1`/`PF_DXT5`/`PF_A8R8G8B8`/`PF_G8`; payload-at-end mips and other pixel formats still fail
explicitly. Mesh extraction reads every render LOD, 16- or 32-bit indices and
all UV sets; OBJ output intentionally writes one selected LOD and its first UV
set. Source mesh data and skeletal meshes remain future work; tagged convex/box
collision hulls are covered in the walking slice below.

## Verification status of the reader

Local checks on 2026-09-10, Release build, all against the installed game:

- Five synthetic CTest suites pass (package tables, properties, runtime,
  container, assets). They cover truncation, block totals, corrupt LZO streams,
  partial compression tables, malformed tags, nesting limits, DXT pixel
  decoding, PNG CRC/zlib framing, TFC bounds, and mesh buffer bounds.
- All nine code packages decode byte-for-byte identically to the Python
  research reader and match on every export's name, class, outer, super,
  payload size and offset. Core yields 234,397 bytes and 1,621 exports;
  WillowGame yields 56,443 exports.
- `tools/census.py` reads 2,008 of 2,008 packages found under the install
  (base game plus 1,096 DLC packages; two UHD texture sidecar files are
  identified and skipped) for 4,751,329 serialized exports, including 77,958
  `Texture2D`, 40,090 `StaticMesh`, 3,911 `SkeletalMesh`, 35,098 `Material`
  and 100,601 `AnimSequence`. These count serialized copies, not unique
  assets, and have not yet been cross-checked against umodel's view.
- `tools/prepare_probe.py` extracts `Env_Ash.Mesh.Ash_Road01` (473 vertices,
  784 triangles, 2 UV sets) and `Prop_Roads.Textures.MetalRoadConcrete_Dif`
  (1024×1024 DXT1, streamed from `Textures.tfc`, 11 resident mips) from
  `Ash_P.upk`, and confirms through the material's `TextureParameterValues`
  that the texture is that mesh's `p_Diffuse`.
- The UE5.8.2 host probe rendered this mesh and texture in
  `/Game/Phase0/Phase0`; the first Phase 0 screenshot is user-verified.

These checks establish agreement with the research reader and a visually
plausible texture, not independent proof of every format field or gameplay
compatibility.

## Host engine probe (Phase 0)

`host/ue5/OpenWillow/` is a minimal UE5 C++ project whose module refuses to
start without `OPENWILLOW_BL2` pointing at an installed game.
`tools/run_ue_probe.ps1 -Engine <UE5 root> -Game <BL2 root>` builds it and
launches the editor with `host/ue5/import_probe.py`, which imports the probe
OBJ and PNG, wires the texture into a material, and places the mesh, a camera
and lights in `/Game/Phase0/Phase0`. This editor import remains a diagnostic
spike, not the eventual runtime loader.

## Material v1 and the first map loader (Phase 1)

```powershell
python tools/prepare_level.py --reader build/Release/ow-package.exe --game $game --map Ash_P
python tests/level_test.py
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game
```

The preparer follows `Ash_P`'s serialized sublevel references, extracts reusable
LOD0 mesh sections and four-channel materials, and writes `local/ash/scene.json`.
The UE5 importer creates `/Game/OpenWillow/Ash_P/Ash_P` with sublevel folders,
collection and ordinary actor transforms, section material overrides and a
spectator camera. Play controls: WASD/mouse, Q/E down/up. `_Dynamic` placements
remain static; there is no physics or script execution. Use `-ImportOnly` for a
headless editor import; that does not validate rendering or camera gestures.

Material v1 supports named diffuse/normal/specular/emissive texture parameters,
parent inheritance and named base-material defaults. It uses opaque lit shading,
linear normal/specular maps, sRGB diffuse/emissive, specular red, emissive RGB
masked by alpha, and constant roughness 0.65. This approximates UE3 shading.
Unsupported assets, graphs and interactive component owners are listed in
`scene.json` under `issues`. Terrain/BSP, skeletal meshes, lightmaps and full
material graphs are not loaded.

The imported map uses a reproducible inspection lighting rig (sun, skylight, reflection capture, exposure and
ambient-occlusion override, anchored at the start camera). It is an inspection approximation, not UE3 lighting; its
values and limits are in the [Material v1 / Ash verification](verification/MATERIAL_LEVEL_V1_VERIFICATION.md).

`--scene-records tools/level-arrays.schema` is bulk CLI metadata for this slice;
`--payload <positive export index>` exposes bounded bytes for observed collection
tails. The Ash-specific prefixes and native collection layout remain subject to
independent visual/in-game validation; see [DECISIONS.md](../DECISIONS.md).

`-ImportOnly` writes the import commandlet log to `<scene>/ue-import.log` and
fails if any material reported `Failed to compile Material`; such a material
would otherwise render as UE's default checkerboard while the graph checks
below still pass. It also reopens and verifies the saved scene: placement counts,
collection transforms, material overrides, imported section bounds against
source OBJ vertices, and material graph/color-space settings. To exercise all
four channels independently of game data, run `python tests/prepare_ue_smoke.py`
then pass `-Scene local/material-smoke -ImportOnly` to the launcher. `-SkipBuild`
uses an already compiled host. All fixture data is synthetic.

Imports also run `host/ue5/verify_uv.py` in a fresh editor process. It checks
saved LOD0 UV0 bindings at every imported vertex instance against the OBJ,
including the inverse OBJ V conversion, and verifies oriented triangle
topology against the original game index order. Results are written to
`ue-uv-verify.json`. This checks the export/import path, not material-graph UV
operations or the original game's appearance. `python tests/prepare_uv_smoke.py`
creates a separate `local/uv-smoke` square: red top-left, green top-right,
blue bottom-left and yellow bottom-right when viewed from its saved camera.

Counts and results of the first Ash import, and what remains open, are in the
[Material v1 / Ash verification](verification/MATERIAL_LEVEL_V1_VERIFICATION.md).

### Sanctuary and the free-flight viewer

The viewer uses the possessed spectator pawn as its camera after restart. Its
collision is disabled for free flight; this does not implement walking or UE3
collision parity. Open an already imported scene without another import:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild
```

Preparation defaults to a separate `local/<map-name>` directory, so preparing
`Sanctuary_P` does not overwrite the Ash manifest. To iterate on material
translation without extracting meshes again:

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game $game --scene local/sanctuary --reuse-textures --outer-shell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ImportOnly -SkipBuild
./tools/test_ue_viewer.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary
```

Use `--reuse-textures` only with the same unchanged game installation. The
refresh preserves placement data and resolves materials by both path and class.
`--outer-shell` is the opt-in hull policy described under the sky notes below;
omit it to keep the placed `_Teleported` overrides.

For the narrow Sanctuary hull placement experiment, rebuild the manifest with

```powershell
python tools/viewer.py --game $game --map Sanctuary_P --action prepare --outer-shell --matinee-first-key
```

(add `--sanctuary-geometry` when preserving the recovered terrain/BSP). This changes only
`InterpActor_29.StaticMeshComponent_393` to the observed first Matinee key;
the default serialized placement remains unchanged. See the
[hull placement note](verification/NATIVE_SKY_APPROXIMATION.md#matinee-first-key-placement-experiment)
for the explicit status and limitations.

On a machine without a discrete GPU, add `-LowEnd` to `run_ue_level.ps1`. It starts the editor or standalone viewer
with reduced rendering settings (listed in [performance](verification/PERFORMANCE.md)). The first launch recompiles
shaders for SM5 and is slow; later launches reuse the cache. The switch changes rendering only; imported content and
saved scenes are identical with or without it. To repeat the recorded sample, `test_ue_viewer.ps1 -Profile` (optionally
`-LowEnd`) runs `OpenWillow.Profile`; other machines will differ.

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -LowEnd
```
A single unnamed sample with an explicit `_Dif` texture name can supply
approximate diffuse color when there is no named diffuse parameter; this is
recorded as an inference, not reconstruction of stripped graphs or material
tint. The preparer also reads the cooked Material resource's texture reference
list after the tagged-property terminator; see
[Cooked-material verification](verification/COOKED_MATERIAL_VERIFICATION.md).

The runtime test checks pawn movement and camera following and captures three
settled views in `Saved/Screenshots/WindowsEditor`. It supplies engine movement
input, not physical keyboard/mouse gestures. The test process uses a 1 FPS
startup threshold to allow correctness inspection on slow maps; this is not a
performance pass. See
[Phase 1 viewer verification](verification/PHASE1_VIEWER_VERIFICATION.md).

The runtime test also logs a `Viewer diagnostic` record: 90 engine frame-time
samples during movement (mean and p95 milliseconds), current process physical
memory and peak process physical memory. These are short correctness-run
diagnostics, affected by startup, window state and shader work; they are not
a controlled renderer benchmark or GPU-memory measurement.

To list remaining diffuse gaps, their effective placed section uses, and the
recorded repair buckets:

```powershell
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
```

The report is read-only and follows actor overrides. It groups gaps by `gap_status_counts` and `issue_counts`; add
`--all-gaps` to print masked/translucent gaps alongside the default opaque priority list. See the
[audit improvements](verification/SKY_FALLBACK_VERIFICATION.md#audit-improvements) and the
[Sanctuary material baseline](verification/SANCTUARY_MATERIAL_BASELINE.md).

The audit also counts `surface_approximation` recipes separately. The two
inspected glacier materials use a partial primary diffuse/normal layer with
retained instance tiling on UV0; snow blend, glow and reflection remain open.
See [glacier validation and limits](verification/GLACIER_PRIMARY_LAYER.md).

The generated UE5 inspection map also receives a temporary `OpenWillow_SkyAtmosphere` actor and, unless a native sky
dome is accepted, a blue `OpenWillow_SkyFallback` shell. Both are labelled `temporary_sky_fallback` in `ue-import.json`
and `ue-verify.json` and are not visual-parity evidence; see the
[sky fallback record](verification/SKY_FALLBACK_VERIFICATION.md) (`PF_A8R8G8B8` extraction:
[record](verification/A8R8G8B8_TEXTURE.md)).

To locate a map's native sky placements and explain why each one does or does
not get a diffuse under the current policy:

```powershell
python tools/sky_census.py --reader build/Release/ow-package.exe --game "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2" --map Sanctuary_P --extract
```

The census lists every placed sky-named `StaticMesh` with its owner, transform, effective materials, parameters and
cooked textures. `--extract` writes the meshes as OBJ and the textures as PNG under `local/sky/<map>/`. It changes no
policy and interprets no stripped graph, Kismet streaming state or lighting. See the
[sky census record](verification/SKY_CENSUS.md).

Scene preparation carries the one accepted native `Prop_Skybox.Meshes.Sky_Dome` placement (Unlit effective material)
into `scene.json`; the importer places it under `NativeSkybox/` without collision or shadows. A second Sanctuary
`Sky_Dome` placement with a floor-material override is deliberately left as ordinary geometry. See the
[native skybox verification record](verification/NATIVE_SKYBOX_VERIFICATION.md).

When the dome's effective material descends from `Common_Materials.Sky.Mat_SkyTimeOfDay_Master`, the preparer also
writes a `sky_approximation` record from the named inputs that survive along the instance chain, and the importer
builds one fixed graph from it; the verifier reports `verified_sky_approximation_materials`. The stripped master graph
is not decoded and the `Time_of_Day` column reading (`/256`) is `UNVERIFIED`. Inputs, graph and omissions:
[sky approximation record](verification/NATIVE_SKY_APPROXIMATION.md).

The `Sanctuary_Outer` hull (`Prop_Skybox.Meshes.SanctuarySky` and its two antennas) is placed with masked
`_Teleported` overrides that Material v1 cannot recover, so it imports invisible by default. `--outer-shell` (on
`viewer.py --action prepare`, `prepare_level.py` and `refresh_materials.py`) drops only those overrides whose
mesh-default material resolved a diffuse, records each replacement in the actor's `outer_shell_replaced` list, and
imports the actors under `OuterShell/` without shadow casting. Which of `_Outer` and `_Land` the running game shows is
Kismet state and is not interpreted. See the
[sky approximation record](verification/NATIVE_SKY_APPROXIMATION.md).

The Hyperion station in front of the moon (`Prop_MoonBase.Mesh.MoonBase02`) has a named entry in
`INSPECTED_COLOR_FALLBACKS` in `prepare_level.py`, and the moon (`Mati_Moon`) one in
`INSPECTED_UNLIT_COLOR_MULTIPLIERS` (recorded as `unlit_color_multiplier`; `verify_level.py` checks the pair as
`verified_unlit_multiplier_materials`). The station shadow mask, crater relief and time-of-day tint stay omitted. See the
[moon base record](verification/MOON_BASE_SURFACE.md).

Observed blocking helpers are retained for source collision where recovered and
hidden from rendering: five `Common_Meshes.Blocking.Blocking_Cube` placements,
the 81 of 94 `Common_Meshes.CollisionCube` placements whose component sets
`HiddenGame`, whose owner actor sets `bHidden`, or which carry no material
other than the cube's own `Mat_Collision`, plus three oversized adjacent
`Sanctuary_P` placements (`StaticMeshActor_372`, `StaticMeshActor_690`, and
`StaticMeshCollectionActor_27` instance `SMC_544`), whose concrete-tile
overrides had made collision boxes visible across rooftops. The two actor
placements are roughly 10 x 9 x 4 m; the collection instance is about 5 m per
side. Their source collision remains enabled. Six cloud
`Blocking_Plane` placements (four `Mat_CloudLayer_Light`, two
`Mat_CloudLayer_01`) are hidden as well. The exact `Sanctuary_Outer`
`InterpActor_34` `Prop_Garbage.Meshes.BoxLrg` placement that blocked the start
view is also hidden. Two additional oversized `Sanctuary_Outer`
`Prop_Garbage.Meshes.BoxLrg` placements (`InterpActor_26` and `InterpActor_33`)
were identified in the UE viewport and hidden as visual-only blockers. `_26`
and `_33` retain their source collision. `_34` does not: its box encloses the
game's own `WillowCoopPlayerStart_0`, so it cannot block landed play (see
DECISIONS.md, 2026-09-23). The remaining 10
`CollisionCube` placements, including
the thin concrete street slabs in front of Scooter's garage, render with their
placed materials. Any other static (non-`InterpActor`) placement whose
component sets `HiddenGame` or whose owner sets `bHidden` is hidden the same
way, collision kept. Nearly all of these are `Sanctuary_Px` low-detail shells
and `_Low` sidewalk proxies that otherwise cover the street with roof atlases
(DECISIONS.md, 2026-09-23). This is a bounded visual artifact policy, not
complete collision or material parity.

Unlit Material v1 colors now feed Emissive Color when no explicit emissive
texture exists. The saved-scene verifier reports `verified_unlit_materials`.
See the [Unlit color verification](verification/UNLIT_COLOR.md) for scope
and the synthetic regression fixture.

To investigate the remaining cooked Material resource bytes before extending
serialization support:

```powershell
python tools/material_resource_census.py --reader build/Release/ow-package.exe --game "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2" --package Sanctuary_P --package Ash_P
python tests/material_resource_test.py
```

The report in `local/material-resources/material_resources.json` holds validated prefix boundaries, raw texture
indices, opaque-tail sizes, hashes and unsigned words; the words have no assigned shader semantics. Unsupported
prefixes and unrecognized tail layouts are recorded as errors and cause a nonzero exit status, without discarding the
raw observations. The observed tail structure and its limits are in the
[census record](verification/MATERIAL_RESOURCE_CENSUS.md). The report directory must be under this checkout's ignored
`local/`; these reports must never be committed.

### First collision and walking slice

Prepared scenes now include observed RB_BodySetup convex and box hulls. The
collision refresh/import and walking regression have been verified on Sanctuary.
For a scene already imported with collision, launch the placeholder character:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -Walk
```

Use WASD and mouse, with Space to jump. Omit `-Walk` for the free-flight viewer.
This uses UE5 movement defaults tuned for inspection; it does not reproduce BL2
movement physics. See [collision preparation, checks and limits](../COLLISION_WALKING_VERIFICATION.md).

Blocking volumes and recorded routes (2026-10-21): `tools/refresh_collision.py` also writes the map's blocking
volumes (`tools/collision_volumes.py`) and `volumes-runtime.json`; pass `--live-volumes` with a walking capture to
settle dynamic ones. `tools/walking_routes.py candidates` proposes stair probes, `tools/real_game/walking_capture.ps1`
walks them in the running game (takes the screen and keyboard; output under `local/realgame/walking/`),
`tools/walking_routes.py routes` writes `walk-routes.json`, and `tools/test_ue_viewer.ps1 -Route` runs
`OpenWillow.RouteWalking` and `OpenWillow.CollisionShapes`.

Sanctuary visual opt-in (2026-10-22): `tools/prepare_level.py` writes the decoded lighting, decal and diffuse-tint records
into `scene.json` for `Sanctuary_P` only (other maps get no `lighting` key, and their decals default-deny). The host applies
the round-4 visuals only when the environment variable `OPENWILLOW_SANCTUARY_OPT_IN` lists them: `grade`, `diffuse_tint`,
`xmas_bulbs`, `static_lights`, `graffiti_decals` (comma-separated; all default OFF). The 0.14 cloud-cap scale stays on. See
[SANCTUARY_SCENE_IMPORT_GAPS.md](verification/SANCTUARY_SCENE_IMPORT_GAPS.md) for the calibrations, all `UNVERIFIED`.

### Selecting an installed map

`tools/viewer.py` lists persistent packages in the base-game cooked directory.
Inside the running viewer, Tab shows an on-screen list of prepared scenes
(`local/*/scene.json`) and imported maps (`Content/OpenWillow/<Map>/`); the
digit keys open an imported one, and entries without a saved map are marked
"not imported". `OWMapList` and `OWMapOpen <n>` do the same from the console.
The list never prepares or imports; `test_ue_viewer.ps1 -Selector` runs the
automated level-switch check.
Package discovery does not imply successful preparation or rendering. Add
`--include-dlc` to discovery and preparation to include the install's `DLC`
directory. The current install has 37 base-game and 45 DLC persistent maps.

```powershell
python tools/viewer.py --game $game --list
python tools/viewer.py --game $game --include-dlc --list
python tools/viewer.py --game $game --map SouthpawFactory_P --action prepare
python tools/viewer.py --game $game --map SouthpawFactory_P --action import --engine 'C:/Program Files/Epic Games/UE_5.8'
python tools/viewer.py --game $game --map SouthpawFactory_P --action view --engine 'C:/Program Files/Epic Games/UE_5.8' --skip-build
```

Omit `--map` for an interactive numbered selection. Use `--list --json` for
machine-readable discovery. Names are matched case-insensitively; duplicate
package names fail explicitly. Preparation writes to this checkout's
`local/<map-name>/`; import and view require a matching prepared manifest.
Import performs the existing saved-scene verification, while view requires an
already imported map. `--skip-build` requires this checkout's compiled host.

DLC preparation indexes packages across the install and locates the named
texture cache. A cache alongside the source package wins; otherwise the cache
name must be unique. Ambiguous cache names fail explicitly. The manifest records
`package_scope`, which material refresh reuses. Base-only preparation retains
the existing base-game search root. DLC discovery alone is not DLC compatibility.

## What a new worktree gets

`tools/provision_worktree_assets.ps1` (run by the shared `post-checkout` hook; setup in [tools/worktree-assets.md](../tools/worktree-assets.md))
copies **missing files only** from the seed, under the main checkout's `local/worktree-seed/`, and never overwrites a file a worktree already has.
It copies `host/ue5/OpenWillow/Content/` and these `local/` folders, each from `seed\<name>\` into `local\<name>\`:

| Folder | Read by (examples, from the suites and check or run scripts) |
|---|---|
| `items`, `character`, `ui` | the weapon, character and HUD suites (`test_controls.ps1`, `run_quest.ps1`, `test_hud_minimap.ps1`, `test_mission_log.ps1`, `run_ue_flash_hud.ps1`) |
| `slice` | the largest group (21 scripts): `check_vendors.py`, `check_shield_check.py`, `play_range.ps1`, `run_quest.ps1` and the `prepare_*_test.py` tests |
| `doors` | `test_mover.ps1`, `test_quest.ps1`, `test_slice_gate.ps1`, `test_mission_page.ps1` |
| `range` | `test_range.ps1`, `play_range.ps1`, `check_range.py`, `test_slice_gate.ps1` |
| `quest` | `run_quest.ps1`, `test_quest.ps1` |
| `menus` | `test_controls.ps1`, `test_mission_log.ps1`, `test_mission_page.ps1` |
| `lootables`, `interactives` | `test_interactives.ps1`, `lootables_editor_test.py` |
| `phaselock` | `run_phaselock_shots.ps1` |
| `weapons`, `vendors`, `inventory-actions` | `test_vendors.ps1` (`inventory-actions` also the `-Items` path of GATES.md) |
| `inventory` | `test_inventory_actions.ps1` (GATES.md): the host reads `local/inventory/gear_manifest.json` (`OpenWillowWalker.cpp`); `prepare_inventory_gear.py` writes `observed_gear.json` there |
| `minimap`, `skillhud`, `ambient`, `audio`, `firing` | `test_hud_minimap.ps1`, `test_action_skill_hud.ps1`, `test_ambient.ps1`, `test_audio.ps1`, `test_weapon_firing.ps1` |

**Left out by default:**

- `sanctuary` is read by several suites (`test_controls.ps1`, `test_interactives.ps1`, `test_inventory_actions.ps1`, `run_hud_shots.ps1`, …)
  but is about 1.7 GB in the main checkout, most of it intermediate (`ue-obj`, `center-pillar-umodel`). Add it for one run with
  `-ExtraSeedFolders sanctuary`.
- `external/` (UModel exports; the full export is over 10 GB). Copy only the subfolder a task needs, by hand.
- `weapon_library` and `gestalt`: read by import and prepare tools only, by no suite script.

**Never copied, at any depth:** folders named `shots`, `screenshots`, `logs`, `captures`, `realgame`, `h4x`, `z3x` and `runs`
(per-run output). `-ExcludeFolderNames` changes that list. The file `observed_gear.json` (written by `prepare_inventory_gear.py`, read by
nothing else) is never copied either: `-ExcludeFileNames` changes that.

The default list is `tools/worktree_seed_folders.txt`, one name per line. The installer and the provisioner both read it, so
there is one list. Parameters: `-SeedFolders` replaces the default list; `-ExtraSeedFolders` adds to it; `-SeedDirectory` names another
seed (the tests use it).

**When the hook runs.** The `post-checkout` hook provisions a worktree once. It runs the provisioner when the worktree has no
marker, or when the previous HEAD is all zeros (git passes that for `git worktree add`). The provisioner writes the marker,
`openwillow-provisioned` in the worktree's own git directory, after a successful run. So a `git worktree add --no-checkout` worktree
is provisioned at its first real checkout, and a provisioned worktree skips branch switches and rebases. File checkouts
(`git checkout -- <path>`) always skip, because the provisioner scans the whole seed and each run took minutes. The marker is in the
git directory, not under `local\`, because `local\` can be a shared junctioned root. `OPENWILLOW_PROVISION=1` forces a run. To top up an existing
worktree, run it by hand: `powershell -File tools/provision_worktree_assets.ps1 -WorktreeRoot <worktree>` (still missing files only; add `-RefreshStale` to also replace a file whose source copy is newer and differs, each one printed, see [tools/worktree-assets.md](../tools/worktree-assets.md)).

**Refused, not copied.** Provisioning stops with an error when a junction or symlink sits on a destination path (`local\`, a
`local\<folder>\`, `host\ue5\OpenWillow\Content\`) or on a seed or source path, and it checks that every write lands under the
folder it was meant for. Fill such a folder by hand, or remove the junction first. A pair whose own folder name is excluded
(for example a top-level `local\logs` under `-SourceWorktree`) is skipped and reported as `skipped (excluded name)`.

**Every pair is checked before the first copy,** so a refusal leaves all destinations untouched. A worktree whose `local\` is itself
a junction (a shared data root, as on the pc-hummer runners) is the one intended exception: its local pairs are skipped with one
line, `local\ is a junction (shared data root); not provisioning local`, Content is still provisioned, and the exit code is 0. Any
other junction stays a refusal. The installer validates the seed path (`local\` and `local\worktree-seed`) before it writes anything,
and refuses a junction there.

Scope: paths are checked lexically from the repository root down. Ancestors above it, 8.3 aliases, and a junction created during the
copy are out of scope; provisioning is a local setup step, not a security boundary.

**Upgrading an existing install.** A seed keeps the copy of the provisioner that the installer wrote, so a change to
`tools/provision_worktree_assets.ps1` or `tools/worktree_seed_folders.txt` reaches new worktrees only after the installer runs again.
From the main checkout (or any populated worktree):

```powershell
powershell -File tools/install_worktree_assets_hook.ps1
```

It refreshes the seed's copy of the provisioner and list, fills the seed folders (missing files only), and reinstalls the hook.
A seed without the list, from before this change, makes the hook fail with a message that names this step.

**A failing hook still leaves the checkout.** `git worktree add` (and `git checkout`) runs the `post-checkout` hook after the
checkout has already happened. When provisioning fails, the new worktree exists, but Git returns a nonzero exit code, so a script
that checks it sees a failure. Look at the hook's messages, fix the seed, then run the provisioner by hand:
`powershell -File tools/provision_worktree_assets.ps1 -WorktreeRoot <worktree>`.
A folder missing from the seed is reported as `(not in the seed)`. `-SourceWorktree` keeps its old behaviour (every `local/` folder of
a populated worktree) but skips the excluded names too.

Sizes: this list is what the suites read; in the main checkout today only `sanctuary` (1.7 GB), `character` (2.2 MB), `external` (4 MB here)
and `slice` (one 0.5 MB file) are present, so the other folders could not be measured here. The seed's own folders are not measured here either;
measure them after the hub fills it (`du` per folder, the same list as the table).

## Regenerating local data on a fresh machine

Everything under `local/` and `host/ue5/OpenWillow/Content/` is generated from the installed game and is not in Git. A
from-scratch regeneration on a second PC (2026-11-02/03, main 683a083) went through the order below; each tool's own
section in this file has its options. Run the non-UE steps in any order their inputs allow; run UE steps one at a time
(they wait for the machine-wide run lock, `tools/ue_lock.ps1`, and for no running editor), and only after the UE module has been built
(`host/ue5/OpenWillow/Binaries/Win64/UnrealEditor-OpenWillow.dll`).

1. Build the core (`cmake -S . -B build -T version=14.50`, then `cmake --build build --config Release`) and run
   `tools/verify_packages.py`. Pass the reader to the Python tools as an absolute path: on Windows a relative
   `build/Release/...` path with forward slashes is not found when a tool starts it as a subprocess.
2. Sanctuary scene: `tools/viewer.py --action prepare --sanctuary-geometry --outer-shell`, then
   `tools/refresh_collision.py`; the door mover (`tools/prepare_mover.py`).
3. Slice data: `tools/prepare_slice_world.py` (21 oracles; it also runs `slice_values.py`), `tools/prepare_action_skill.py`,
   `tools/audio_census.py census` then `tools/audio_slice_chain.py`, the gestalt tables and UModel gestalt glTF,
   `tools/seed_inventory_demo.py`, `tools/weapon_slice_gear.py`, the export index and `tools/slice_npc_assets.py all`,
   `tools/weapon_view_model.py`.
4. UModel exports for Maya (glTF body, head and hands, PNG textures; MD5 hands and first-person AnimSets).
5. UI: the font library first, then the movies, the harnesses, `tools/prepare_inventory_movie.py` on
   `SharedWillowInventory`, the skill tree, HUD icons, minimap and the Ruffle web build (section "BL2's HUD movie over UE5").
6. Weapon part library inputs (part census, `export_weapon_skins.py`, library meshes, skins, index) and the firing
   coverage recipes.
7. UE imports, one editor at a time: Sanctuary (`run_ue_level.ps1 -ImportOnly`), Maya and her arm and combat clips, then
   `seed_slice_player_assets.ps1 -Steps fx` **before** `seed_inventory_assets.ps1` (the weapon items need the Infinity
   material that step creates), the slice NPCs, the Phaselock effects (`research/particle_system.py`, the UModel FX
   exports, `import_phaselock_fx.ps1`, `tools/prepare_phaselock_fx.py`, then the import again), the slice paint and the
   weapon library import.
8. Weapon previews (`tools/render_weapon_previews.py`), then the worktree seed (`tools/install_worktree_assets_hook.ps1`).
9. Suites: quest, weapon firing, inventory actions, action-skill HUD, minimap.

Machine notes: scripts that default to `C:\Program Files\Epic Games\UE_5.8` take `-Engine <path>` for an engine
installed elsewhere. In Git Bash, MSYS rewrites arguments that start with `/` into Windows paths, which breaks UE content
paths such as `/Game/OpenWillow/...`: set `MSYS2_ARG_CONV_EXCL=/Game` for tools that take them (PowerShell is not
affected). New worktrees get only part of this data from the seed; see [tools/worktree-assets.md](../tools/worktree-assets.md).

## Where things are

| Path | What |
|---|---|
| `src/` | `ow-core` library (package reader, LZO container, asset importers) and the `ow-package` CLI |
| `tests/` | Synthetic CTest suites and scene-preparation unit tests; no game data |
| `tools/` | Census, probe/level preparation, material refresh, UE launch scripts, array schemas |
| `host/ue5/` | Minimal UE5 C++ project, editor-Python importer/verifier, in-game map selector, viewer/walking/selector/profile automation tests |
| `third_party/lzokay/` | Vendored MIT LZO1X decoder (provenance in `THIRD_PARTY.md`) |
| `research/` | Community-demand fetch and analysis scripts (the Steam review text they fetch stays in ignored `local/`) and the original Python package reader used as a comparison oracle |
| `docs/` | Plan, research, verification records; this file |
| `local/` | Ignored. Every game-derived output lands here |

## Sanctuary artifact trace

`tools/audit_sanctuary_artifacts.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --output local/artifact-pass.json`
records IcePlate, WorldTransition and HLS effective placements plus source
properties and omitted terrain/BSP class counts. Use it after material refresh;
counts do not establish which missing floors terrain/BSP will fill. See
[the verification record](verification/SANCTUARY_ARTIFACT_PASS.md).

## Terrain floors

For a complete Sanctuary preparation, use:

```powershell
python tools/viewer.py --game $game --map Sanctuary_P --action prepare --sanctuary-geometry --outer-shell
```

This runs static-mesh preparation, terrain preparation, then BSP preparation
with triangle collision, stopping on a failed stage. It is scoped to Sanctuary.
Import the resulting scene normally. Native terrain blending, the BSP texel
scale and collision flags retain the limitations below.

`tools/prepare_terrain.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision`
rewrites a prepared scene with one static mesh per TerrainComponent whose vertex/strip data corroborates the terrain
hole/diagonal flags, a labeled material approximation, and (with `--collision`) triangle-mesh collision. Cooked weighted
materials drive a host weighted sum only when every weight array matches its paired PF_G8 texture (31/31 on Sanctuary;
[DECISIONS.md](../DECISIONS.md) 2026-09-22); failed verification keeps the labeled dominant-layer fallback. This does
not establish native shader or original-game visual parity. It also writes `terrain-runtime.json`;
`test_ue_viewer.ps1 -Terrain` then runs `OpenWillow.TerrainWalking`, which stands on each terrain, drops into a flagged
hole cell and walks one component seam, and logs `Terrain hole path:` records for every probe frame. A displaced or
occluded endpoint does not directly verify the original hole location. See
[the terrain handoff](verification/SANCTUARY_TERRAIN_BSP_HANDOFF.md).

## Sanctuary root BSP polygons

`tools/prepare_bsp.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision`
adds the persistent-level root `Model`/`ModelComponent` polygons of a frozen Sanctuary scene as ordinary mesh sections
with native material assignments, texture coordinates projected onto each surface's own base point and texture axes
(`--uv surface_axes`, the default; `--uv planar` keeps the earlier 128 cm world-planar placeholder), and (with
`--collision`) host triangle collision. Every decode gate must pass before any file is written; volume-owned Models are
rejected structurally. The divisor that turns projected distances into repeats is not in the package, so
`--texel-scale` (default 128) is recorded in the manifest as `UNVERIFIED`; lightmaps and collision flags are kept only
as opaque hashes. The script is scoped to `Sanctuary_P` plus `Sanctuary_Land` and also writes `bsp-runtime.json`;
`test_ue_viewer.ps1 -Bsp` then runs `OpenWillow.BspWalking`. `python tests/bsp_test.py` covers the decoder on
synthetic fixtures. See [the BSP record](verification/SANCTUARY_BSP_POLYGONS.md) and the
[texture-axis record](verification/BSP_TEXTURE_AXES.md).

Sections whose preparer could not choose a material (currently two terrains labeled `neutral_constant`) import with
the lit gray `M_OpenWillowNeutralFallback` host material and are counted as `neutral_fallback_sections` in
`ue-import.json` / `ue-verify.json` (see the BSP record above).

## Repeatable inspection viewpoints

Create `local/sanctuary/inspection-views.json` with a `views` array containing
1 to 12 objects. Each object requires `location` (world centimeters),
`rotation` (pitch, yaw, roll in degrees), and `fov` (10 to 150 degrees).
For example, a synthetic viewpoint is
`{"location":[0,0,1000],"rotation":[-20,45,0],"fov":75}`.

Run `tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Inspect`.
The host waits eight seconds at each viewpoint, asserts camera position,
rotation and FOV, and requests numbered PNGs under the project's ignored
`Saved/Screenshots/WindowsEditor/` directory. Inspect the resulting files;
the camera assertions alone do not verify appearance or original-game parity.
The saved map and its starting pose are not changed. `-Inspect` is exclusive
with the walking, terrain, BSP, selector and profile test modes.

## Maya with the Infinity and Phaselock (host prototype)

The Infinity visual keeps only chosen fragments of UModel's pistol gestalt
glTF. Decode the gestalt part ranges with a local array schema, then filter:

```powershell
# tools/gestalt-arrays.schema (tracked since 2026-10-07) holds the array element types: GestaltInfos, Parts, and the
# socket mappings, part bounds and mesh sockets that tools/gestalt_sockets.py reads
./build/Release/ow-package.exe "$game/WillowGame/CookedPCConsole/Startup.upk" --properties <GestaltDef_Pistol index> `
  --property-offset 4 --array-schema tools/gestalt-arrays.schema > local/infinity/gestaltdef.json
python tools/filter_gestalt_gltf.py --gltf <UModel GestaltDef_Pistol_GestaltSkeletalMesh.gltf> `
  --gestalt local/infinity/gestaltdef.json --output local/infinity/Infinity.gltf `
  --parts Pistol_Body_Vladof Pistol_Barrel_Vladof Pistol_Grip_Vladof Pistol_Scope_Vladof
```

To roll a gun from its balance data instead of naming parts by hand:

```powershell
python tools/weapon_recipe.py --reader build/Release/ow-package.exe --package "$game/WillowGame/CookedPCConsole/Startup.upk" `
  --balance GD_Weap_Pistol.A_Weapons_Legendary.Pistol_Vladof_5_Infinity --seed 3 --output local/items/infinity_3.json
python tools/filter_gestalt_gltf.py --gltf <gestalt glTF> --gestalt local/infinity/gestaltdef.json `
  --recipe local/items/infinity_3.json --output local/items/infinity_3.gltf
```

The recipe lists the merge chain, candidates, weights, name parts and the
rules that are still UNVERIFIED.

`roll()` now seeds `CrtRand` (the MSVC `rand()`), so the C++ generator can reproduce it; the slice tools pin
Python's own generator. The weapon parts census, the generator manifest and the C++ generator
(docs/verification/WEAPON_PARTS_CATALOG.md):

```powershell
python tools/weapon_catalog.py packages; python tools/weapon_catalog.py crosscheck; python tools/weapon_catalog.py summary
python tools/weapon_gen_manifest.py            # -> local/weapons/gen/weapon_gen_manifest.txt (base game)
python tools/weapon_gen_firing.py --game <BL2>  # -> local/weapons/gen/weapon_gen_firing.json (firing templates for rolled guns)
./build/Release/ow-weapongen.exe local/weapons/gen/weapon_gen_manifest.txt --balance <balance> --stage 30 --seed 1
python tools/weapon_gen_parity.py              # Python vs C++ on every balance, stages and seeds; report under local/
```

In the host, `-owweapongen=<manifest>` overrides the default path, `ow.RollGun <balance or pool> [stage] [seed]` adds a
rolled gun to the backpack and `-owweapongentest` writes a host self-check under `local/weapons/gen/host_check/`. Live
confirmation: `tools/real_game/gen_confirm_capture.ps1` then `python -I tools/real_game/gen_confirm.py stats ...`.

Import with `OPENWILLOW_PISTOL_GLTF` pointing at the filtered glTF and
`OPENWILLOW_INFINITY_TEXTURES` at UModel's PNG export of
`Mati_VladofLegendaryPistol_Infinity`, running
`host/ue5/import_infinity_proxy.py` as a `pythonscript` commandlet. The pistol
and Phaselock arm clips come from `tools/import_maya_combat_anims.ps1`.

`-owwalk -owmaya -owcombattest -owcombatshots` on `Sanctuary_P` spawns a
training dummy once Maya lands, runs a fixed aim/fire/Phaselock sequence and
writes `OWCombat_1_Idle` to `OWCombat_5_FiringWall` PNGs under
`Saved/Screenshots/WindowsEditor/`, then quits. Controls in play: LMB fire,
F Phaselock, 1 equip, 0 holster. See DECISIONS.md (2026-09-25) for what is
data-derived and what is estimated.

## BL2's HUD movie over UE5 (browser-overlay prototype)

Marcus's mission screen has its own step: `python tools/prepare_mission_screen.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"` ([MISSION_SCREEN.md](verification/MISSION_SCREEN.md)).

Convert the UI movies into `local/ui/run` first (DECISIONS.md 2026-09-26:
`tools/extract_swfmovie.py`, then `tools/gfx_to_swf.py --localization
<install>/WillowGame/Localization/INT --inline-font-imports`, font library
first, and `tools/hud_harness_swf.py`), with a Ruffle web build in
`local/ui/run/ruffle`. Convert the font library into `local/ui/run/gfxfontlib.swf` before any movie: movies import it
both as `../gfxfontlib.swf` and as a plain `gfxfontlib.swf`, which resolves next to the movie. Since 2026-11-03
`gfx_to_swf.py --inline-font-imports` copies the library into the movie's folder itself when it is missing there (listed
under `copied_font_libraries` in the movie's `.json` report). A movie converted before the library existed keeps the
import and shows fallback fonts: convert it again. After converting `SharedWillowInventory`, run
`python tools/prepare_inventory_movie.py --input <converted SharedWillowInventory.swf> --output local/ui/run/SharedWillowInventory/SharedWillowInventory.swf`
(the Ruffle nested-import workaround, [INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md)); without
it the inventory suite fails. Then:

```powershell
./tools/run_ue_flash_hud.ps1 -Engine $engine -Game $game            # editor; press Play
./tools/run_ue_flash_hud.ps1 -Engine $engine -Game $game -GameWindow
```

A switch that drives a run and quits, such as `-owinvshots`, needs `-GameWindow`: without it the script opens the editor, no game starts, and it refuses to run.
With `-GameWindow` such a run is stopped after 20 minutes (an estimate, not measured; an hour for `-owgunshots`, whose list has any length: pass `-TimeoutSeconds` for a long list): the script stops the editor and any page server it started, by process id after checking the
command line, and fails with a nonzero exit; a game that exits with a nonzero code (a crash) gets the same. After a clean exit a server stays up for the next run. `-TimeoutSeconds N` sets the limit and `-TimeoutSeconds 0` removes it. Someone playing with `-GameWindow` and no
such switch gets no limit, and without `-GameWindow` there is never one. The switches are a list in `tools/run_limit.ps1`, so a new driver has to be added there.

Tests, with no editor: `python tests/run_limit_test.py` (the rules) and `python tests/run_ue_flash_hud_test.py` (the real launcher, a stand-in editor and the real `serve.py`: stall, crash, clean exit, a server it did not start, the lock released each time). With a real editor running nothing can be tested: it prints `NOT RUN: an editor is running` and exits nonzero unless `--allow-skip` is given.

The script starts `tools/hud_overlay/serve.py` on port 8767 if it is not
running and launches Sanctuary as Maya with `-owflashhud=<url>` and
`-owflashskills=<url>`. The HUD overlay logs "OpenWillow Flash HUD page loaded"
and "HUD movie ready" in the UE log.
Since 2026-10-22 the movie is the default gameplay HUD: without `-owflashhud` the host opens
`http://127.0.0.1:8767/` itself (so a running `serve.py` on that port is enough) and draws the canvas fallback until the
page reports ready (echoing the per-run id the host put in its URL) or if it fails to load; `-owcanvashud` forces the fallback (the scripted test and capture runs pass it). The log says `OpenWillow HUD path:
movie|canvas (<reason>)` and, from the page, `OpenWillow HUD 3D path warp|affine: <why>` (the perspective warp of the
turned clusters; add `?hud3d=affine` to the page URL for the 2D stand-in). Gameplay-HUD captures for a parity review:
`./tools/run_hud_shots.ps1 -Game $game [-Level 36 -Shield] [-Affine]` (slice weapons, at rest and after three shots,
checked by `tools/check_hud_shots.py`; output under `local/hudshots/`); real-game reads of the HUD texts, crosshair and
ammo icon per weapon: `tools/real_game/hud_parity_capture.ps1 -Character <your save file>` (PowerShell 7). See `docs/verification/HUD_PARITY.md`.
It is a prototype: see DECISIONS.md for what is driven and what is not.
Maya's action-skill indicator (the icon's cooldown veil and the Siren emblem while Phaselock holds) needs the icon
movie: `python tools/prepare_hud_icons.py --reader build/Release/ow-package.exe --game $game`. Its in-engine check,
`./tools/test_action_skill_hud.ps1 -Game $game [-SharedLock <file>]`, casts once and checks the logged state against
the real game's timings; real-game capture: `tools/real_game/action_skill_hud_capture.ps1`, layout read:
`tools/real_game/hud_layout_capture.ps1` (`docs/verification/ACTION_SKILL_HUD.md`).
The minimap (`p1.minimapEcho.minimap`, `docs/verification/HUD_MINIMAP.md`) needs the level's map movie and an icon
manifest: `python tools/prepare_minimap.py --reader build/Release/ow-package.exe --game $game` (writes
`local/ui/run/UI_TacticalMap_Sanctuary/`, `SharedWillowTacMaps/` and `local/slice/minimap.json`; the host reads the
manifest by default or from `-owminimap=<json>`). Its in-engine check, `./tools/test_hud_minimap.ps1 -Game $game
[-SharedLock <file>]`, puts Maya at eight real-game capture spots; live reads: `tools/real_game/scripts/minimap.py`.
The BL2-style StatusMenu Inventory movie is enabled by default; press **Tab** (or **I**) for Inventory and **K** for
Skills; the header tabs and the K/I keys switch between the two pages. Pass `-NoInventoryMovie` only to use the
fallback host inventory panel. Equip, transfer and comparison flows follow the observed stock behaviour: see
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md) (2026-09-30 and 2026-10-04 sections). Weapon
cells use static mesh previews generated from the locally exported UModel glTF meshes; if those ignored previews are
absent, run `python tools/render_weapon_previews.py` to create them under `local/ui/run/previews/`.

**F** opens the native UE weapon mesh preview (hold the left mouse button and drag to orbit it; **F** or **Escape**
returns to inventory). It uses the already imported `Weapons/Items/SK_<recipe ID>` asset, and gear meshes that cannot be
resolved show unavailable. Design and limits: [DECISIONS.md](../DECISIONS.md) 2026-09-30 "Stock inventory target,
camera-space framing and native item inspection".

Existing UI Trace SDK observations can populate local gear cards without
guessing rolled parts or package identity:

```powershell
python tools/prepare_inventory_gear.py local/ui/traces/<trace>.jsonl --output local/inventory/observed_gear.json
python tests/inventory_gear_test.py
./tools/test_inventory_actions.ps1 -Port 8791 -Extra @('-owinventoryshots','-NoLiveCoding')
```

The exporter does not equip observations or merge them into a live manifest.
Review the output, preserve stable IDs and copy only reviewed entries into
`local/inventory/gear_manifest.json`. `funStatsMarkup` preserves observed Flash
TextField formatting; the adapter passes it only to the movie, never browser
HTML. `packageResolved` and `visualIdentityResolved` remain false. The capture
flag writes settled open/equip/inspect/close screenshots to the ignored UE
Saved/Screenshots directory. Drag/drop tests dispatch DOM events; key tests
use Slate, so physical mouse/keyboard verification remains separate.

Maya's inventory preview plays the game's own third-person `Idle_Inventory`
clip and draws an ink outline. To build them in a fresh worktree:

```powershell
# 1. UModel 1590 MD5 export of the third-person body and its AnimSets
umodel.exe -path=<CookedPCConsole> -game=border -export -md5 -out=local/external/umodel/maya-body-anims GD_Siren_Streaming_SF Skel_SirenBody SkeletalMesh
umodel.exe -path=<CookedPCConsole> -game=border -export -md5 -out=local/external/umodel/maya-body-anims GD_Siren_Streaming_SF Base_Siren AnimSet
# 2. Body reference pose, then conversion (--anchor none: no first-person camera correction; needs numpy)
$env:OPENWILLOW_CHARACTER_MESH='Skel_SirenBody'; $env:OPENWILLOW_CHARACTER_ANIM_FOLDER='ThirdPerson'
$env:OPENWILLOW_CHARACTER_REFERENCE='local/character/anim/body_ref_pose.json'; $env:OPENWILLOW_CHARACTER_ANIMS=''
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
python tools/prepare_character_anims.py --anchor none --mesh <...>/Skel_SirenBody.md5mesh --reference local/character/anim/body_ref_pose.json --animset <...>/AnimSet/Base_Siren --clips Idle_Inventory Idle_var1 --output local/character/anim/siren_body.json
# 3. Import the clips, then the outline material and matte character material
$env:OPENWILLOW_CHARACTER_REFERENCE=''; $env:OPENWILLOW_CHARACTER_ANIMS='Body=local/character/anim/siren_body.json'
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
# Optional default head colour correction for the inventory display only.
# UModel export includes the local property dump used by the look script.
umodel.exe -path=<CookedPCConsole> -game=border -export -png -out=local/external/umodel/maya-menu-head CD_Siren_Skin_Default_SF Mati_Default_Head MaterialInstanceConstant
$env:OPENWILLOW_MENU_HEAD_PROPS="$pwd/local/external/umodel/maya-menu-head/CD_Siren_Skin_Default_SF/MaterialInstanceConstant/Mati_Default_Head.props.txt"
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_menu_look.py -unattended -nullrhi
# 4. Armed inventory pose, from the already exported Rifle_Siren AnimSet
# Use the Python environment with numpy installed. Paths passed to the UE
# commandlet must be absolute because its working directory differs.
python tools/prepare_character_anims.py --anchor none --mesh <...>/Skel_SirenBody.md5mesh --reference local/character/anim/body_ref_pose.json --animset <...>/AnimSet/Rifle_Siren --clips Idle_Inventory --output local/character/anim/siren_body_inventory_rifle.json
$env:OPENWILLOW_CHARACTER_ANIMS="InventoryRifle=$pwd/local/character/anim/siren_body_inventory_rifle.json"
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
```

Framing, clip and outline width are `Config=Game` properties of `AOpenWillowInventoryMayaDisplay`
(`[/Script/OpenWillow.OpenWillowInventoryMayaDisplay]` in `DefaultGame.ini`). Without the clip Maya shows in bind pose;
without the outline material she has no ink line. How the selected weapon is attached and posed, and what is still
unverified, is in [INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md) ("Selected weapon on Maya's
display copy").

The same menu-look script creates `M_OW_MenuBackdrop`, the world dimming that excludes Maya's stencil-247 pixels
(renderer setting `r.CustomDepth=3`; gain, saturation and vignette are configurable on the display actor). Without this
material, world dimming is disabled and the host logs a warning. No new extraction is needed; regenerate this local
material after updating an existing worktree. Design: [DECISIONS.md](../DECISIONS.md) 2026-09-30 "Inventory backdrop
grading excludes Maya".

For Maya's Skills tab, convert `UI_StatusMenu.StatusMenu` and its shared imports
into `local/ui/run` with the same converter and a StatusMenu harness, then
prepare Maya's tree and icon movies from the installed packages:

```powershell
python tools/game_colours.py --reader build/Release/ow-package.exe --package "$game\WillowGame\CookedPCConsole\Startup.upk" --seed <private notes>
python tools/prepare_skill_tree.py --reader build/Release/ow-package.exe --game $game --output local/ui/run
python tools/hud_overlay/serve.py --movies local/ui/run
```

The game's UI colours are not in the repository (DECISIONS.md 2026-11-08): `tools/game_colours.py` writes the
ignored `local/ui/colours/game_colours.json` (rarity tiers from the install, the measured keys from a private
seed, or receive the file with `tools/private_sync.ps1 -Direction Pull`). Without it the pages and tools use
neutral greys and say so once. Run it before `prepare_skill_tree.py`, which bakes the skill-tag colour in.

Open `http://127.0.0.1:8767/skills.html?points=41&action=1` for a standalone visual
check. In the UE game window, press **K** to open Maya's Skills tab, hover a
skill for its description, use the arrows to rotate branches, and press
**Esc** or the movie's close button to return to play. The standalone `points`
query is only a display check; the page never changes grades itself.

In UE the host owns Maya's level, XP and grades (`UOpenWillowSkills`). The host earns no XP yet, so pass
`-Extra @('-owlevel=45')` to start with points (one per level from level 5; the default level 1 has none). Phaselock
(**F**) works only after its point is spent in Skills; `-owcombattest` starts at level 5 or more with Phaselock trained.
Clicking a skill or the action skill logs `OWSKILL {"branch":B,"tier":T,"cell":C}` (`-1,-1,-1` for the action
skill); the host validates it and answers with `owSkills({points, actionGrade, grades})`, and logs each decision as
"OpenWillow Skills spend (B,T,C) accepted" or "refused: <reason>". Grades live on the walker (no save file yet) and
their effects are not applied. The host reads `local/ui/run/skilltree_siren.json` (`-owskilltree=<file>` overrides). The
spend rules are in [DECISIONS.md](../DECISIONS.md) 2026-09-27 "Host-owned skill points for Maya"; the info box and
footer (built from the install by `prepare_skill_tree.py` via `tools/skill_stats.py` and arranged by
`tools/hud_overlay/skill_info.js`) are in 2026-09-27 "Skill info box, footer and progress band from install data".
Synthetic tests: `python tests/skill_stats_test.py` and `node tests/skill_info_test.js`. `OpenWillow.Skills` is a UE
automation test of the spend rules on a synthetic tree; run it in `-game` mode (see `tools/test_ue_viewer.ps1` for the
pattern).

For a repeatable UE capture, launch the same script with
`-GameWindow -Extra @('-owcombattest','-owcombatshots','-owskillshots')`. It writes the `OWCombat_*` PNGs (including
`OWCombat_8_Skills.png` and `OWCombat_9_AfterSkills.png`) under UE `Saved/Screenshots` before quitting. `-owcombatshots`
is test-only: it raises Maya's level, fills the weapon slots and sets demo currency and ammo (made-up numbers). Other
switches: `-owslots=<2..4>`, `-owmoney=<n>`, `-owerid=<n>`, `-owinventoryselftest` (one PASS or FAILED line per check, then the `passed`/`FAILED` summary and a quit, unless another driver such as `-owinventoryactions` is on the command line); **R** reloads in play.
`-owinvshots` is a separate capture that drives the inventory page with the stock keys and writes `OWCombat_D*.png`. What
each switch sets up, the page's `OWINVTIME` lines and the steps of `tools/test_inventory_actions.ps1` are described in
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md) (2026-09-29 and 2026-10-04 sections).

## Tracing the real game's UI code (golden files for menus)

With the community mod SDK installed in the game (THIRD_PARTY.md), copy
`tools/sdk_trace/openwillow_uitrace/` into `<game>/sdk_mods/` and put the
absolute path of the repository's `local/ui/traces` in a `trace_dir.txt` next
to its `__init__.py`. In game, enable "OpenWillow UI Trace" in the mod menu,
use the menus, then disable it to close the file. Then:

```powershell
python tools/sdk_trace/summarize.py                       # newest trace
python tools/sdk_trace/summarize.py --timeline StatusMenuExGFxMovie
```

Traces are game data and stay under `local/`. See DECISIONS.md 2026-09-26.

Tracer 0.2.1 adds object identity to return records and `out.D` to
`GFxUI.GFxObject:GetDisplayInfo` returns; earlier traces omit the getter's completed output, so do not use
zero-filled inputs as proof of a flat transform (see "Getter output observation fix" in
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md)). Synthetic callback checks:
`python tests/ui_trace_returns_test.py`.

## Driving the real game (ground-truth captures)

`tools/real_game/realgame.ps1` (dot-source it) launches the installed game for captures and talks to it through
`tools/real_game/openwillow_realgame/`, our own Library mod for the community mod SDK (THIRD_PARTY.md). The mod runs
`*.py` command files on the game thread and writes `.out` replies; command scripts share one namespace. Needs
`$env:OPENWILLOW_BL2`. Everything it writes goes under ignored `local/realgame/`.

```powershell
. tools/real_game/realgame.ps1
Enter-RunLock; Backup-Saves; Install-Driver          # lock shared with UE runs; copy saves first
Start-Game @('-windowed','-ResX=1280','-ResY=720','-nostartupmovies')
Invoke-GamePy 'print(block_saves(), get_pc())'       # at the main menu, before loading a character
Invoke-GamePyFile tools/real_game/scripts/weapon_cards.py cards   # helpers: spawn, record, card trace
Invoke-Burst 'phaselock/cast' 7 40 { Send-Key F } 0.75            # QPC-stamped frames around a key press
Remove-Driver; Exit-RunLock
python tools/real_game/golden_cards.py               # join records, card trace and screenshots
python tools/real_game/golden_card_compare.py        # evaluator on the exact rolled parts vs the cards
Invoke-GamePyFile tools/real_game/scripts/weapon_dump.py cards     # every weapon part/type/name part value, live
python tools/real_game/golden_card_compare.py --live-data local/realgame/cards/live_weapon_data.json ...  # with the overlay
```

Input is scan-code keys (`Send-Key`, arrows included), `Send-ClickAt`, `Send-Wheel` and `Send-Drag`; it takes the
screen and keyboard, so only run it when nobody is using the machine. Rules learned the hard way: look objects up
again in every command (a stale weapon reference crashed the game), keep spawned items in memory and remove them
before any travel or quit, and compare the save folder with the backup afterwards. `scripts/phaselock.py` samples the
lift skill every frame and marks `StartActionSkill` and the weapon's reload/put-down calls. Results:
`docs/verification/REALGAME_GROUND_TRUTH.md`, DECISIONS 2026-10-02.
`scripts/ambient_npcs.py` (`Invoke-GamePyFile ... ambient_npcs.py ambient`) lists live pawns and dens (`amb_pawns`,
`amb_dens`, `amb_live`), samples positions over time (`amb_sample`) and places the player (`amb_goto`) for the
Sanctuary ambient NPC record. `amb_compose` reads each live citizen's materials and attached meshes (hair, hats, gear);
`amb_view_*`, `amb_cam` and `amb_aim_at` frame a pawn.
`scripts/scene_owners.py` (`Invoke-GamePyFile ... scene_owners.py lanea`, then `so_dump('owners', [materials])`) lists the
live components of interactive objects, mission spawners and wiring actors with their hidden flags and poses; `so_view` and
`so_camera` place the player and read the exact view (`GetPlayerViewPoint`, FOV) for a matched host inspection viewpoint.

### Pending records and guarded input (every real-game script)

Before a run changes anything the maintainer owns, it writes a **pending record** under `%LOCALAPPDATA%\OpenWillow\`
(`tools/real_game/pending.ps1`, `realgame_run_pending-<script>-<pid>.json`): the script, its PID and process start time
(local and UTC), the backup folder, each root's live folder and tree hash (one SHA-256 over every file's path and SHA-256,
paths compared case-sensitively), the driver folder's owner id, the keys and buttons held right now, and one line per change, each written before that change: the driver's install and save
block, the launch, SDK mods off (`Set-SdkModsOff`). The roots are every folder a launch can rewrite: `SaveData`, `Config`,
the SDK's `sdk_mods/settings` and Sanity Saver's `sdk_mods/SanitySaver` (absent ones are recorded as absent, and one that
appears during the run keeps the record). `Backup-Saves` and `Save-VerifiedBackupRecord` write it;
`Install-Driver`, `Start-Game` and `Set-SdkModsOff` refuse without it. The record goes only when every root is the backup
again by tree hash, the backup itself is unchanged, no game runs and the driver folder is gone (`Complete-PendingRecord`, or
`Complete-BackupRun` in the shorter captures). Otherwise it stays, with both locks.

The two capture finalizers (`projectiles_capture`, `weapon_parity2_capture`) restore through `Restore-VerifiedRoots` too: before each root is
restored, its backup is compared with the file hashes the run took when that root verified. A backup changed since is not restored (row
`BACKUP CHANGED after verification`), the saves line reads NOT IDENTICAL, and the record and both locks stay. The manual recovery printed for
an unconfirmed game exit names `Restore-PendingRecord`, which makes the same check. Synthetic test: `tests/real_game_finalizer_recheck_test.py`.

A verified root whose backup folder is missing counts as changed, never as empty (`Get-TreeHashes` reads a missing folder as an empty one):
`Restore-Tree` refuses a missing backup before it touches the live folder, `Restore-VerifiedRoots` reports `BACKUP CHANGED / MISSING` and
restores nothing for that root, and the pending-record check keeps the record. The two finalizers hash each staged backup once and use that one
table both for the comparison with the live tree and as the trusted baseline. `Restore-PendingRecord` does not trust `verified_hashes.json` alone:
a root is refused (row `REFUSED`, record kept) unless the pending record names it with the same live folder and the manifest's snapshot gives
the record's tree hash for it. Accepted risk: between the re-check of a backup and the copy from it there is still a short window in which a
change would be restored; it needs another process writing our ignored backup folder during a run, and no staged copy guards against it.

The driver folder is always exclusive: `Install-Driver` refuses a folder that exists unless the caller claimed it for this
run (`-ClaimedBy <id>`, an `.owrun` file and nothing else), and writes an owner id into the folder's `.owrun` and the record.
`Remove-Driver`, and any recovery, removes only a folder whose `.owrun` matches that id exactly and that still holds only the
driver's files.

Every capture starts with `Assert-NoPendingRecords`, before any lock: any `*pending*.json` there (ours, the showcase
capture's `showcase_*pending*.json`, the mission screen branch's `mission_screen_mod_pending.json`) stops it and prints the
exact steps. The writer is identified by PID and process start time, never by its command line: while that process lives, or
when it cannot be shown gone (no start time recorded, an unreadable process), nothing is completed. A record is completed
automatically only when its writer is shown gone and the roots already match the backup, or this driver's untouched copy is
the only leftover. The saves are never rolled back automatically. The way back by hand:

```powershell
$env:OPENWILLOW_BL2 = '<game folder>'; . tools/real_game/realgame.ps1
Restore-PendingRecord '<record>'   # add -WriterGone only when you know the writing run has ended and it cannot be shown
```

While a record exists, that is the only way back: `Restore-VerifiedBackup` refuses then, and the printed recovery names only
`Restore-PendingRecord`. It holds the shared run lock throughout and checks that no Borderlands 2 runs before the copy, after
it, before restoring and before removing the record (a game seen at any of them stops it, record kept). It first copies every
current root to `local/realgame/pre-restore-<time>-<pid>/` and verifies that copy by tree hash (so anything played since the
failed run is set aside, not lost), removes the driver folder only on an exact owner match, restores from the verified backup,
verifies by tree hash and only then removes the record. `Restore-VerifiedBackup`, for a backup no record names, does the same
lock, game checks and pre-restore copy. Nothing locks the game itself out: **don't start Borderlands 2 while a recovery runs.**
A game seen after the restore is reported loudly (record kept, pre-restore copy named), but what it wrote meanwhile may
already be overwritten. A record whose backup manifest lacks one of `SaveData`, `Config`, `sdk_settings`, `SanitySaver` (an
older format) is never completed automatically: `Restore-PendingRecord` restores what it covers, says which roots it cannot
vouch for and leaves `recovery_note.txt` in the backup folder.

Input is guarded: every key and button press is remembered until its release, and each capture's `finally` starts with
`Release-HeldInputs`, so an error, a timeout or Ctrl+C leaves nothing held. The held set is also written into the pending record
before each press (no record, no press). A run killed outright (Task Manager, a power cut) skips its `finally`: if the game
window is still open, a key it held can stay held in the game until the next run's start check or `Restore-PendingRecord`
sends the recorded releases (only when the writer is shown gone). The list is cleared only when no Borderlands 2 process runs at
all; a game process without a window keeps it, and the run refuses. Presses held across steps sit in their own
`try`/`finally` too. The native calls go through `Invoke-NativeKey` / `Invoke-NativeMouse` only, so the tests can replace
them. Tests (no game): `python tests/real_game_pending_test.py`. Tests that run these scripts import
`tests/realgame_test_env.py`, which gives every PowerShell run its own pending folder under the temp folder. The harness
honours that folder only in the explicit test mode (`OW_TEST_PENDING_DIR` under Temp together with
`OW_TEST_MODE=synthetic-real-game`), and in that mode only on save, game and driver folders under Temp. Either variable
alone, or test mode pointed at real folders, refuses to start: a real run must set neither.

## Sanctuary ambient NPCs (host, behind a flag)

Off by default. `-owambient=<manifest>` (or `OPENWILLOW_AMBIENT`) spawns the town's citizens and runs the stock perch
cycle and node walks with stand-in rules ([record](verification/SANCTUARY_AMBIENT_NPCS.md), all movement rules UNVERIFIED).

```powershell
python tools/census_ambient_npcs.py                      # dens, points, node graph, perch definitions -> local/slice/
powershell -File tools/seed_ambient_npc_assets.ps1       # UModel extraction, clip conversion, import (needs OPENWILLOW_UMODEL)
powershell -File tools/seed_ambient_npc_assets.ps1 -Steps attach -Compose <amb_compose json>   # hair, hats, gear, head textures
powershell -File tools/seed_ambient_npc_assets.ps1 -Steps outline   # rebuild only the ink-line material
python tools/prepare_ambient_world.py                    # host manifest local/slice/ambient_world.json (--observed copies a capture)
powershell -File tools/test_ambient.ps1 -Seconds 60      # self-test, takes the run lock; -Shots tours the viewpoints
```

## Reading the game's UnrealScript (bytecode disassembler prototype)

`research/script_disasm.py` (Python, read-only) turns every script function in the code
packages into pseudo-code. Needs `OPENWILLOW_BL2` or `--game <CookedPCConsole>`; generated
listings belong under ignored `local/`.

```text
python research/script_disasm.py --check                      # structural validation, ~10 s
python research/script_disasm.py WillowGame.upk StatusMenuInventoryPanelGFxObject.PanelOnInputKey
python tests/script_disasm_test.py                            # synthetic tests, no game needed
```

Native functions print as `native_<n>` and their bodies are not in the packages (the sort
cycle of the inventory is one). Layouts marked UNVERIFIED in the source are fitted, see
`docs/verification/SCRIPT_BYTECODE_DISASM.md`.

The C++ loader and VM now support `--script-check`, `--disasm`, `--run`,
`--run-batch` and `--vm-sweep`. The two diagnostic harnesses are:

```powershell
python tools/vm_census.py --reader build/Release/ow-package.exe `
  --json local/phase2/vm-census.json
python tools/replay_trace.py local/ui/traces/<trace>.jsonl `
  --reader build/Release/ow-package.exe --limit 400 `
  --output local/phase2/trace-replay.json
```

Replay accepts `--function <substring>` and `--cooked <directory>`. It compares
scalar returns on newly instantiated receivers with class defaults, skips lossy
objects/aggregates/out results, and blocks native/stub executions from match
counting. Matches do not establish live-state parity. Pairing rejections,
mismatches and skip/block reasons remain in the local report. Evidence and
remaining work: [VM prototype](verification/SCRIPT_VM_PROTOTYPE.md).

Batch format: one case per line, `Package.Class.Function<TAB>Package.SelfClass`
followed by `<TAB>Parameter=kind:value` fields. `i/f/b/y/s/n/o` are explicitly
typed int/float/bool/byte/string/name/null-object; `d/t` adapt numeric/text input
to the reflected parameter type. String/name/text bodies are UTF-8 hex; bool
is `0` or `1`, null-object has an empty body. Missing trailing optional inputs
retain defaults; other missing inputs fail. Output is JSONL with case index,
result type/value, native flag, per-case error and unimplemented-call list.
The batch command exits zero when individual cases fail: inspect those fields.

The UE5 inventory now links the C++ VM for item-only backpack movement. Build
`cmake --build build --config Release` before the Win64 UE editor target; its
module links `build/Release/ow-core.lib` and `ow-lzokay.lib`. Inventory opens
the VM lazily from `OPENWILLOW_BL2`. `OWINVVM` log entries carry script expression
counts/errors; page reports include `vm.enabled/calls/errors/steps/discarded`.
The in-engine suite includes `vm_backpack_down` and `vm_backpack_up`, which must
observe new successful script calls and the expected actual page selections.
Equipment, transfers and sorting still use the existing host adapter.

For a direct installed-script check:

```powershell
$cooked = Join-Path $env:OPENWILLOW_BL2 'WillowGame/CookedPCConsole'
build/Release/ow-package.exe "$cooked/WillowGame.upk" --inventory-move 1 0 5 --cooked $cooked
```

The provider covers item-only rows (headers and empty cells are not modelled). If UE stalls waiting for the local Zen
server before loading, the inventory runner accepts the installed fallback:
`-Extra @('-ddc=InstalledNoZenLocalFallback','-d3d11')`, a launch-time workaround only. Evidence: the
[VM record](verification/SCRIPT_VM_PROTOTYPE.md).

### Native functions and sharing local files

The bodies of `native_<n>` functions are in `Borderlands2.exe`, not the packages.
Analysing the executable locally is allowed; the workflow, what may be committed and the
tooling notes are in [NATIVE_ANALYSIS.md](NATIVE_ANALYSIS.md) (policy:
[LEGAL.md](LEGAL.md), "Analysing the executable"). `tools/private_sync.ps1` mirrors
non-regenerable game-derived files (such as an analysis database under `local/analysis`)
to a store outside the repository; see "Working on two machines" in that page.

## Tools added 2026-10-01 and 2026-10-02 (one line each)

Everything here writes game-derived output only under ignored `local/` (or, for Ghidra, `%OPENWILLOW_ANALYSIS%`), and
every reading it supports stays `UNVERIFIED` until it is compared with the running game.

Native analysis of the executable (local only; policy in [LEGAL.md](LEGAL.md), workflow in
[NATIVE_ANALYSIS.md](NATIVE_ANALYSIS.md), "Native registration and queries"; DECISIONS 2026-10-02):

| Tool | What it does | Command |
|---|---|---|
| `tools/ghidra/run.ps1` | Drives the imported Ghidra project headlessly; `-Tables` builds the native tables, `-Query` decompiles by name, native number, string, callers or vtable slot | `powershell -File tools/ghidra/run.ps1 -Tables -Apply` / `... -Query MissionTracker.UpdateObjective,native:114 -Callees 1 -Label` |
| `tools/ghidra/OwNativeTables.java` | Ghidra script (run by `-Tables`): reads the registration tables, writes `natives.tsv` and `gnatives.tsv` and labels each native | via `run.ps1 -Tables` |
| `tools/ghidra/OwNativeQuery.java` | Ghidra script (run by `-Query`): resolves queries, writes decompilation under the analysis output folder; its header lists every query form | via `run.ps1 -Query '@<file>'` |
| `tools/ghidra/script_natives.py` | Lists every native `UFunction` of the code packages with its `iNative` number (input to the join above) | `python tools/ghidra/script_natives.py <out.tsv> [--cooked <CookedPCConsole>]` |
| `tools/ghidra/class_layout.py` | Computes 32-bit field offsets of a script class from the packages so a native's field read can be named (oracle: `Core.Object` = 0x3C) | `python tools/ghidra/class_layout.py WillowGame.MissionTracker` |
| `research/mission_event_link_ids.py` | Structural oracle: counts the link ids on mission events and reports any outside the ranges the dispatch note predicts; aggregate counts only | `python research/mission_event_link_ids.py [--packages Startup ...] [--examples]` (needs `OPENWILLOW_BL2`) |

`ow-package` modes added for the slice and the paint work (none changes the install; the executor modes run installed data through our own code, not the game):

| Mode | What it does | Command |
|---|---|---|
| `--payload-file` | Writes an export's raw bytes to a file (a shader cache is too large for `--payload`'s JSON) | `ow-package "$cooked\RefShaderCache-PC-D3D-SM3.upk" --payload-file 1 local/paint_research/refcache.bin` |
| `--names` | Prints the package name table in index order as JSON | `ow-package "$cooked\RefShaderCache-PC-D3D-SM3.upk" --names > local/paint_research/ref_names.json` |
| `--mission-run` | Runs one mission's native executor over installed data; steps `accept`, `kickoff`, `obj:<name>[:<bit>]`, `custom:<name>`, `turnin`, `tick:<s>` | `ow-package "$cooked\Startup.upk" --mission-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked $cooked accept kickoff obj:RockPaper_GoToRange turnin` |
| `--behavior-run` | Runs one behavior provider alone; steps `enable:`, `disable:`, `tick:`, `event:<name>[:...]` and `fire:<link id>:<event>` (link-id filter, -1 = all links) | `ow-package "$cooked\Sanctuary_Dynamic.upk" --behavior-run <provider-path> --cooked $cooked fire:-1:OnTakeDamage` |
| `--kismet-run ... --tick` | Runs a Kismet sequence from an entry point (`--remote`, `--mission`, `--op`, `--originator`); trailing `--tick <s>` pairs advance time and run what is due | `ow-package "$cooked\Sanctuary_Dynamic.upk" --kismet-run TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide --cooked $cooked --remote RE_Ep14_OpenMarcusDoor --tick 1 --tick 3` |

The step and event names above are examples from the verification records; the rules the executors follow are the
`UNVERIFIED` notes in [NATIVE_MISSION_DISPATCH.md](verification/NATIVE_MISSION_DISPATCH.md).

Weapon paint and effects (detail in "Weapon paint: where the Master_Gun reading comes from" above):

| Tool | What it does | Command |
|---|---|---|
| `tools/material_static_parameters.py` | Decodes the static parameter set a cooked MIC keeps after its properties (631 of 631 in `Startup.upk` consume their bytes exactly) | `python tools/material_static_parameters.py --reader build/Release/ow-package.exe --package "$cooked\Startup.upk" --mic <name>... --output local/paint_research/static_params.json` |
| `tools/weapon_paint_model.py` | Library: our own-words reading of Master_Gun's colour model, shared by the preparer, the thumbnail renderer and the tests (not a command) | `python tests/weapon_paint_test.py` |
| `research/d3d9_bytecode.py` | Library: our own SM3 (vs_3_0 / ps_3_0) token reader, written from Microsoft's public D3D9 bytecode description; `disassemble(code)` prints a plain register listing; listings are game-derived and stay under `local/` | imported by the paint research; checked in `python tests/weapon_paint_test.py` |
| `research/particle_system.py` | Reads cooked `ParticleSystem` templates (emitters, LODs, modules, baked distributions, `BurstList`, `DynamicParams`) to JSON under `local/phaselock/emitters/`; reads only, renders nothing | `python research/particle_system.py [Part_SirenASHandOrb ...] [--package GD_Siren_Streaming_SF] [--oracle]`, tests `python tests/particle_system_test.py` |
| `tools/prepare_phaselock_target.py` + `tools/import_phaselock_target.ps1` | A stock Phaselock target for the host: the bullymong's mesh, zone-colour material, idle, death and its four `PhaseLock_*` clips (our reader for identities, UModel for the exports, the slice NPC clip conversion), imported under the ignored `/Game/OpenWillow/Characters/PhaselockTarget`; `host_target.json` tells the host the paths, mesh offset and scale | `powershell -File tools/import_phaselock_target.ps1 -Game <BL2> -UModel <umodel.exe>` (under the UE lock), then `tools/run_phaselock_shots.ps1 -Extra '-owcombatpawn=Bullymong'`; tests `python tests/prepare_phaselock_target_test.py` |

Inventory open-time benchmark (DECISIONS 2026-10-02; results in the 2026-10-02 section of
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md); one PC, no original-game figure):

- `-owinvopenbench=<N>` and `-owinvopenbenchdelay=<s>` (UE command line) open the inventory N times and log `OWINVTIME`
  lines per open; `-owinvnopreload` turns off the level-start preloads for an A/B comparison (it is not a test default).
- `powershell -File tools/test_inventory_actions.ps1 -OpenBench 5 [-OpenBenchDelay 20] [-Items local/items/slice]
  [-NoInventoryMovie]` runs the benchmark instead of the action suite and prints one row per open. Pass
  `-Extra @('-ddc=InstalledNoZenLocalFallback','-d3d11','-owinvnopreload')` for the no-preload run; `-Extra` replaces the
  default list, so repeat the defaults.

## Independent oracles: umodel and the game's own object dumps

Two external oracles are run against the existing decode. Neither is copied
from; both stay on the user's machine and write only to ignored `local/`.
See THIRD_PARTY.md for provenance and docs/LEGAL.md for the rules.

### umodel

umodel (UE Viewer, MIT) is a second reader of the same bytes. Point
`--umodel` at `umodel.exe`; the Borderlands game tag is `border`.

```powershell
python tools/crosscheck_umodel.py --umodel $umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --dlc --output local/umodel/crosscheck-all.json
```

compares umodel's `-list` with our `--exports` per package (index, serial offset, serial size, class short name, object
name); drop `--dlc` for the 914 base packages only.

Exporting assets for the second tool uses umodel directly:

```
umodel.exe -game=border -export -gltf -png -nolightmap -uncook -groups `
  -path=<CookedPCConsole> -out=local/umodel <Package>
```

```powershell
python tools/crosscheck_umodel_assets.py --scene local/sanctuary/scene.json `
  --umodel-exports local/umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/umodel/crosscheck-assets.json
```

compares each scene mesh with the matching `<Level>/<Outer>/<Group>/<Name>.gltf` and each decoded PNG with umodel's; the
glTF-to-UE axis mapping is searched, not assumed. `tests/crosscheck_umodel_test.py` and
`tests/crosscheck_umodel_assets_test.py` cover both on synthetic fixtures. Statuses and results:
[umodel record](verification/UMODEL_CROSSCHECK.md).

### OpenBLCMM object dumps

OpenBLCMM ships the output of the game's own `obj dump` console command: an SQLite index
(`%LOCALAPPDATA%/OpenBLCMM/extracted-data/BL2/data.db`) into per-class dump files packed in `blcmm_data_BL2-*.jar`.
Unlike umodel this is not a second decoder; it is what the running engine reported, so it is an oracle for observed
behaviour.

`python tools/blcmm_dumps.py "<object name>" [--raw]` prints one object.
Level objects are named `Sanctuary_P.TheWorld:PersistentLevel.Terrain_2`, with
a colon after `TheWorld`.

```powershell
python tools/crosscheck_blcmm_dumps.py --scene local/sanctuary/scene.json `
  --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 `
  --output local/blcmm/crosscheck.json
```

compares terrain components, BSP model components, actor placement and material texture picks against the dump;
`--reader`/`--game` are needed only for the material comparison. `tests/blcmm_dumps_test.py` and
`tests/crosscheck_blcmm_dumps_test.py` cover the parser and the comparisons on synthetic dump text. Statuses and results:
[dump record](verification/BLCMM_DUMP_CROSSCHECK.md).

## BSP texture axes against the editor's Polys exports

Neither oracle above sees BSP surfaces, but cooked volume-owned Models keep an
editor `Polys` export whose FPoly records carry explicit `Base`, `TextureU`
and `TextureV` vectors.

```powershell
python tools/crosscheck_bsp_polys.py --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/bsp/polys-all.json
```

compares those vectors with the surface records' base-point and texture-axis fields (`--packages` narrows it) and exits
non-zero on a `differ`, an out-of-range reference or a parse error; `no_unique_poly` is reported, not counted.
`tests/crosscheck_bsp_polys_test.py` covers it on synthetic fixtures. Method and results:
[texture-axis record](verification/BSP_TEXTURE_AXES.md).

## Inventory preview follow-ups (paint pass and Maya pose audit)

Backpack scrolling, transfer flows and Maya's preview framing are recorded in
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md) (2026-09-30 sections), not repeated here.

An optional paint pass applies the existing thumbnail palette approximation to
one already imported recipe mesh, without reimporting meshes or deleting folders:

```powershell
python tools/prepare_weapon_paint.py --recipe local/items/smg_maliwan_epic_1.json `
  --materials local/external/umodel/weapon-materials-20260929 `
  --output local/items/paint/smg_maliwan_epic_1.json
$env:OPENWILLOW_WEAPON_PAINT = "$pwd/local/items/paint/smg_maliwan_epic_1.json"
& 'C:/Program Files/Epic Games/UE_5.8/Engine/Binaries/Win64/UnrealEditor-Cmd.exe' `
  "$pwd/host/ue5/OpenWillow/OpenWillow.uproject" -run=pythonscript `
  "-script=$pwd/host/ue5/import_weapon_paint.py" -unattended -nullrhi
```

Run this after `import_weapon_items.py`, which replaces item materials. The
prepared JSON and imported textures/materials remain ignored. This is tested
only for the selected Maliwan epic SMG; packed normal/emissive semantics,
pattern placement and original Master_Gun lighting remain unverified.

`--recipe` also accepts several explicit paths, writing a list when there is
more than one. The UE importer accepts either form. Inputs are validated before
preparation writes its output; unsupported pattern inputs fail rather than
silently choosing a substitute. Keep existing Infinity paint imports separate.

Audit Maya's third-person hold references without changing the package decoder:

```powershell
python tools/audit_maya_menu_pose.py --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/character/anim/hold-reference-audit.json
```

It checks the reflected AnimSetList property type and local reference identity through the existing package CLI;
results and limits are in [INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md) ("Maya
hold-reference audit").

`tools/hud_overlay/probe_panel_projection.js` is a developer-only rendering
benchmark, not loaded by the inventory. Evaluate its repository source in the
collaborative preview, then start `owProbePanelProjection()` asynchronously and
poll `window.owPanelProjectionProbeState`. A visual hold argument of at most
10000 ms allows a screenshot before cleanup. It creates a temporary second
Ruffle player, isolates equipment-panel art and tests a synthetic CSS plane
with a matching HTML target. Angles/perspective are synthetic, not stock values.


## Bounded Sanctuary mover bridge

`tools/prepare_mover.py` follows one installed SeqAct_Interp/group/actor binding and emits an ignored local manifest.
Build CMake Release before UE5, then pass `-owmover=<manifest>` to enable Maya's nearby E interaction (no manifest
leaves the bridge inactive). `tools/test_mover.ps1` runs the two-cycle input/movement/collision test; it refuses an
existing editor and never saves the map. Commands, limitations and evidence:
[mover record](verification/SANCTUARY_MOVER_PROTOTYPE.md).


## Sanctuary Fire-mission slice data (stock values instead of stand-ins)

These tools recover the data the slice mission needs from the installed packages with the owned
reader. All output is game-derived and stays under ignored `local/`. Everything they report is
structural: nothing here has been compared against the running game yet, and each record marks
what is fitted or `UNVERIFIED`.

```powershell
$reader = "build\Release\ow-package.exe"
$game   = $env:OPENWILLOW_BL2
$cooked = "$game\WillowGame\CookedPCConsole"

# World placement: range trigger, Marcus and his walk, dummy spawn, target mover, respawn point
python tools/prepare_slice_world.py --reader $reader --game $game      # local/slice/world.json
python tools/slice_values.py --reader $reader --game $game             # XP reward, Maya health

# Phaselock and one upgrade path (Suspension) from skill data
python tools/prepare_action_skill.py --reader $reader --game $game     # local/character/action_skill_siren.json

# Weapons: legal parts of a balance, card stats, the slice guns, drop tables
python tools/weapon_balance.py --reader $reader --package "$cooked\Startup.upk" parts --help
python tools/weapon_slice_gear.py --reader $reader --game $game --level 8 --seed 1   # local/items/slice
python tools/loot_pools.py --reader $reader --package "$cooked\Startup.upk" table --help
python tools/weapon_card_audit.py --reader $reader --package "$cooked\Startup.upk" --trace <ui trace .jsonl>

# What a shot does (docs/verification/WEAPON_PROJECTILES.md): add the "firing" block to a recipe that has stats
python tools/weapon_firing.py --reader $reader --package "$cooked\Startup.upk" --recipe <recipe .json> [--print]
python tools/weapon_firing_coverage.py --game $game [--only-firing]     # the firing test's coverage recipes + firing blocks (local/items)
pwsh -File tools/test_weapon_firing.ps1 [-Guns a,b,c] [-SkipBuild] [-Engine <UE>]  # in-engine -owfiringtest + check_weapon_firing.py
# (tools/weapon_refresh_firing.py, the L7 live-dump firing block, was retired on 2026-11-02: the cooked firing block replaces it)
powershell -File tools/capture_firing_frames.ps1                       # host frames at the real frames' pose (-owfiringshots)
python tools/firing_contact_sheet.py --offsets=-2:10                   # real / host sheets under local/firing/frames/sheets

# Audio identity chain (no decoding): UE3 AkEvent -> Wwise event -> bank -> .wem
python tools/audio_census.py census
python tools/audio_slice_chain.py --reader $reader                     # local/slice/audio.json (every line of the mission's dialog group)

# Marcus, the target dummy and the stock Maliwan pistol: UModel export + UE import job
python tools/slice_npc_assets.py all                                   # local/slice/npc_assets.json
powershell -File tools/seed_slice_npc_assets.ps1                       # runs tools/slice_npc_editor.py in the editor

# Player side: mission pistol paint, then the UE assets (additive; see below). The turn-in loot stand-in (--reward-only) was removed on 2026-10-07.
python tools/prepare_weapon_paint.py --recipe local/items/slice/slice_mission_pistol_fire.json `
  --materials local/external/umodel/slice-npc/Pistol --mesh /Game/OpenWillow/Weapons/MaliwanPistol/SK_Pistol_Maliwan_2_Fire_seed1 `
  --reader $reader --package "$cooked\Startup.upk" `
  --output local/items/paint/slice_mission_pistol_fire.json
powershell -File tools/seed_slice_player_assets.ps1                    # steps fx, items, paint

# Slice guns: export each recipe's MaterialInstanceConstant chain, prepare, then paint Weapons/SliceItems
umodel.exe -path=$cooked -game=border -export -png -out=local/external/umodel/slice-guns Startup <Mati name> MaterialInstanceConstant
python tools/prepare_weapon_paint.py --recipe <local/items/slice/slice_*.json ...> `
  --materials local/external/umodel/slice-guns --mesh-folder /Game/OpenWillow/Weapons/SliceItems `
  --reader $reader --package "$cooked\Startup.upk" --output local/items/paint/slice_guns.json
powershell -File tools/seed_slice_player_assets.ps1 -Steps paint -Paint local/items/paint/slice_guns.json
```

`--reader/--package` lets the paint tool fill scalar and vector parameters that no MIC sets from the base Material's
own parameter expressions, and read the leaf MIC's static parameters ([DECISIONS.md](../DECISIONS.md) 2026-10-01 and
2026-10-02); without them a chain that leaves a zone colour to the base material fails rather than guessing.
`tools/slice_npc_assets.py` needs numpy and Pillow in the Python that runs it.

### Weapon paint: where the Master_Gun reading comes from

Master_Gun's expression graph is stripped from the cooked packages, so the paint model was recovered from compiled
data instead (2026-10-02, AI-assisted, `UNVERIFIED` against the running game):

```powershell
# Static parameter sets of every MaterialInstanceConstant with a static permutation (exact-consumption oracle)
python tools/material_static_parameters.py --reader build/Release/ow-package.exe `
  --package "$cooked\Startup.upk" --mic Mati_MaliwanUncommon Mati_JakobsCommonPistol `
  --output local/paint_research/static_params.json
# Raw shader cache object for local study (220 MB, stays under local/)
build/Release/ow-package.exe "$cooked\RefShaderCache-PC-D3D-SM3.upk" --payload-file 1 local/paint_research/refcache.bin
build/Release/ow-package.exe "$cooked\RefShaderCache-PC-D3D-SM3.upk" --names > local/paint_research/ref_names.json
# Preview the reading on a prepared gun (pure Python thumbnail renderer)
python tools/render_weapon_previews.py slice_pistol --items local/items/slice `
  --paint local/items/paint/slice_guns.json --output local/paint_research/thumbs --jobs 1
```

- `tools/material_static_parameters.py` decodes the static parameter set a cooked MIC keeps after its properties (631 of
  631 in `Startup.upk`); `research/d3d9_bytecode.py` is our own SM3 token reader, written from Microsoft's public
  description (its output is game-derived and stays under `local/`); the colour model is written in our own words in
  `tools/weapon_paint_model.py` and drawn by `host/ue5/import_weapon_paint.py`. The reading, the texture colour space
  (pass 2) and the shading inputs (pass 3, behind a flag) are all `UNVERIFIED` against the running game; see the three
  weapon-paint entries dated 2026-10-02 in [DECISIONS.md](../DECISIONS.md).

`tools/seed_slice_player_assets.ps1` holds the machine-wide UE run lock (`tools/ue_lock.ps1`) for each editor launch and never deletes
`Weapons/Items`, Maya's folder or slice NPC content. `fx` runs `host/ue5/import_infinity_proxy.py` only when
`Weapons/InfinityProxy` is absent (it supplies `M_OW_FxAdditive` and `M_OW_BulletHole`). `items` runs the additive
`host/ue5/import_slice_items.py` (pool-rolled slice guns into `Weapons/SliceItems`, grey stand-in material; existing
`SK_<id>` assets are skipped). `paint` runs `import_weapon_paint.py`, which accepts a `mesh` target and MIC chains
without a pattern texture. How `tools/run_quest.ps1` and `tools/test_quest.ps1` use the slice items:
[SANCTUARY_RPG_MISSION.md](verification/SANCTUARY_RPG_MISSION.md).

Gun visuals (2026-10-04, `docs/verification/WEAPON_VISUALS.md`):
- `tools/weapon_refresh_fragments.py` rebuilds the fragment list (body variants, no `*_None` parts, hidden-bone
  triangles cut) of existing local recipes; `tools/weapon_refresh_stats.py` re-evaluates their `stats`, optionally on
  the live overlay; `host/ue5/import_gun_meshes.py` re-imports the meshes of named ids only.
- `-owgunshots -owgunids=<id,id>` captures each gun in first person and from the side (`OWGun_<id>.png`,
  `OWGun_<id>_side.png`); `tools/weapon_visual_compare.py` compares a real and a host frame cell by cell.
- The arms play per-type clip sets: convert `Anim_1st_Person.<type>` with `tools/prepare_character_anims.py --clips
  Idle Run_F Sprint Jump_Start Jump_Idle Jump_End Draw ADD_Fire_Recoil` (bones are matched by name, so the rifle set's
  other bone order converts), then import with `OPENWILLOW_CHARACTER_ANIMS=AssaultRifle=...;SMG=...;Shotgun=...`.
- `python tools/weapon_view_model.py --game <BL2 dir> --dir <items folder...>` writes `weapon_view.json` beside the
  recipes: each recipe's weapon-type `PlayerViewOffset` and `FirstPersonMeshFOV`. The walker places the arms and sets the
  foreground FOV from it when a gun is selected; rerun it when a recipe folder changes (a recipe without an entry gets
  no offset and 45).
  Since 2026-11-03 it also writes `muzzle_flash_fx` (per element, the type's muzzle-flash particle emitters: axis lock,
  size, offset, life, colour) and `shell_casing_fx` (the casing particle's life, start velocity and acceleration), read
  from the cooked particle systems by `tools/weapon_muzzle_fx.py` (`--system <ParticleSystem>` / `--collection <path>`
  print one as JSON; the lookup-table layout is fitted, see WEAPON_VISUALS.md section 18). `-owflashfill=<f>` sets the
  host's flash-quad fill (default 0.5).
- `pwsh tools/real_game/gun_ads_capture.ps1` (real game, hub lock, saves backed up and compared, written 2026-11-07, never run): an idle and an ADS still of each
  named recipe in Sanctuary, lane E's way, with the live zoom state read back, output under ignored `local/realgame/gun_ads/`. Default guns: the slice shotgun and a Hyperion pistol on iron sights
  (`WEAPON_VISUALS.md` section 23). `-MeasureFov` adds, per gun at hip and at full zoom, the view's horizontal FOV from known directions projected through the HUD canvas and two frames 4 degrees apart under `fov/`, so the turn can be measured from pixels too. Each gun is made at Maya's level. The finalizer is the migrated one, which `tests/gun_ads_capture_test.py` holds equal to `phaselock_integrity_capture.ps1`'s. Do not add `slice_mission_pistol_fire`:
  readying it made it vanish in lane E (cause UNVERIFIED).
- `pwsh tools/real_game/weapon_parity2_capture.ps1` (bar lifted 2026-11-09, see "Bar lifted"; real game, hub lock, saves backed up and compared): the world FOV
  through the HUD canvas, the flash/casing particle systems and impact-explosion hooks per gun, an equal-level status
  series and hip/ADS/reload frames of the slice guns (`scripts/weapon_parity2_probe.py`). Written 2026-11-03; it has not
  run to completion yet (the game stopped at a Steam message box). Not run since the finalizer fix (PR #200): `UNVERIFIED` for its own capture until it runs once.

Weapon part library (2026-10-23, `docs/verification/WEAPON_PART_ASSETS.md`): every gestalt mesh once and every skin as a
material instance, guns assembled from their parts at run time (`OpenWillowWeaponAssembly`).

```powershell
python tools/weapon_part_census.py --game $env:OPENWILLOW_BL2 --live local/realgame/wpa/live_visual_census.json `
  --output local/wpa/census/visual_census.json          # cooked census (+ comparison with the live one)
pwsh tools/real_game/weapon_part_session.ps1 -Mode Census   # live census (game, saves blocked; RTSE-style find_all)
python tools/export_weapon_skins.py --umodel <umodel.exe>    # UModel 1590: every skin MIC + textures, inventoried
python tools/weapon_library.py meshes                        # one glTF primitive (UE slot) per gestalt fragment
python tools/weapon_library.py skins --game $env:OPENWILLOW_BL2
python tools/weapon_library.py index                         # local/weapon_library/library.json (parts, skins, bones)
powershell -File tools/import_weapon_library.ps1 [-Only meshes|skins] [-Reloads 'Pistol=<prepared reload json>,...']
python tools/weapon_assembly_sample.py --game $env:OPENWILLOW_BL2  # sample: type x manufacturer, legendaries, skin sweep
powershell -File tools/run_weapon_assembly_shots.ps1 -Items local/wpa/sample -Quick   # host frames + assembly log
python tools/weapon_assembly_oracle.py --log <run.log...> --index local/wpa/sample/sample_index.json `
  --live local/realgame/wpa/live_guns.json                   # triangle oracle: game = host per gun
pwsh tools/real_game/weapon_part_session.ps1 -Mode Capture -Inspect -RecipeList <file of recipe paths>
```

- The host backpack holds 39 items (the game's maximum), so `run_weapon_assembly_shots.ps1` shows at most 39 guns per run;
  split a larger sample into folders. Run `weapon_view_model.py` on each folder so the guns are held at their type's offset.
- `-owweaponprebuilt` forces the old per-recipe meshes (A/B); a recipe with an explicit `MeshPath` always uses it. A gun the
  library cannot build completely falls back too, with `fallback to the pre-built mesh ... (<reason>)` in the log.
- `OpenWillow.WeaponAssembly` is a UE automation test of the planning rules on a synthetic library; run it in `-game` mode
  (`UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.WeaponAssembly; Quit"`).
- A meshes import re-imports the reload clips recorded in `local/weapon_library/reloads.json`.
- `-owgunexec=<cmd>|<cmd>` (with `-owgunshots`) runs console commands before the captures, `+` for a space, e.g.
  `-owgunexec=ow.RollGun+GD_Itempools.WeaponPools.Pool_Weapons_All_06_Legendary+30+1 -owgunids=gen_Pistol_Torgue_5_Calla_30_1`
  shows a generated gun (`-ExecCmds` did not reach the console in a map launch here).
- The capture session spawns guns in memory only, empties the save's backpack in memory for Inspect (saves are blocked and
  compared afterwards), closes the queued "Unlocked!" notices through the SDK, and holds each gun through the inventory
  manager (the quick-slot key did not switch to a freshly spawned gun).

- `tools/audio_slice_chain.py` stops at raw Wwise Vorbis `.wem` files. No decoder is approved
  yet; picking one is a maintainer decision (license and provenance entry first).
- `tools/slice_npc_editor.py` imports `unreal` and only runs inside the editor.
- `tools/build_phaselock_fx_constants.py` rebuilds the ignored `local/phaselock/fx_constants.json` that `import_phaselock_fx.py` needs, from the decoded emitters and `fx_manifest.json`. One colour in it stays hand-kept: the tool keeps it and stops if it is missing ([PHASELOCK_STOCK_DATA.md](verification/PHASELOCK_STOCK_DATA.md), "The stock colour constants").
- `tools/import_phaselock_fx.ps1` waits for the machine-wide UE run lock (`tools/ue_lock.ps1`) like the suite scripts. `host/ue5/import_phaselock_fx.py`
  and the `tools/run_phaselock_shots.ps1 -Extra` switches `-owbubbleradius` and `-owfxscalar` are described in
  [PHASELOCK_STOCK_DATA.md](verification/PHASELOCK_STOCK_DATA.md). Write the values as literals, since a PowerShell
  variable inside `-Extra` from a `-File` call is not expanded.
- The structural oracles (`research/behavior_census.py`, `research/struct_defaults_census.py`),
  `ow-package --properties-batch` and `tools/weapon_card_audit.py` are described in
  [BEHAVIOR_DATA_DECODE.md](verification/BEHAVIOR_DATA_DECODE.md) and
  [WEAPON_BALANCE_DECODE.md](verification/WEAPON_BALANCE_DECODE.md).

Records: [behavior data](verification/BEHAVIOR_DATA_DECODE.md),
[weapon balances](verification/WEAPON_BALANCE_DECODE.md),
[world placement](verification/SLICE_WORLD_PLACEMENT.md),
[Phaselock stock data](verification/PHASELOCK_STOCK_DATA.md),
[audio chain](verification/SLICE_AUDIO_CHAIN.md),
[NPC assets](verification/SLICE_NPC_ASSETS.md).
