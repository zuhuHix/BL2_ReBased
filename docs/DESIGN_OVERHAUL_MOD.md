# BL2: RECLAMATION: overhaul mod design proposal

Working title. A concept design grounded in `BL2_REMASTER_ANALYSIS.md`. Every feature below is tagged with the
technical layer it needs and whether that layer is **proven**, **likely**, or **unproven**.

---

## 0. The unifying idea (why this mod exists)

Content mods fail when they're a pile of guns. The successful ones (Exodus, Reborn) still read as "vanilla plus stuff."
To feel *new*, the mod needs a spine.

> **Atlas is coming back to Pandora, and it brought a competitor.**

Atlas is canonically the fallen megacorp, crushed between BL1 and BL2, its guns reduced to ultra-rare relics.
That's a gift: a **lore-legitimate reason** to introduce an entire new manufacturer, a new enemy faction, new bosses,
and a new reason to re-enter old areas. Nothing has to be justified as "a mod added this." The corporate war *is* the
frame, and BL2's satire register is exactly the right home for it.

This matters practically too: a narrative frame lets you re-theme **existing maps** (which is what the engine permits)
instead of needing **new maps** (which it doesn't). See §6.

**Design pillars:**
1. **Every manufacturer should change how you play, not just your numbers.**
2. **Reward mastery, not meta-compliance.** The OP-level complaint in the analysis is that builds *narrow*. Invert it.
3. **Additive, not destructive.** Exodus's key lesson: add gear, don't replace it. Preserves vanilla muscle memory.
4. **Modular.** BL2Fix's lesson: every subsystem toggleable. Also the honest answer to "would a remaster ruin it?",
   which was a live fear in the harvested threads.

---

## 1. Feasibility legend

| Tag | Meaning |
|---|---|
| **[DATA]** | Text/hotfix mods, object property edits. Proven, cheap, the bread and butter. |
| **[SDK]** | PythonSDK runtime hooks. Proven for game logic, UI, loot, stats, events. |
| **[ART]** | Texture/mesh/material replacement via UPK Explorer + TFC Installer. Proven but memory-budgeted. |
| **[POP]** | Map population edits, proven: `More Loaders On Pandora`, `Enemy_Randomizer`, `4xspawns`, `No Spawn Limit`. |
| **[?]** | **Unproven, needs a prototype spike before it's promised.** |
| **[X]** | Blocked. Don't attempt. |

Hard blockers from the analysis, restated so nothing below violates them:
**no re-baked lighting, no renderer change, no 64-bit, no netcode replacement.**
New geometry is *not* a hard blocker; it's an unproven long-shot with concrete routes; see §6.1 and §11.
The design below assumes it fails, so nothing depends on it.

---

## 2. New manufacturers

The single highest value-per-effort feature in the whole proposal. A manufacturer in BL2 is largely **data**: part
lists, stat modifiers, behaviours, plus a material/texture set and a card. You get an enormous identity payoff from
mostly-[DATA] + [ART] work.

Three new brands, each with a mechanical hook, not just a stat curve.

### 2.1 ATLAS (Reclaimed): *"Precision, restored."*
The prestige brand. Clean white/gold/teal panels against Pandora's rust, instantly readable as foreign.

**Hook: Mark & Converge.** Landing a critical hit applies **Mark** to the target (a few seconds).
- Marked enemies take bonus damage from *all* sources, so Atlas is a **co-op enabler**, not a solo brand.
- Reloading an Atlas weapon while an enemy is Marked releases a homing micro-volley.

**Identity:** low mag, high per-shot value, punishes spray. The anti-Bandit.
**Feasibility:** Mark as a debuff/status is **[DATA]/[SDK]** (status effects and on-crit behaviours exist).
Homing projectile behaviour is **[?]**. Spike it early; if steering isn't available, fall back to a
delayed micro-explosion at the Mark's location, which is definitely achievable.

### 2.2 VERITY DYNAMICS: *"Your warranty is your weapon."*
The satire brand, and the most genuinely novel thing here. Verity doesn't sell guns; it **licenses** them.

**Hook: Licensing.** Every Verity weapon carries a licence with a duration measured in *kills*.
- **Licensed:** exceptional, comfortably above-curve.
- **Lapsed:** the gun visibly degrades. Fire rate sags, the card turns red, the gun starts *complaining at you.*
- **Renew** at any vending machine for cash, scaling with weapon level.

This does three things at once: it makes money meaningful in the late game (a known dead resource in BL2), it
creates a genuinely new decision loop, and it is a perfect BL2-flavoured joke about live-service monetisation.
Torgue would have opinions.

**Feasibility: [SDK]**; needs kill tracking, per-item persistent state, card rendering, vendor UI hook.
All within demonstrated SDK scope (`Bank & Stash Anywhere`, `Storage Manager` prove item-state + UI work).
Budget this as the most engineering-heavy manufacturer. **Ship it toggleable**; some players will hate it,
and that's fine, it should be a spice not a tax.

### 2.3 ANSHIN: *"You will be fine."*
Anshin already exists in BL2 as a **shield/health** brand, so the logo, palette and voice are established, 
you're extending a brand, not inventing one. Cheapest art burden of the three.

**Hook: Sustain.** Anshin guns trade raw damage for leech, regen and team utility. Damage dealt heals you and
nearby allies. Anshin weapons cannot crit; they're *maintenance equipment*.

This directly serves the strongest positive signal in the whole analysis: **co-op is why people love BL2**
(co-op appears in 24.6% of negative reviews precisely *because* people are desperate to play together).
A support archetype gives a fourth player a real job.

**Feasibility: [DATA]** almost entirely, transfusion/leech behaviours already exist in vanilla.

### 2.4 Optional fourth: PANGOLIN
Also an existing shield brand. **Hook: Bulwark.** ADS deploys a personal overshield that decays while firing.
Ship only if the first three land. **[DATA]/[SDK]**

---

## 3. New guns & weapon changes

### 3.1 New guns: target ~90–120 pieces
Exodus's 130 is the proven ceiling; don't exceed it on v1.

**Rule: additive.** New guns occupy new slots in drop pools. Nothing vanilla is deleted. (Exodus's discipline, and
the reason players trust it.)

Split roughly:
- **~45** new-manufacturer commons/uniques establishing each brand's feel
- **~25** legendaries with *build-defining* effects, not stat sticks
- **~15** boss-specific drops (§5), each teaching the boss's mechanic
- **~10** "reclaimed relics": Atlas prototypes as the mod's chase tier
- **~15** re-costumed vanilla weapons for the new zone's loot table

### 3.2 Weapon feel fixes: straight from the harvested complaints
These were specific, repeated and actionable in the research:

| Complaint (verbatim from harvest) | Fix | Tag |
|---|---|---|
| Jakobs recoil "taking 1–2 seconds before they start recovering" | Rebuild recoil recovery curves per-manufacturer | [DATA] |
| "Starting guns suck and feel really bad" | Lift the level 1–15 floor; raise white/green base stats | [DATA] |
| Pearls and many uniques are underpowered | Pass over every pearlescent and dead legendary | [DATA] |
| Melee doesn't scale into OP levels | Melee scaling curve rework | [DATA] |

That table is the cheapest goodwill in the entire document. It is all data edits, and every line traces to a real
recurring player complaint rather than designer taste.

---

## 4. Rebalance: the part that decides whether this is loved or ignored

The analysis was blunt: **slag dependency and OP levels are the loudest design complaints in BL2 discourse.**
The recurring phrasing was that OP levels "limit build diversity a lot, you are forced to play the meta builds."

### 4.1 Slag: reframe, don't delete
Deleting slag would break a decade of builds and is exactly the "remaster ruins it" fear. Instead:
- Slag stays best-in-class as a **damage multiplier**, but stops being the **price of admission**.
- Raise non-slag damage floors in UVHM so unslagged play is *viable but suboptimal*.
- Give every character a non-weapon slag route so slagging isn't a mandatory gun slot.

Net: slag becomes a skill expression instead of a tax. **[DATA]**

### 4.2 OP levels: replace vertical grind with horizontal challenge
The complaint isn't difficulty, it's *narrowing*. So make the endgame widen instead:
- Replace flat OP scaling with **modifier sets** (a Mayhem-like, but authored rather than random).
- Modifiers change *how* you fight, not just enemy HP multipliers.
- Reward diverse loadouts explicitly, bonus rewards for clearing without repeating a manufacturer.

**[SDK]**: the Difficulty Modes and Enemy Level Randomizer mods in the SDK database prove the hooks exist.

### 4.3 Loot & grind
Grind/RNG showed up across every source. Targeted, not blanket:
- Dedicated drops get meaningfully better rates
- **Bad Luck Protection**: pity counter per boss **[SDK]**
- Re-roll a mission reward once **[SDK]** (BL2Fix precedent)

### 4.4 Quality of life: table stakes, ship all of it
Inventory/bank/UI was **8.7% of negative reviews**, tied for the largest *design* complaint.
Bank + backpack expansion, sort/filter, mark-as-junk/lock, dialogue and cutscene skip, fast-travel-from-anywhere.
**[SDK]/[DATA]**: all individually proven; mostly integration work.

---

## 5. New bosses

Exodus proved **custom bosses are achievable**. Method: existing skeletal meshes + new material/texture treatment
+ new behaviour scripting + new attack patterns + bespoke loot. **[DATA]/[SDK]/[ART]**

Four, each teaching one of the mod's new systems:

1. **The Auditor** *(Verity Dynamics)*: periodically **revokes your weapon licences** mid-fight, forcing you onto
   your unlicensed backup. Teaches: don't build a loadout you can't survive losing. Comedy: it bills you for the privilege.
2. **PROTOTYPE: CONVERGENCE** *(Atlas)*: marks *you*, and its own shots converge on the Mark. Teaches Mark
   mechanics by inverting them. Counterplay is movement and breaking line-of-sight.
3. **Matron of the Deep**: a genuine raid-tier fight for a re-themed cavern arena. Adds/phases, punishes tunnel vision.
4. **The Reclaimer**: final Atlas construct. Cycles through the mod's mechanics in phases as a graduation exam.

**Constraint:** no new skeletal meshes means silhouettes come from existing rigs. Re-texture, re-scale, re-script,
add particle signatures. That's the same technique Gearbox used for most vanilla bosses anyway.

---

## 6. A new zone: the honest answer

**There is no supported way to author new geometry** (the long-shot routes are in §11). BL2 PC ships `CookedPCConsole` with editor code stripped; both
`Binaries/Win32/Editor/` and `UserCode/` on the installed copy are **empty**. No level editor was ever released
despite a long-running community petition. Static lighting is Lightmass-baked into `Lighting.tfc`, so even if you
placed geometry you couldn't light it.

**What you can do is re-author an existing map into something that plays as new.** Proven at [POP] level:
mods already add enemy types never present in an area, change population distributions, and remove spawn limits.

### The right vehicle: Digistruct Peak (`TestingZone_P`)

Of the 37 base-game persistent levels plus DLC maps, this one is uniquely suited, and it's a genuinely elegant fit:

**Digistruct Peak is canonically a simulation.** It is an arena that *digistructs arbitrary content by design*.
Anything you stage there, new enemies, new factions, recombined assets, impossible mixes of Dragon Keep fantasy
props and Hyperion loaders, is **automatically lore-consistent**. No hand-waving required. The one map in the game
whose fiction actively licenses reuse.

**The zone: "The Reclamation Range".** Atlas has seized the digistruct facility to run live-fire trials of recovered
prototypes. You fight *through Atlas's simulation of its own comeback.*

What that gets you, all within proven layers:
- Wholly new enemy compositions and factions **[POP]**
- New objectives, waves, modifiers, escalation structure **[SDK]**
- New loot tables and vendors **[DATA]**
- New music/ambience *(Wwise repacking is **[?]** and poorly tooled; treat audio as stretch)*
- Re-textured props and palette shift **[ART]**

### 6.1 "Can we add map parts?": the precise answer

A BL2 map is not one file. `HyperionCity` on the installed copy is eight packages: `_P` (persistent baked geometry,
23 MB), `_Dynamic` (movable/interactive actors), `_Combat` (encounters/spawns), `_Light`, `_FX`, `_Audio`,
`_Skybox`, `_Px`. The `_P` geometry is untouchable; the `_Dynamic` and `_Combat` layers are where mods live.

| Tier | Capability | Status |
|---|---|---|
| 1 | **Edit existing map actors**: collision, location, rotation, visibility | **PROVEN.** Real mod: `set ResearchCenter_MissionMain.TheWorld:PersistentLevel.InterpActor_24 bCollideActors False`. Move walls, disable barriers, relocate platforms. |
| 2 | **Unlock fenced-off space**: out-of-bounds and cut areas behind blocking volumes | **PROVEN technique**; per-map survey required. Geometry already exists and is lit. |
| 3 | **Place existing meshes at runtime** as `DynamicSMActor`/`KActor` via SDK | **PLAUSIBLE, UNPROVEN.** Spawning actors is proven; using it as a level-building workflow isn't. Won't be lightmapped, dynamic lighting only, so it'll read slightly "off." **Phase 0 spike.** |
| 4 | **Author new geometry** | **NO.** No editor, no cooker. Packages are version 832 / licensee 46, Gearbox's serialisation; stock UDK output is not binary-compatible and has never been loaded. Baked lighting can't be regenerated. |

Net: *rearrange* yes, *unlock* yes, *place props* probably, *build new* no. A re-authored zone can therefore differ
**physically** from vanilla, not just in population, opened areas, removed barriers, moved platforms, possibly
runtime-placed cover.

**Secondary candidates** if you want more re-authored space: the slaughter domes
(`CreatureSlaughter_P`, `BanditSlaughter_P`, `RobotSlaughter_P`) are lore-justified arenas,
and `ThresherRaid_P` / `Boss_Volcano_P` are compact single-purpose maps.

**Be honest in the marketing.** Call it a *re-authored zone*, not a new map. The community knows the engine's
limits, Exodus is respected precisely because it never overclaimed. Promising "a new area" and delivering a
re-dressed one is the fastest way to lose trust.

---

## 7. What actually delivers "brand new experience"

Ranked by *experience delta per unit of effort*, this ordering is the real recommendation:

| # | Feature | Delta | Cost | Tag |
|---|---|---|---|---|
| 1 | **New manufacturers with mechanical hooks** | Very high | Medium | [DATA][ART][SDK] |
| 2 | **Endgame modifiers replacing OP grind** | Very high | Medium | [SDK] |
| 3 | **QoL bundle** | High | Low | [SDK][DATA] |
| 4 | **Weapon feel fixes** | High | **Very low** | [DATA] |
| 5 | **Re-authored zone + new bosses** | High | High | [POP][SDK][ART] |
| 6 | **Verity licensing system** | High (divisive) | High | [SDK] |
| 7 | Visual pass (curated textures + ReShade + DXVK) | Medium | Medium | [ART] |
| 8 | Slag/UVHM rework | Medium | Low | [DATA] |

**Rows 3 and 4 are nearly free and address the most-cited complaints in the entire research corpus.** Whatever else
happens, ship those. If the project stalls after that, it has already improved the game for most players.

---

## 8. Suggested build order

**Phase 0: Spikes (do this first, before promising anything publicly).**
Prototype the four **[?]** items: projectile homing, per-item persistent state for licensing, Wwise audio repacking,
and **runtime dynamic-mesh placement** (§6.1 tier 3, the one that decides how physically different the zone can be).
These are the only things that can invalidate the design. Two weekends of investigation saves a scrapped feature.

**Phase 1: Foundation.** Weapon feel fixes + QoL bundle. Shippable on its own. Builds an audience early.

**Phase 2: Identity.** Anshin ([DATA]-cheap) → Atlas → new guns. This is where it starts feeling like a new game.

**Phase 3: Endgame.** Modifier system, loot rework, bad-luck protection.

**Phase 4: Content.** Re-authored zone, bosses, Verity licensing.

**Phase 5: Presentation.** Curated ~300-texture budget, ReShade grade, DXVK, UI polish.

---

## 9. Risks

| Risk | Mitigation |
|---|---|
| **Multiplayer requires byte-identical mods** | Design single-player-first; document exact-match co-op. Unavoidable. |
| **~4 GB address ceiling** | Fixed texture budget from day one. Never ship alongside the full UHD pack. |
| **Scope death**: the #1 killer of overhaul mods | Phases 1–2 must be independently shippable. |
| **Take-Two is litigious** (re3/reVC precedent) | Ship *modifications*, never redistribute game assets. Standard mod distribution is well-tolerated; asset redistribution is what draws fire. |
| **"Don't ruin BL2"** sentiment | Everything toggleable. Vanilla+ preset on install. |
| **SHiFT/online instability breaks co-op anyway** | Out of your control: [X]. Don't let it block single-player work. |

---

## 10. Summary

The engine forbids exactly three things you might have wanted: **new geometry, new lighting, a new renderer.**
Everything else, new manufacturers with real mechanical identity, ~100 new guns, a rebuilt endgame, custom bosses,
and a re-authored zone with a lore-airtight excuse for existing, is inside proven territory.

The strongest version of this mod isn't the one with the most guns. It's the one where **Atlas's return gives every
new system a reason to exist**, and where the two cheapest features in the document, weapon feel and QoL, quietly
fix the complaints players have actually been repeating for a decade.

---

## 11. "Is the source code available?": and the real routes to new maps

### 11.1 What "the code" means, and its availability

| What | Status |
|---|---|
| UE3 engine C++ source | Licensee-only; never public. UDK = binaries + UnrealScript only. |
| BL2 game source (Gearbox Willow branch) | Never released. No known leak. **Do not pursue even if one surfaces**: undistributable, taints contributors, and Take-Two sued re3/reVC fans for less. |
| UnrealScript (game-logic layer) | **Legitimately available**: ships as bytecode in the UPKs; UE Explorer decompiles ~all of WillowGame/Core/Engine (1,426 classes). Not C++, not the editor. |
| The exe as RE target | No PDB shipped **[verified locally]**. unrealsdk already reversed object layouts / ProcessEvent hooks. |

### 11.2 Can the editor be unlocked like BL1's?: NO **[verified locally]**
BL1's Dr. Zed patch worked because BL1's exe still contained the editor. A string scan of `Borderlands2.exe`
(ASCII + UTF-16) finds only name-table *references* (`UnrealEd.EditorEngine` ×1, `WillowEditor.*` ×3) and **zero**
hits for the actual editor classes: `UEditorEngine`, `EditorFrame`, `GenericBrowser`, `LevelEditor`,
`BuildLighting`, `CSG`, `BrushBuilder`, `GeometryMode`. The editor is compiled out. Nothing to unlock.
(Incidental: build path `f:\BAMBOO-TAMBOTI2-BL2-STEAM\Development\Src\D3D9Drv\` confirms stock UE3 src layout and
D3D9Drv as the sole renderer module.)

### 11.3 The precedent: you don't need source, you need a package-format toolkit
**Mass Effect Legendary Explorer** (Package Editor, Pathfinding Editor (place StaticMeshes/splines/cover),
Live Level Editor) is a full level-editing toolkit for a UE3 game built with zero source access, purely by reversing
the cooked package format. Legal (interoperability RE), tolerated, and the template for anything here.

### 11.4 Routes, ranked

| Route | Idea | Verdict |
|---|---|---|
| **A. Rocket League-style** | Author in a mid-2011 UDK (package version **832**: verify via its `Engine.upk` header), drop `.udk` into `CookedPCConsole`, `Open <Map>` from console. RL does exactly this because Psyonix ≈ UDK 2013 with minimal licensee changes. | **1-day spike.** Likely fails on LicenseeVer 46 serialisation, but if it loads even partially it's transformative. |
| **B. LEX-for-Willow** | Reverse the *write* side of the level format: clone exports, extend actor lists, fix name/import tables; compose "new" levels from the existing mesh library inside a cloned `_P`. | **Proven category, no source needed, years of work.** Read side already largely done (UE Explorer, umodel, UPK Explorer, unrealsdk). |
| **C. SDK runtime placement** | Place dynamic mesh actors from a JSON layout at map load. No file-format work. | **Easiest.** No lightmaps (dynamic lighting only). = §6.1 tier 3. |
| **D. Patch exe to re-enable editor** | n/a | **Dead** (§11.2). |

**Pragmatic plan: A → C.** UDK doesn't know Willow gameplay classes (population nodes, fast-travel, player starts),
so even a successful Route A yields bare geometry, inject the gameplay layer via SDK. If A fails, C alone still
delivers physically recomposed spaces inside existing maps.

**Legal line:** clean-room RE of file formats for interoperability (what LEX, umodel, unrealsdk all do) is the
tolerated side. Redistributing Gearbox assets or touching leaked source is the side that gets sued.

---

## 12. Measured: how much of BL2 an engine reimplementation would inherit vs rebuild

Measured directly from the installed packages with `research/native_count.py` (custom LZO decompressor + UE3
package reader; per-class data in `research/native_by_class.json`). Zero unparsed functions.

```
20,119 functions in Core/Engine/GameFramework/GearboxFramework/WillowGame/GFxUI/IpDrv/OnlineSubsystem/AkAudio
  12,978  64.5%  UnrealScript bytecode  -> INHERITED, executed by a script VM as-is
   7,141  35.5%  native (FUNC_Native)   -> body lived in Borderlands2.exe, must be rebuilt
   2,453         script "events" called *from* native code (timing contract must be honoured)
```

| Bucket of the 7,141 natives | Count | Share | Difficulty |
|---|---|---|---|
| Core builtins (operators, math, strings) | 286 | 4.0% | Trivial |
| Online / SHiFT / save / DLC / platform | 609 | 8.5% | Replace with own systems, don't replicate |
| Scaleform GFx (Flash UI) bridge | 512 | 7.2% | Maps onto a SWF player |
| Wwise audio bridge | 17 | 0.2% | Maps onto an audio backend |
| Stock UE3 engine natives (253 classes) | 1,914 | 26.8% | Contracts public via UDK `.uc` headers |
| **Gearbox-specific natives (443 classes)** | **3,803** | **53.3%** | **Undocumented: the true RE burden** |

Concentration: top 50 classes hold 51.8% of natives; top 200 hold 78.5%. Biggest: `WillowPlayerController` 298,
`WillowPawn` 246, `WillowVehicle` 156, `WillowInteractiveObject` 134, `WillowAIPawn` 121, `WillowWeapon` 121.

Key systems:
- **Behavior system: 302 `Behavior_*` classes, 268 pure UnrealScript → inherited.** Only 34 have natives (55 fns).
- **Stat/skill/item core is fully native, zero script**: `SkillDefinition` 44, `AttributeDefinition*` family,
  `WeaponPartDefinition` 8, `InventoryBalanceDefinition` 7, ~120 functions total. Small, dense, and the math is
  largely documented by the modding community. (This is the "Money Shot feels weaker" failure surface.)
- **Animation: data inherited, player rebuilt.** `SkeletalMeshComponent` 127 natives; every `AnimNode*` is native
  with zero script (~30 node types). No animation is re-authored.

Net: an engine reimplementation is *rebuild ~1,900 documented + ~3,800 undocumented natives concentrated in ~200
classes, while 64.5% of gameplay logic runs from the files.* Still years, but measurable ones, with a coherent M3:
`WillowPawn` + `WillowPlayerController` + `WillowWeapon` + the ~120-function stat core = a character that walks and
shoots with real guns.
