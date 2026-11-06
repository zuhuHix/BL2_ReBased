<div align="center">

<!-- Logo: pending hub decision. The untracked .github/assets/BL2_ReEngine-logo-512.png is the maintainer's file and is not used here. -->

<img src=".github/assets/banner.png" alt="BL2_ReBased: Borderlands 2, re-based onto a modern engine" width="100%">

[![CI](https://github.com/zuhuHix/BL2_ReBased/actions/workflows/ci.yml/badge.svg)](https://github.com/zuhuHix/BL2_ReBased/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Status: prototype](https://img.shields.io/badge/status-prototype-orange)](#where-things-stand)
[![Engine: UE 5.8](https://img.shields.io/badge/engine-Unreal%20Engine%205.8-black)](docs/OPENWILLOW_ENGINE_PLAN.md)
[![C++20](https://img.shields.io/badge/C%2B%2B-20-00599C)](CMakeLists.txt)
[![Last commit](https://img.shields.io/github/last-commit/zuhuHix/BL2_ReBased)](https://github.com/zuhuHix/BL2_ReBased/commits/main)
[![Discussions](https://img.shields.io/badge/chat-discussions-8250df)](https://github.com/zuhuHix/BL2_ReBased/discussions)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-yellow)](https://buymeacoffee.com/zuhu)

</div>

# Borderlands 2 ReBased

Hey, I'm zuhu. This is Borderlands 2 ReBased.

Borderlands 2 runs on Unreal Engine 3, a decade-old engine that Gearbox modified on top of, which makes it even worse to decode than stock UE3. What I'm actually doing here is decoding and reverse-engineering BL2's own game files, and writing custom Python import scripts to bring them into Unreal Engine 5. I want to be clear about this: I'm not modeling copies of anything and I'm not remaking assets from scratch. I'm reading the original files and rebuilding the engine underneath them.

[Why bother?](#why-bother) · [What you'd get](#what-youd-get-eventually) · [Progress in pictures](#progress-in-pictures) · [Where things stand](#where-things-stand) · [Roadmap](#roadmap) · [Is this legal?](#is-this-legal) · [Help me keep going](#help-me-keep-going) · [For developers](#for-developers)

---

> **There is no game to play yet.** What exists is a partial developer prototype. The engine reads the game's files and shows three maps as frozen scenery in UE5. A developer slice on Sanctuary lets Maya walk, shoot, use Phaselock, fight stand-in enemies on a gun range, and run one mission. All of it runs in a developer harness on labelled stand-ins, and most of it has **not** been compared with the original game. No enemy in the main slice attacks Maya yet, and there's no story. I put this up early so people can watch it grow, not because it's ready.
>
> This is a fan project. Not affiliated with Gearbox, 2K or Take-Two. No game files and no Gearbox code live in this repo. It only works against **your own purchased copy** of Borderlands 2, and it never touches your install.

## Why bother?

Borderlands 2 is basically two separate things. There's *the stuff*: maps, guns, characters, sounds, the story, the skill trees, the files sitting in your game folder right now, and honestly they're brilliant, 14 years later people still love this game. And there's *the machine*: `Borderlands2.exe`, the 2012 program that loads all of that and turns it into a game. It's old, 32-bit, and locked. Nobody outside Gearbox can touch it.

Every big complaint people have (broken co-op, out-of-memory crashes, no ultrawide, no level editor) lives in the machine, not the stuff. Mods can change the stuff. They can't touch the machine. Hence why this project should give this game a third life ! can you imagine unlimited modding, new npc's ,new maps, new characters, new UI, reworked anything. Litteraly endless possibilites

So this project builds a new machine: something that reads BL2's original files straight out of your game folder, exactly as they shipped, and runs them on UE5. Nothing of Gearbox's ships with it.

Same idea as [OpenMW](https://openmw.org/) (Morrowind), [OpenRCT2](https://openrct2.org/) (RollerCoaster Tycoon 2), and [Ship of Harkinian](https://www.shipofharkinian.com/) (Ocarina of Time). Those work. Nobody's done it for BL2 yet. That's the challenge.

<details>
<summary><b>Why can't mods just fix this?</b></summary>
<br>

Short version: mods can deliver about 80% of what players ask for (balance, QoL, new guns, sharper textures). The remaining 20% is exactly the stuff people want *most*, and it's physically inside `Borderlands2.exe`:

- **Co-op** runs through a backend mods can't replace
- **64-bit** needs the engine recompiled, and only Gearbox can do that
- **Modern graphics** are blocked by the 2012 DirectX 9 renderer baked into the exe
- **New maps** need an editor that was stripped out before the game shipped

Full research: [docs/BL2_REMASTER_ANALYSIS.md](docs/BL2_REMASTER_ANALYSIS.md).

</details>

## What you'd get (eventually)

**The first milestone:** walk around Sanctuary as Maya, grab a gun, Phaselock something and finish a mission, all running on BL2's own game code in UE5. Everything below comes after that.

When it's done (see the roadmap; this isn't a remaster, not a remake) here's what a new machine actually buys you:

- **Co-op that works.** No SHiFT, no forced account linking, no hardlock on the title screen.
- **64-bit.** The ~4 GB memory wall behind a lot of the crashes and Ultra HD pack problems: gone.
- **Modern graphics.** Real ultrawide, any resolution, unlocked framerate, optional modern lighting.
- **New maps.** BL2 never got a level editor. UE5 comes with one built in.
- **Your mods still work.** BLCMM text mods edit the same data we load, so they should just carry over.
- **Your saves still work.** Real save files, same characters.
- **It can't be cancelled.** If the servers go, the game keeps working.
- **The Pre-Sequel too.** Same engine underneath, comes along later for free.

## Progress in pictures

*Images as of 2026-11-06. Each part shows the state in one line, then the game next to BL2_ReBased where a real-game frame exists, and an older frame where it shows progress. Where no real-game frame exists yet, the section says so instead of showing a match. Captions include what still looks wrong.*

The full-size frames are in the [`progress-gallery`](https://github.com/zuhuHix/BL2_ReBased/releases/tag/progress-gallery) release, not in the repository: they show game content, and the rules for that are in [docs/LEGAL.md](docs/LEGAL.md).

<!-- Gallery images are release assets on the progress-gallery prerelease (uploaded by the maintainer). Not in the git tree. -->

### Sanctuary and the world

**State:** Sanctuary loads as a walkable scene in UE5, but it still looks far flatter and darker than the game. Lighting and materials are approximations.

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/sanctuary-game-2026-09-22.jpg" width="48%" alt="Game: Sanctuary street with BL2's HUD">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/weapons-host-2026-11-06.jpg" width="48%" alt="BL2_ReBased: a host street view in the slice">
</p>

<p align="center"><sub>Left: the game (2026-09-22). Right: a host street on 2026-11-06, the same kind of view (from the weapons frame below). The host side is flatter, and its lighting and materials are still approximations.</sub></p>

### Maya and Phaselock

**State:** Phaselock casts, locks and lifts a target in the host. Round 24 has merged (#104), but the target's size, colour and pose are still being tuned.

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/maya-phaselock-host-2026-09-28.jpg" width="48%" alt="Older: Phaselock cast, no lock yet">
</p>

<p align="center"><sub>Before: an old Phaselock cast (2026-09-28), a purple bubble with no enemy locked.</sub></p>

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/maya-phaselock-host-2026-11-06-yaw180-1500ms.jpg" width="60%" alt="Phaselock in the host, round 24, at the default facing">
</p>

<p align="center"><sub>Phaselock in the host, round 24, at the default facing: the bubble and a lifted bullymong. About 5.1/10 on our visual critic against a target of 8. The target is too large and pale, and which way it faces is still unsettled. No real-game frame of this exact view is published yet.</sub></p>

### Weapons and projectiles

**State:** The guns are built from the game's own parts and fire with their own firing modes, muzzle flash and impacts. Their look is still short of the game: an independent critic scored the host against real-game frames at 6.5 out of 10 in the last round (lane E round 4, 2026-10-22; the target is 8).

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/weapons-pairs-2026-10-04.jpg" width="60%" alt="Pair sheet from an older lane: game and host guns">
</p>

<p align="center"><sub>An older lane C pair sheet (2026-10-04, an earlier build). The rows alternate: rows 1 and 3 are the real game (BL2's own HUD and minimap), rows 2 and 4 are the host, each pair showing the same gun. Paint and reload framing still differ, and the weapon paint is UNVERIFIED.</sub></p>

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/weapons-host-2026-11-06.jpg" width="48%" alt="Now: a Vladof assault rifle in the slice">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/projectiles-host-2026-11-02.jpg" width="48%" alt="Shotgun test shot in the host">
</p>

<p align="center"><sub>Left: a Vladof assault rifle in the host (2026-11-06), still being fitted. Right: floating damage numbers from a shotgun blast at a test target in the host (2026-11-02). Spread and damage rules are covered in docs/verification; no real-game frame of this view.</sub></p>

### Enemies and the gun range

**State:** The gun range is on `main`: host enemies move, approach and drop loot on death. Their AI rules are host-side and partly UNVERIFIED, and no real-game range exists to compare with.

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/range-host-2026-11-06.jpg" width="60%" alt="The host's gun range with enemies on snow">
</p>

<p align="center"><sub>The host's gun range outside Sanctuary: two bullymongs and three spiderants on snow, with a totem. Enemy AI rules are host-side and partly UNVERIFIED. No real-game range exists to compare with.</sub></p>

### Skills

**State:** The skills page is navigable and runs its spend rules. Skill modifiers on Maya's survival stats are on `main` (#93); respec at a placed machine is still an open PR (#106).

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/skills-game-2026-10-21.jpg" width="48%" alt="Game: Skills page one second after Tab">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/skills-host-2026-11-06.jpg" width="48%" alt="BL2_ReBased: Skills page on the same view">
</p>

<p align="center"><sub>Left: the game (2026-10-21). Right: the same page and tree in the host (2026-11-06). Close in layout, but the backdrop and character differ, and the character isn't drawn on this page yet. The skill numbers and effects are UNVERIFIED against the game.</sub></p>

### Inventory and menus

**State:** The inventory and status pages run in the host, with the game's own card movies. Card fit and some numbers still differ from the game.

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/inventory-game-2026-10-21.jpg" width="48%" alt="Game: inventory page">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/inventory-host-2026-11-06-r1.jpg" width="48%" alt="BL2_ReBased: inventory page, 2026-11-06">
</p>

<p align="center"><sub>Left: the game's inventory page (2026-10-21). Right: the same page in the host, with real card rows and rendered guns (2026-11-06). Some guns render too pale, a known paint and preview problem, and the backdrop and a few values still differ.</sub></p>

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/inventory-host-2026-10-09.jpg" width="48%" alt="Before: host inventory, 2026-10-09">
</p>

<p align="center"><sub>Before: the host's inventory page on 2026-10-09, an older frame. Guns are silhouettes and the money values are test values.</sub></p>

### Vendors and lootables

**State:** The gun vending machines, the currency, the Ammo Dump and Dr. Zed (#99) and the aimed pickup (#101) are on `main`. Lootables are an open PR (#105). The shop page is a host stand-in, and no real-game shop frame exists to compare with.

<p align="center">
  <img src="https://github.com/zuhuHix/BL2_ReBased/releases/download/progress-gallery/vendors-host-ammo-dump-2026-11-06.jpg" width="60%" alt="The Ammo Dump in the host">
</p>

<p align="center"><sub>The Ammo Dump in the host. The shop page is a stand-in, not BL2's own; prices and rules come from local data. A like-for-like real-game shop frame doesn't exist yet.</sub></p>

## Where things stand

*Updated 2026-11-06. Pictures are from the progress gallery, and each one is dated.*

**Short version:** there's no game to play yet. The Sanctuary slice is a partial developer prototype: it runs in a developer harness, on stand-ins, and most of it has not been compared with the original game. The vending shop is not complete: the gun machines' stock and currency, and the Ammo Dump and Dr. Zed, are on `main` (#99), but lootables are an open PR (#105).

### What works today (merged to `main`)

Each item names the merged PR. Anything not checked against the running game is `UNVERIFIED` in its record.

- **Maya.** Movement numbers from the stock data (#77). Shield, health and Fight For Your Life in one host state (#72). Action-skill cooldown on BL2's own HUD movie (#47).
- **Down and respawn.** The down, bleed-out, second-wind and respawn rules run on the VM (#59, [COMBAT_DOWN_RESPAWN.md](docs/verification/COMBAT_DOWN_RESPAWN.md)).
- **HUD.** BL2's movie HUD by default (#54), the mission tracker and Fight For Your Life in the movie (#63), damage arrows and the level-up banner (#67), the shield bar (#80), BL2's minimap clip (#53), and the Map screen (#97).
- **Guns.** Four slice guns with ADS, reload clips and muzzle flash (#48, #68). Guns built from the game's own parts (#57, #58, #86). Firing from the parts, including elemental explosions (#56, #70). Bullets carry momentum (#62).
- **Pickups.** The world pickup card uses BL2's own card movie (#95). E takes the aimed pickup, not the nearest one (#101, merged 2026-11-06; the host rule is `UNVERIFIED`).
- **Gun range.** Static dummies and moving enemies with enemy AI, and loot on death from the VM's death roll (#89, merged 2026-11-06).
- **Skills.** The skills page is navigable with the keyboard (#35, #49). Skill modifiers on Maya's survival stats (#93, merged 2026-11-06, with the skill stack #73 to #85 that was folded into it).
- **Vendors.** Currency and stock persist, and non-weapon item values were confirmed live (#87). The Ammo Dump and Dr. Zed sell, and ammo purchases fill the pools (#99, merged 2026-11-06). The shelves for shields and grenade mods sell stand-in gear with no stats.
- **Phaselock.** Presentation and colour constants (#50, #94), and round 24 of the hand's colour layers and target probe (#104). Lift bob and reload/weapon-swap rules are confirmed in the real game ([PHASELOCK_STOCK_DATA.md](docs/verification/PHASELOCK_STOCK_DATA.md)).
- **Inventory and menus.** Tab switching with Comma, Period and the number keys (#91, #92). VALUE thumbnails and card fits (#69). An item set that would leave the backpack without room is refused (#71). The Missions log shows the four Fire mission states (#64, #100).
- **The Fire mission.** Accept, objective and turn-in run in the host on stock world data, with labelled stand-ins ([SANCTUARY_RPG_MISSION.md](docs/verification/SANCTUARY_RPG_MISSION.md)).
- **The slice gate.** `tools/test_slice_gate.ps1` runs 14 checks of the Phase 4 loop in sequence, in one editor session (#90, #98). A NOT_AVAILABLE link means the host lacks that piece. Its latest recorded run is on `main` at 195a016 plus the #98 branch (2026-11-04): **10 PASS, 0 FAIL, 4 NOT_AVAILABLE**. Several links rely on named test fixtures (a teleport, a granted Phaselock point, a lethal-damage call), and the checker prints each one next to its result. A PASS means the host did this in this order in one session. It does not mean the rule matches Borderlands 2. The record is [SLICE_GATE.md](docs/verification/SLICE_GATE.md).

**Since that run,** the range (#89), skill modifiers (#93), ammo and health vending (#99) and the aimed pickup (#101) have merged. No newer gate run is recorded in the docs, so the links those PRs affect are not re-graded here.

### In progress (open PRs, not on `main`)

These are not in the game yet, and the records above don't count them.

- **Vendors part 3 (#105):** lootables and pickups in the host.
- **Quick Change respec (#106):** respec at the placed machine.

### Not yet

- **Parity with the original game.** Most of the slice has not been compared with BL2. Nothing here should be read as "it works like the real thing" unless its record says so.
- **Enemies that fight back in the slice.** The main slice's stand-in has no attack on `main`. The range's enemies have AI, but the gate's enemy-attack link has not been re-run since the range merged.
- **The rest of the game.** The other ~79 maps, the other Vault Hunters, the full campaign, co-op, your saves.

Next up: the open PRs above, a re-run of the slice gate on current `main`, and a proper side-by-side comparison of the slice with the original game. The small items are tagged *good first task* in [ROADMAP.md](ROADMAP.md#now--next).

<details>
<summary><b>Show me the numbers behind that</b></summary>
<br>

Every claim above comes from a dated verification record. Automated checks are reported separately from "a human looked at it," and anything unverified is labelled `UNVERIFIED` in the code and docs.

- **Phase 0, gated 2026-09-10.** The reader reads 2,008 of 2,008 packages: 4,751,329 serialized exports. Nine code packages decode byte-for-byte identically to an independent Python reader. Tagged properties on a real weapon part match BLCMM's dump. One mesh and one texture were extracted and rendered in UE 5.8.
- **Phase 1, in progress.** `Ash_P`, `Sanctuary_P` and `SouthpawFactory_P` load as frozen scenes with a four-channel material approximation, an inspection lighting rig, a Sanctuary sky approximation, an opt-in outer-hull import and a free-flight camera. Saved scenes reopen with zero verification errors. A command-line selector lists 37 base-game maps (82 with `--include-dlc`), and an in-game Tab list switches between imported scenes. Walking is an opt-in placeholder, verified on Sanctuary only. Sanctuary runs at 12 to 15 FPS on an integrated-GPU laptop. Not done: native sky graph decode and parity, skeletal meshes, lightmaps, real material graphs, full collision, the other 79 maps.
- **Slice gate, 2026-11-04.** `tools/test_slice_gate.ps1` on `main` at 195a016 plus the #98 branch: 10 PASS, 0 FAIL, 4 NOT_AVAILABLE of 14 links. Host behaviour only.

Records: [decisions log](DECISIONS.md) · [Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md) · [Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md) · [cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md) · [map selection / Southpaw Factory](docs/verification/MAP_SELECTOR_VERIFICATION.md) · [UV / winding](docs/verification/UV_WINDING_VERIFICATION.md) · [collision and walking](COLLISION_WALKING_VERIFICATION.md) · [performance / in-game selector](docs/verification/PERFORMANCE.md) · [umodel cross-check](docs/verification/UMODEL_CROSSCHECK.md) · [object-dump cross-check](docs/verification/BLCMM_DUMP_CROSSCHECK.md) · [BSP texture axes](docs/verification/BSP_TEXTURE_AXES.md) · [slice gate](docs/verification/SLICE_GATE.md) · [gates and baseline](docs/verification/GATES.md).

The progress gallery's images are game content, so they are release assets and not in the repository (see [docs/LEGAL.md](docs/LEGAL.md)). Anyone with the game can reproduce the same kind of frame with the commands in [docs/TOOLING.md](docs/TOOLING.md).

</details>

## Roadmap

Six phases. Each one ends with a gate (a thing you can actually see or do) so it's always clear whether this is moving.

**Current priority: a vertical slice, not breadth.** Porting all 82 maps is the easy part of this project. The hard, unproven part is phases 2 through 4, actually running the game's code. So right now the goal is proving those on **one map (Sanctuary, already the furthest along) and one Vault Hunter** end-to-end, before spending more time on additional maps or characters. Full details and why: [ROADMAP.md](ROADMAP.md#priority-the-vertical-slice).

- **Phase 0 · Read the files.** The engine can open every BL2 file. Done: reads all 2,008 packages, gated 2026-09-10.
- **Phase 1 · See a map.** Fly around Sanctuary in UE5, verified against the real game. No enemies, no guns yet. Full 82-map coverage comes later, in phase 5. In progress: three maps load as frozen scenery (since 2026-09-13), placeholder walking on Sanctuary (since 2026-09-14), and Sanctuary is not yet compared with the real game. It looks close, but is still missing some lighting and NPCs.
- **Phase 2 · Run the game's brain.** BL2's own gameplay code executes, scoped to Sanctuary and one Vault Hunter. In progress: a prototype since 2026-09-30 (C++ bytecode loader, interpreter and Core natives), driving backpack navigation in UE5 since 2026-10-01.
- **Phase 3 · Make a body move.** Walking, jumping, falling, animation and collision, scoped to the same slice. Maya's movement numbers come from the stock data (#77). Maintainer-tested on 2026-10-02: walking, jumping and collision work well (no verification record yet).
- **Phase 4 · One Vault Hunter, proven.** Spawn, fight, loot a gun, equip it, use a skill, complete one mission, die, respawn, on Sanctuary. About 3,800 of Gearbox's undocumented functions have to be worked out by analysing the executable and watching the game. This is the mountain, and it is the vertical slice. In progress: the slice pieces run in the host (Maya, Phaselock, the Fire mission, guns, pickups, the range, down and respawn, vending). One scripted run of the loop (2026-11-04) passed 10 of 14 links. Not yet compared with the actual game.
- **Phase 5 · Fill it out.** The remaining ~79 maps and 5 Vault Hunters, plus the whole campaign with your real save file. Not started.
- **Phase 6 · Beyond.** Co-op, DLC, The Pre-Sequel, mods, level editor. Not started.

I work on this actively, and I'm not promising completion dates. The project started on 2026-09-10. The dated milestones are in [ROADMAP.md](ROADMAP.md#how-its-going). Phase 4 is the mountain: it's the unproven part, and where most of the effort goes.

## Related project: BL2_RTSE

I also make [BL2_RTSE](https://github.com/zuhuHix/Bl2_RTSE), a mod for the real, installed Borderlands 2 that reads and hooks the running game through the Python SDK. I use it as a ground-truth tool here: when I want to know what the game does (a value, the order of events, what a native returns), I look at the live game first and only then decode packages or read native code. Anything it dumps stays on my machine and never goes in this repository.

## Help me keep going

This is a solo project, full stop. I live in Portugal and make about €1k a month, so spending a tenth of my salary on this every month just isn't realistic. AI does most of the decoding grunt work (I run a couple of ChatGPT setups I call Astra and Luna) and it's genuinely fast and good at this, that's how we got this far this quickly. But I'm on a €20/month subscription, and I can't just throw a chunk of my income at a passion project.

Borderlands 2 has always been my favourite game. My childhood game. This project is really close to my heart and I mean that sincerely. You can follow along and expect weekly updates, assuming I haven't burned through a month's AI usage in the first three days of the week. Which, uh, happened this week.

If you want to help me spend way more time on this (a better subscription, maybe eventually a game dev coach who can point me in the right direction) there's a Buy Me a Coffee link below. I also have a Patreon, but that one's more tied to my YouTube and TikTok stuff.

I'm not sure anyone actually reads this far, but if you did, thank you for checking the project out. I'm open to any constructive criticism, and if you feel like sending a PR, go for it.

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-yellow)](https://buymeacoffee.com/zuhu)

## Is this legal?

Yes. I take it seriously. The rules are the same as [OpenMW](https://openmw.org/) (Morrowind) and [OpenRCT2](https://openrct2.org/) (RollerCoaster Tycoon 2):

1. **I never share game files.** Not a texture, not a sound. Nothing extracted from the game *ever* enters this repository. Extracted assets live locally during import but stay git-ignored.
2. **No leaked source, and no code copied from the game's binary.** I read file formats, watch what the real game does, and (since 2026-10-01) analyse the game executable on my own machine to understand how its native code behaves. Decompiler output and analysis databases are game-derived data and stay out of this repository; the code in it is written from understanding, not from listings. I'm not a lawyer and none of this is legal advice; the risks are spelled out in [docs/LEGAL.md](docs/LEGAL.md).
3. **You need your own copy of BL2.** The engine refuses to start without it, and never modifies your install.
4. **I don't make money from this.** No paid builds, no premium anything. Ever. It's a passion project.
5. **Screenshots.** The git tree never holds screenshots of loaded game content. A small set of comparison screenshots, reviewed before upload, is published as assets on the `progress-gallery` prerelease, outside the tree. No game files or extracted assets are ever published, and any gallery asset is removed at the rights holder's request. This states what the project does, not a legal conclusion. Full scope: [docs/LEGAL.md](docs/LEGAL.md).

Full policy: [docs/LEGAL.md](docs/LEGAL.md). License: [MIT](LICENSE). Covers my code, nothing else.

<details>
<summary><b>FAQ</b></summary>
<br>

**Can I play Borderlands 2 in this?**
Not yet. You can fly around three maps (Phase 1), and the developer slice on Sanctuary lets Maya shoot stand-ins on a range, pick up a gun and run one mission. That's a test harness, not the game. Shooting real enemies in the campaign is Phase 4. The full campaign is Phase 5. It's gonna be a while...

**Is this a remaster? A remake?**
Neither. A remaster re-does the *stuff* (new textures, new models). A remake rebuilds everything from scratch. This keeps the original stuff untouched and replaces only the *machine* that runs it. Remasters will follow later on when people enjoy their new modding capabilities.

**Will my mods work?**
That's the plan. Text mods edit the same game data this loads, so they should carry over once gameplay runs (Phase 4–5). SDK mods will need a compatibility layer later. It won't be hard and most mods will keep working.

**Will my saves work?**
Yes. Reading real save files is a Phase 5 task. The save format is already documented by the community.

**Why Unreal Engine 5?**
Borderlands 2 is Unreal Engine *3*. UE3's systems (materials, animation, particles, cutscenes, scripting) all have direct descendants in UE5, so we translate into them instead of inventing replacements. That's a much smaller problem.

**Who's making this?**
Me and AI, ChatGPT (Astra and Luna), Claude aswell (opus 5.5 and sonnet 5.5) they do the grunt work: decoding formats, chasing down smaller one-off tasks. If you want to help cover my monthly sub, or just support the hours going into this, there's a [Buy Me a Coffee](https://buymeacoffee.com/zuhu), Thank you so much to my current supporter (George) ! <3

**Why "ReBased"?**
Come on guys, it's obvious hahaha. (The code still uses the working title *OpenWillow* in identifiers like `ow-package`; "Willow" is Gearbox's internal name for the BL2 engine.)

**What if it fails?**
I don't plan to stop ([the plan's commitment](docs/OPENWILLOW_ENGINE_PLAN.md#9-commitment)). If it ever does fail, the repo will say so, and the research and tools stay useful to the modding community.

but to be fair like cmon, there is no way this fails and I'll push through the whole way. also with newer and better AIs it's gonna get faster and easier everyday ! 

</details>

---

## For developers

Everything below is the technical layer. Click to expand.

<details>
<summary><b>How it works: architecture</b></summary>
<br>

Measured directly from the installed game: 20,119 functions across the nine code packages. **12,978 (64.5%) are UnrealScript bytecode** and will run in our VM as-is. **7,141 (35.5%) were native C++** inside `Borderlands2.exe` and must be rebuilt. Of those, 286 are trivial builtins, 609 are online/save/DLC plumbing we replace rather than replicate, 512 bridge the Scaleform UI, 1,914 are stock UE3 natives whose contracts are public via UDK, and **3,803 are Gearbox's own, undocumented**. Full breakdown: [engine plan §0](docs/OPENWILLOW_ENGINE_PLAN.md#0-ground-truth-the-numbers-this-plan-rests-on).

Three layers. The script VM and the asset pipeline are engine-agnostic C++; the native layer is where the host engine shows up.

```
                 ┌──────────────────────────────────────────────────────┐
                 │  HOST ENGINE (UE5: renderer, physics, audio, UI)     │
                 └───────────────▲──────────────────────▲───────────────┘
                                 │                      │
   ┌─────────────────────────────┴───┐    ┌─────────────┴──────────────────┐
   │  NATIVE LAYER (C++)             │    │  ASSET PIPELINE  ◄── Phase 1   │
   │  the 7,141 rebuilt functions    │    │  package loader (UPK/TFC)  [done] │
   │  Actor/Pawn/Controller, traces, │    │  textures, static meshes   [done] │
   │  movement, animation, particles,│    │  materials (approximation) [wip]  │
   │  AI, stat core, weapons, UI     │    │  levels (actors+transforms)[wip]  │
   └─────────────────────────────▲───┘    │  skeletal, anim, lightmaps,    │
                                 │        │  Kismet, Wwise, Bink, SWF  [todo] │
   ┌─────────────────────────────┴───┐    └────────────────────────────────┘
   │  UNREALSCRIPT VM  ◄── Phase 2   │
   │  UObject model, bytecode        │
   │  interpreter, states, latents,  │
   │  native dispatch table          │
   └─────────────────────────────────┘
        runs the 12,978 inherited functions unchanged
```

The host engine is Unreal Engine 5, decided and locked at the Phase 0 gate; the plan rules out switching later. UE3's material graphs, AnimTrees, Cascade, Matinee and Kismet all have direct UE5 descendants to translate *into*, which is the whole reason it won out; all Phase 1 work targets UE 5.8. Full reasoning, and the alternative considered (Godot), is in [engine plan §2.1](docs/OPENWILLOW_ENGINE_PLAN.md#21-host-engine-decision-decide-by-end-of-phase-0-never-after).

</details>

<details>
<summary><b>How we build it: methodology</b></summary>
<br>

Analyse freely, publish only our own code. File formats, observed behaviour and (since 2026-10-01) analysis of the game executable are all fair sources for *understanding* what the game does. Raw decompiler output and analysis databases stay local, because they're game-derived data, and the code in this repo is written from our own notes, never pasted or converted from a listing. No leaked source. When a public reference implementation (UE Viewer, UDK headers) helps pin down a serialization *order*, we read it (never copy from it) and log the source and its license in [THIRD_PARTY.md](THIRD_PARTY.md). The practical side, including how to share local analysis between two machines, is in [docs/NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md).

Every rebuilt native function needs a definition of "correct," and there are three we'll accept. **UDK**, a free running copy of UE3, covers the 1,914 stock natives. **The original game, instrumented** with [unrealsdk](https://github.com/bl-sdk) confirms the rest: hook a function, log what goes in and out during real play, then implement until our engine reproduces that log. **Community documentation** (the BLCM wiki, bl2.parts, Lootlemon) fills in stat math the other two don't reach. Reading the native function itself is now the fastest way to *find* the rule; it doesn't replace the check. A rule read from native code and not yet confirmed by one of the three stays labelled `UNVERIFIED` until it earns that label removed.

That standard isn't just for natives. Every change in this repo ends in a check run against the real game, and the check gets written down: synthetic tests in CI, differential checks against a real install, visual checks by an actual person looking at the screen. [DECISIONS.md](DECISIONS.md) is the record of all of it: every architectural choice, what got verified, and what didn't.

It's a one-person project, and I'm not quiet about the tooling. I own every architectural call, every verification and every license or provenance decision myself, but day to day, ChatGPT (Astra and Luna) handles format decoding and smaller one-off tasks, and Claude handles keeping the project organized plus last-resort coding once I've burned through a month's usage on my plan. Both operate under a written brief ([CLAUDE.md](CLAUDE.md)) and guard hooks in [`.claude/`](.claude/). Since 2026-09-30 an assistant may edit the package reader, container code and `CMakeLists.txt` and must then name the change and what is verified in its final message; the hooks still prompt me for license, dependency and provenance files and block game-file and decompiler-database writes.

</details>

<details>
<summary><b>Build and run it</b></summary>
<br>

You need: Windows, CMake, Visual Studio 2022 C++ build tools, Python 3, and an installed Borderlands 2. Unreal Engine 5.8 is only needed for the map viewer and the in-engine slice.

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure          # synthetic tests, no game needed
python tools/verify_packages.py --reader build/Release/ow-package.exe   # needs the game
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

Inspecting properties, running the full-install census, extracting a texture or mesh, preparing and opening a map in the UE5 viewer: [docs/TOOLING.md](docs/TOOLING.md). Everything the tools produce lands under `local/`, which is git-ignored, so extracted assets never enter the tree. The current pass/fail baseline for each gate is in [docs/verification/GATES.md](docs/verification/GATES.md).

The code keeps the working-title prefixes from before the rename: the reader is `ow-package`, the CMake option is `OPENWILLOW_LZO`, the UE host reads `OPENWILLOW_BL2`.

</details>

<details>
<summary><b>What's implemented right now</b></summary>
<br>

A standalone x64 C++20 tool, `ow-package`, reads version 832/46 packages: name/import/export tables; fully and partially LZO-compressed containers; object records with outer paths; a `PackageStore` that indexes an install lazily and resolves imports across packages; per-class export census; tagged properties (scalars, object refs, nested and fixed-layout structs, arrays via a schema file); resident `Texture2D` mips to PNG (DXT1/DXT5/A8R8G8B8, inline or TFC-streamed); all render LODs of a `StaticMesh` and one LOD to OBJ; bulk scene metadata and bounded payload bytes for the level preparer.

`tools/prepare_level.py` follows a map's serialized sublevel references, extracts reusable mesh sections and four-channel materials, and writes a manifest. `host/ue5/` is a minimal UE5 C++ project plus editor-Python importer/verifier that builds the scene, an inspection lighting rig and a free-flight spectator pawn, then reopens the saved scene and verifies it.

The Sanctuary slice lives in `host/`: Maya's movement, state, HUD, guns, pickups, Phaselock, the gun range, the Fire mission, the skills page and the gun vending machines' stock and currency, with labelled stand-ins and unverified rules where the records say so. The ammo and health vending is on `main` (#99); lootables are an open PR (#105). The slice gate (`tools/test_slice_gate.ps1`) runs its 14 checks in sequence and reports any link whose piece the host lacks as NOT_AVAILABLE.

Not implemented: runtime package streaming, other pixel formats, lightmaps, material graph translation, full UE3 collision parity, a general native layer, saves, and enemies that attack Maya in the main slice. (Terrain and BSP are imported for Sanctuary only; skeletal meshes come in through UModel for Maya, Marcus and the dummy.)

Full detail and what each check does and does not prove: [docs/TOOLING.md](docs/TOOLING.md).

</details>

<details>
<summary><b>All the documents</b></summary>
<br>

- [ROADMAP.md](ROADMAP.md): checkbox-level tracker: done, next, blocked
- [DECISIONS.md](DECISIONS.md): dated log of every architectural and parsing decision and its evidence
- [docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md): the plan: numbers, architecture, sources of truth, phases, commitment
- [docs/BL2_REMASTER_ANALYSIS.md](docs/BL2_REMASTER_ANALYSIS.md): research: what players actually want, what modding can and cannot reach
- [docs/TOOLING.md](docs/TOOLING.md): every tool, flag and command, with what each check proves
- [docs/verification/](docs/verification/): dated verification records for each shipped slice
- [docs/verification/SLICE_GATE.md](docs/verification/SLICE_GATE.md): the Phase 4 loop, run end to end, link by link
- [docs/verification/GATES.md](docs/verification/GATES.md): gate commands and the current baseline
- [docs/LEGAL.md](docs/LEGAL.md): working rules (analysis allowed, no game data or transcribed code in the repo), non-affiliation, contributor certification, license
- [docs/NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md): analysing the game executable, and sharing local analysis between two machines
- [THIRD_PARTY.md](THIRD_PARTY.md): dependency and reference provenance
- [CONTRIBUTING.md](CONTRIBUTING.md): how to contribute, and the sensitive areas
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md): Contributor Covenant 2.1
- [research/](research/README.md): the community-demand corpus and the original Python package reader

</details>

---

<div align="center">
<sub>BL2_ReBased is an independent fan project. Borderlands and related marks are trademarks of their respective owners. Not affiliated with, endorsed by or supported by Gearbox Software, 2K Games or Take-Two Interactive. Promotional artwork is used for identification only. Formerly "OpenWillow".</sub>
</div>
