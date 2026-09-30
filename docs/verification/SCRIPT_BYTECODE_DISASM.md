# UnrealScript bytecode disassembler (Phase 2, step 2): verification record

AI-assisted (2026-09-30). This is the first piece of ROADMAP Phase 2: a **read-only
disassembler prototype in Python** (`research/script_disasm.py`), the same way
`research/native_count.py` preceded the C++ package reader. It executes nothing.
There is no C++ port yet, no interpreter, and no object model; those are the next steps
(see "What is not done").

No game bytes are in the repository. The generated listings stay under ignored `local/`.
The opcode set is the general, public UE3 expression-token set written from knowledge of
the format, then fitted to this build by the structural checks below. No code from UE
Explorer or any GPL tool was read or copied.

## What was established, and how it was checked

**Function layout.** Recovered from the packages, not from a source. The export data of a
script `UFunction` is: `u16 N`, `N` x `u16` (the Gearbox local-variable array named in
ENGINE_PLAN.md), 10 x `i32` (object references and line fields, meaning UNVERIFIED),
`i32 ScriptBytecodeSize` (in-memory size), `i32 ScriptSize` (file size), the script bytes,
then the function tail the census already reads (`iNative`, `OperPrecedence`,
`FunctionFlags`, optional `RepOffset`, `FriendlyName`). The script always ends in `0x53`
(`EX_EndOfScript` in this build).

**In-memory offsets.** `Jump`, `JumpIfNot`, `Case`, `Skip` and friends count *memory*
bytes, where every object reference occupies 8 bytes instead of the 4 stored in the file.
Shown two ways: jump targets only land on statement starts after converting, and a
least-squares fit of `ScriptBytecodeSize` against per-opcode reference counts gave exactly
0 extra bytes for reference-carrying opcodes once converted (and +4 for the five opcodes
that carry a plain `i32`, `0x4C`-`0x50`).

**The oracle is structural, not a spot check.** A function "decodes exactly" only if all of
these hold: the header's `ScriptSize` ends exactly at the function tail; the last byte is
`0x53`; the expression grammar consumes exactly `ScriptSize` bytes; the decoded in-memory
size equals the header's `ScriptBytecodeSize`; every jump/case target is a statement
start. A wrong operand size desynchronises the stream and fails one of these within a few
instructions. Some opcode layouts had two candidates that decode equally (for example
`DynArrayFindStruct` as `EwP` versus `EwEP`); those are marked UNVERIFIED in the source.

```text
python research/script_disasm.py --check
Core.upk                              44 script functions      44 decode exactly      0 fail
Engine.upk                          3574 script functions    3573 decode exactly      1 fail
GameFramework.upk                     14 script functions      14 decode exactly      0 fail
GearboxFramework.upk                 508 script functions     508 decode exactly      0 fail
WillowGame.upk                      8356 script functions    8347 decode exactly      9 fail
GFxUI.upk                             42 script functions      42 decode exactly      0 fail
IpDrv.upk                            206 script functions     206 decode exactly      0 fail
OnlineSubsystemSteamworks.upk        234 script functions     234 decode exactly      0 fail
AkAudio.upk                            0 script functions       0 decode exactly      0 fail
TOTAL                              12978 script functions   12968 decode exactly     10 fail    (9.95 s)
```

12,978 script functions is the same total the native census reports, so the loader sees
every one. The 10 failures are listed by `--show`: five in-memory-size mismatches of 8-24
bytes (`WillowPlayerController.InnerSetOnlineStatus`, `WillowGameInfo.CheckMapChangeConditions`,
`FrontEndPlayerListGFxObject.RefreshPlayerList` and `.HandlePlayerDetailsButtonClick`,
`WillowGFxColiseumOverlayMovie.extSetupResultsScreen`), four reference/jump desyncs
(`GameInfo.UpdateBestNextHosts`, `CreditsDataProviderGFxObject.AppendNewCreditsDataToFront`,
`StatusMenuExGFxMovie.GetHighestChainedPlotMissionCompleted`, `WillowWeapon.InitMeshAnimation`,
`WillowGFxUIManager.PlayMovie`). A few opcodes (`DynArraySort`, `DynArrayInsertItem`,
`StructMember`) are the likely culprits; not yet investigated.

```text
python tests/script_disasm_test.py
Ran 11 tests ... OK   (synthetic byte strings only; no game data)
```

## What this gives us already

Readable real UnrealScript for the menu and gameplay glue. Example (shape only, names are
from the package): `StatusMenuInventoryPanelGFxObject.PanelOnInputKey` disassembles to a
controller-id check, a delegate call to `OnInputKey`, and a call to
`WillowInventoryGFxMovie.HandleInputKey`. Script-side navigation and equip logic
(`NormalMove`, `MoveDelta`, `StartEquip`, `CompleteEquip`, `IsComparing`,
`InventoryPanelInputKey`) is bytecode and can be read.

**Not everything is script.** The backpack sort cycle itself (`extOnChangeSort`,
`ApplySortConfiguration`, `SetSortLabel`, `GetNextSortConfiguration`) is **native C++**: only
the glue (`OnListSort`, `SaveBackpackSortPreference`) is bytecode. Sort order and grouping
therefore still have to come from observing the real game, as the keyboard observation in
INVENTORY_MOVIE_PROTOTYPE.md does.

## What is not done

- **No C++ port.** The loader belongs in `src/` next to `package.cpp`; that needs a
  `CMakeLists.txt` change and possibly a touch of the `Reader`, both listed as sensitive in
  CLAUDE.md. Waiting on explicit confirmation.
- **No interpreter and no object model** (ROADMAP Phase 2 bullets 1 and 3).
- Opcode operand layouts marked UNVERIFIED in the source table, including the meaning of
  `0x4C`-`0x50`, `0x5E` and `0x5F` (a typed `Let`).
- Native function *names* come from `FriendlyName`/`iNative` of exports that happen to be
  loaded (the nine code packages); natives defined elsewhere print as `native_<n>`.
- Decoding structure does not prove semantics. Running the code and comparing with the real
  game's trace (`tools/sdk_trace`) is the real test, and belongs to the interpreter work.
