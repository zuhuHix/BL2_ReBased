# Borderlands 2 — Deep Analysis, Community Demand, and Remaster/Overhaul Feasibility

**Compiled:** 2026-09-09
**Scope:** (1) why BL2 is loved, (2) what people actually want from a "BL2 Remastered", (3) whether a full overhaul — including graphics — is technically achievable.

---

## 0. Executive verdict (read this first)

Three findings drive everything below.

**1. The thing people love about BL2 is not renderable.** Across every unbiased data source, praise clusters on *loot chase, writing, character builds, DLC, and co-op* — not visuals. "Art style holds up" appears in only ~2–3% of reviews, and almost always as *praise for the existing look*. A graphics-first remaster targets the least-requested axis.

**2. The #1 live grievance is that co-op is currently broken.** In BL2's negative Steam reviews, **24.6% mention co-op and 14.3% explicitly describe multiplayer as broken** — the single largest complaint cluster by a wide margin, alongside SHiFT/ToS/ads (13.5%), crashes (10.3%) and UI (8.7%). This is a *2025–2026 regression* caused by the SHiFT matchmaking migration, not a 2012 design flaw. Anyone building "BL2 Remastered" is competing with a game whose most-cited defect is a backend Gearbox broke recently.

**3. A true graphical overhaul is blocked by one specific thing: you cannot re-bake the lighting.** Everything else — textures, materials, post-processing, UI, meshes, gameplay, new gear, new bosses — is achievable with existing tooling. But BL2 ships as `CookedPCConsole` with the editor **stripped** (I verified: `Binaries/Win32/Editor/` exists and is **empty**), its lighting is pre-baked Lightmass data in a 154 MB `Lighting.tfc`, and there is no path to re-run Lightmass without the editor and uncooked source assets. You can make BL2 *sharper and better graded*. You cannot make it *re-lit*.

**Bottom line:** A community "BL2 Remastered" is very feasible as a **systems + QoL + texture/post-process overhaul** — which is genuinely what the data says people want. A **full graphical remaster in-engine is not achievable by modding**, and a UE5 remake is both a total asset re-authoring project and the most legally exposed thing you could build, given Take-Two now owns Gearbox.

---

## 1. Methodology and data provenance

This is built on primary data I collected and can re-run, not on vibes. Scripts and raw data are in `research/`.

| Source | Method | n | Bias profile |
|---|---|---|---|
| Steam reviews — recent | Public `appreviews` API, chronological | 1,500 | **Unbiased sample** |
| Steam reviews — most helpful, all-time | Public `appreviews` API | 477 | Community-vote weighted |
| Steam reviews — negative only | Public `appreviews` API | 126 | Complaint corpus = de facto fix-list |
| Steam Discussions threads | Scraped listing pages | 247 | Current active topics |
| Reddit threads | Search-index harvest (titles + snippets) | 226 | **Query-seeded — see caveat** |
| BL3 / BL4 reviews | Public API | 1,242 | Comparison baseline |
| **Local BL2 install** | Direct file forensics, 26.5 GB | — | Ground truth |

**Caveat you must hold onto:** Reddit is blocked to automated crawlers, and the open mirrors sit behind proof-of-work anti-bot walls which I did not circumvent. I reached Reddit through a search index, which means **I chose the topics by choosing the queries**. Reddit percentages measure *"how much discourse exists on a topic I asked about"*, not *"how often people spontaneously raise it."* Treat Reddit as **qualitative texture** and Steam reviews as **quantitative signal**. Where the two agree, confidence is high.

### Headline numbers

| Game | Steam rating | Positive | Total reviews |
|---|---|---|---|
| **Borderlands 2** | Very Positive | 161,444 | **178,856 (90.3%)** |
| Borderlands 3 | Very Positive | 62,030 | 76,940 (80.6%) |
| Borderlands 4 | **Mixed** | 35,031 | **58,177 (60.2%)** |

BL2 currently runs ~2,100 average concurrent players with a ~3,600 24h peak — **up ~29.6% over 30 days** — fourteen years after release. All-time peak was 81,062.

That BL4 sits at *Mixed* while BL2 sits at *Very Positive* is the commercial argument for a remaster, and it is the most important number in this document.

---

## 2. PART I — Why Borderlands 2 is loved

### 2.1 What the data says, ranked

From the unbiased Steam corpora (% of reviews touching the theme):

| Theme | Recent (n=1500) | Most-helpful (n=477) |
|---|---|---|
| Loot chase / guns | 13.2% | 15.9% |
| Story | 8.5% | 9.6% |
| Co-op with friends | 6.4% | 9.0% |
| Writing / humour | 7.1% | 8.2% |
| Replayability / hundreds of hours | 5.4% | 8.0% |
| Characters / skill trees | 5.5% | 6.9% |
| DLC quality | 4.1% | 4.8% |
| Handsome Jack specifically | 3.5% | 3.4% |
| Art style | 2.3% | 3.1% |

### 2.2 The design analysis — *why* those things work

**a) The loot chase is tuned around a legible dopamine curve.**
BL2's procedural gun system layers manufacturer identity (Jakobs = high-crit, no elemental; Torgue = explosive gyrojets; Maliwan = elemental; Tediore = throw-to-reload; Vladof = fire rate; Hyperion = reverse recoil; Bandit = magazine size; Dahl = burst-on-ADS) on top of parts-based stat rolls. Two effects most looters miss: guns are *readable at a glance by silhouette and colour*, and rarity actually maps to a felt power delta. A player who sees orange in a loot pile has been trained, correctly, that something has changed. One of the most-upvoted reviews I pulled says exactly this, in cruder terms:

> "seeing the colour orange in a pile of loot after 10 hours of grinding makes me instantly start crying, nutting, and passing out from exhaustion all at once"

**b) Handsome Jack is structurally, not just tonally, well-built.**
Jack is not merely well-written and well-performed (Dameon Clarke); he is *architecturally* embedded. He is on the ECHO constantly, he reacts to your progress, he kills characters you have met, and — critically — he is *funny*, which makes him likeable, which makes his cruelty land. He is also the only Borderlands antagonist who is a real narrative counterweight to the player: he thinks he is the hero, and the game commits to that. Later entries substituted volume (the Calypso Twins) or absence. The reviews notice: Jack is named in 3.4–3.5% of *all* reviews, which for a single named character is extraordinary.

**c) Skill trees produce genuinely different games, not different numbers.**
Salvador's Gunzerking (dual-wield, and the infamous Grog/DPUH combos), Zer0's Deception/melee, Krieg's Bloodlust/Mania self-damage economy, Gaige's Anarchy stacking (deliberately trading accuracy for damage), Maya's Phaselock as crowd control *and* support node — these are not skins on a damage stat. The build culture (thousands of hours of theorycrafting, `bl2.parts`) exists because the trees support it. Harvested Reddit threads repeatedly frame BL1's trees as "really quite boring" by comparison.

**d) The DLC is, unusually, the best content in the game.**
*Tiny Tina's Assault on Dragon Keep* is widely treated as the high-water mark of the franchise's writing — a tabletop-framing device that turns a loot shooter into a grief narrative without dropping the comedy. *Captain Scarlett*, *Mr. Torgue* and *Hammerlock* are all substantial. Combined with the Season Pass structure, this is why "best value as a full package" recurs. 4–5% of reviews reference DLC unprompted.

**e) Co-op is the delivery mechanism for all of the above.**
Four-player drop-in, shared world, no instanced loot — which creates the arguments, the ninja-looting folklore, the social memory. The second-most-upvoted review in the entire all-time corpus is a joke about a friend stealing a gun. This matters enormously for the remaster question: **co-op is not a feature of BL2, it is the medium of BL2.**

**f) The art style is a technical decision that aged into an aesthetic advantage.**
The hand-inked, cel-shaded look was chosen partly to sidestep the fidelity race. The consequence in 2026 is that BL2 does not read as "an old game" the way a photoreal 2012 title does — it reads as *stylised*. This is why so few players ask for a graphical overhaul, and the strongest argument *against* making one the centrepiece.

### 2.3 The honest counter-case

BL2 is not universally adored, and the dissent is coherent. From harvested threads: gunplay recoil recovery is criticised (Jakobs weapons "taking 1–2 seconds before they start recovering"), early-game guns "feel really bad", movement is dated next to BL3's slide/mantle, and there is a real constituency arguing BL2 is their *least* favourite mainline entry. In negative reviews, "movement/gunplay feels dated" hits **8.7%**. One of the best-articulated positive reviews concedes the point directly:

> "Later entries have smoother gunplay, better movement, and plenty of quality-of-life improvements, but none of them put everything together quite as well… It is also undeniably janky. Movement feels old."

That sentence is the thesis of the entire remaster opportunity.

---

## 3. PART II — What people actually want in a "BL2 Remastered"

### 3.1 Cross-source frequency table

Percentage of items in each source touching each theme. **Reddit column is query-seeded — see §1.**

| Theme | St. recent | St. helpful | St. negative | Steam forum | Reddit |
|---|---|---|---|---|---|
| **C: multiplayer / connection BROKEN** | 1.8% | 3.4% | **14.3%** | 1.2% | 0.4% |
| **C: SHiFT / account / ToS / ads** | 1.9% | 3.6% | **13.5%** | 2.0% | 0.0% |
| **C: crashes / stability / perf** | 1.7% | 2.1% | **10.3%** | 6.1% | 4.9% |
| **C: inventory / bank / UI** | 1.9% | 2.9% | **8.7%** | 0.4% | 4.9% |
| **C: movement / gunplay dated** | 2.3% | 2.9% | **8.7%** | 0.8% | 5.8% |
| C: grind / RNG / drop rates | 2.0% | 2.5% | 4.0% | 0.4% | 5.8% |
| C: UVHM / OP levels / scaling | 2.5% | 3.1% | 3.2% | 2.8% | 12.4% |
| C: needs mods to be good | 1.7% | 2.3% | 2.4% | 2.4% | 14.6% |
| C: level cap / endgame | 1.0% | 2.1% | 0.0% | 2.4% | 6.6% |
| C: slag requirement | 0.5% | 0.6% | 0.0% | 0.0% | 4.0% |
| C: backtracking / fast travel | 0.5% | 1.0% | 0.8% | 0.0% | 0.9% |
| C: 32-bit / memory / UHD pack | 0.3% | 0.2% | 0.0% | 0.0% | 1.8% |
| C: FOV / ultrawide / resolution | 0.3% | 0.4% | 0.0% | 0.4% | 5.3% |
| C: wants remaster / remake | 0.3% | 0.4% | 0.0% | 0.4% | 9.3% |
| C: no PC split-screen | 0.1% | 0.0% | 0.0% | 0.0% | 0.0% |

*Stability note — this is the point of the §1 caveat, shown empirically. As the Reddit harvest grew 93 → 189 → 226 results, that column swung hard (UVHM/OP 23.7%→11.1%→12.4%; slag 9.7%→3.7%→4.0%; "needs mods" 22.6%→22.2%→14.6%) while the Steam columns never moved. **Read the Reddit column as evidence that a topic exists and what people say about it — never as a measure of how much they care.** The Steam review columns carry all the weight in this table.*

Independent corroboration from Steam Discussions: of 247 recent threads, **multiplayer/co-op/connection is 20.2%** — the largest single category, more than double the next (crashes/technical, 10.5%).

### 3.2 The demand list, ranked by evidence

#### Tier 1 — Overwhelming, load-bearing

**1. Make co-op work again.** Not a nice-to-have; the defining current complaint. From the highest-voted review in the corpus (+48):

> "This used to be one of my favorite games to play with friends. It used to work incredibly well… With the ToS updates have seemingly come some new backend changes that make it almost impossible to play with my friend… Changing network settings to switch from offline mode to friends only? The game hardlocks. Booting up the game and trying to load the title screen? 25% chance the game hardlocks."

And (+36): *"Shift is what they use for multiplayer and it is completely broken. Looping loading screens & glitches galore. Should've stayed with Steam's free servers."*
And (+11): *"Nearly impossible to play with friends in 2026 due to bugs, glitches, and the migration to the SHiFT servers."*
And (+2, 232h played): *"One of the best games of all time, that I will never be able to play again with my friends… I don't know how its possible to screw something up so royally after it had been functioning for literal decades."*

Sub-asks clustering here: reliable session creation, no forced re-linking, no disconnect-every-30-minutes, and repeatedly **"give us back direct/LAN/Steam-native connection as an option."**

**2. Kill the account/ToS/ads friction.** 13.5% of negatives. Players object to mandatory SHiFT linking on a single-player game, to the Take-Two ToS/data-collection change (there is a *pinned* Gearbox thread titled "Response to Recent Community Concerns About Take-Two's Terms of Service"), and — with real venom — to BL4 advertisements patched into a 2012 game. The most-cited line of the whole exercise:

> "if they can update the game to put in ads, then they can update the game to fix the problems that have persisted for years"

**3. Stability.** Crashes, fatal errors on join, out-of-video-memory. 10.3% of negatives, 10.5% of forum threads. Substantially traceable to the Ultra HD texture pack (see §4.2).

#### Tier 2 — Strong and consistent

**4. Inventory / bank / UI overhaul.** 8.7% of negatives — tied with movement as the largest *design* complaint in the unbiased corpus, and a persistent Reddit thread topic (4.9%). Concrete asks: more backpack and bank space, sorting and filtering, item comparison, mark-as-junk/lock (both of which *Borderlands GOTY Enhanced already added to BL1* — the precedent exists), and a less clumsy item card.

**5. Modernise movement and gunplay feel — carefully.** 8.7% of negatives. The asks are specific and modest: slide, mantle/climb, better ADS transitions, faster recoil recovery, BL3-grade gunfeel. Strong counter-current: a visible faction insists this would make it "not BL2." Recommended framing: **optional module, default off.**

**6. Rework UVHM / slag / OP levels.** The loudest *design* topic in Reddit discourse (12.4% of harvested threads — but this figure swung between 11% and 24% as the sample grew, so treat the magnitude as soft). What is *not* soft is the content: the complaint is unusually precise and repeats almost verbatim across independent threads.
- Slag becomes mandatory rather than tactical: *"Slag is so much better than everything else… it made getting no slag basically not an option"*; *"slag (the Pickle of Borderlands game mechanics) is essential."*
- OP levels compress build diversity: *"it limits the build diversity a lot, you are forced to play the meta builds"*; *"OP levels turned Borderlands into a solo game"*; *"OP levels aren't worth it."*
- Melee scaling breaks at OP: the single most-specific remaster ask I found was *"proper balancing and scaling for Melee damage in OP levels."*
- Underpowered gear tiers never got fixed: *"fix pearls and under powered weapons as well as revamping certain skills such as raving retribution."*

#### Tier 3 — Real but smaller

7. Level cap / endgame restructure (6.6% Reddit, 2.4% forum).
8. Drop rates and farming tedium.
9. Backtracking; fast-travel-from-anywhere (already a popular mod, "BSABT").
10. FOV slider, proper ultrawide, 4K, unlocked framerate.
11. Playable TPS/other Vault Hunters in the BL2 campaign — a recurring wish, e.g. *"I want to play the BL2 campaign as Athena."*
12. PC split-screen (essentially zero organic mentions — currently served by Nucleus Co-op / Universal Split Screen).

### 3.3 What people explicitly do *not* want

- **Graphics-first.** "Modern graphics" appears, but overwhelmingly as *"rerelease the game but with modern graphics and slightly improved gameplay"* — a garnish, not the ask. Meanwhile "art style holds up" is a *praise* category.
- **Gearbox touching it.** Widespread stated distrust that a Gearbox-made remaster would repeat BL3/BL4 mistakes. Several threads argue "if they were going to, they'd have done it for the 10th anniversary in 2022."
- **Changing the writing or the tone.** Zero appetite. This is the crown jewel and everyone knows it.
- **A remaster that breaks mod compatibility.** For a large slice of the PC playerbase, BL2 *is* modded BL2.

### 3.4 Precedent: Borderlands GOTY Enhanced (2019)

Gearbox already ran this exact experiment on BL1: higher-res textures, 4K/high-framerate support, improved lighting and character models, a minimap, always-visible objective waypoints, a BL2-style reworked inventory with **lock/junk flags and auto-pickup**, SHiFT support, Golden Chests, six new legendaries, character customisation — plus a re-tuned final boss "in direct response to fan feedback."

That is almost precisely the Tier 1–2 list above. **The template for what a BL2 remaster should be already exists, made by the same studio, and it was well received.** This is the strongest available argument that the community's ask is realistic rather than fantastical.

---

## 4. PART III — How BL2 modding actually works (with local file forensics)

I analysed the installed 26.5 GB copy directly rather than relying on write-ups. Verified facts are marked **[verified locally]**.

### 4.1 The engine, precisely

| Property | Value | How established |
|---|---|---|
| Engine | Unreal Engine 3, Gearbox "Willow2" branch | Config paths `WillowGame/`, `WillowEngine.ini` **[verified locally]** |
| Cooked package version | **832**, licensee version **46** | Parsed `HyperionCity_P.upk` header **[verified locally]** |
| Executable arch | **i386 / 32-bit**, `LARGE_ADDRESS_AWARE = YES` | Parsed PE header of `Borderlands2.exe` **[verified locally]** |
| Renderer | **Direct3D 9, Shader Model 3** | `RefShaderCache-PC-D3D-SM3.upk` (58 MB); no SM4/SM5 cache exists **[verified locally]** |
| Physics | PhysX 2.8-era + APEX (incl. `_Legacy` variants) | `PhysXLoader.dll`, `NxCharacter.dll`, `PhysXCore.dll`, `APEX_Clothing_x86.dll` **[verified locally]** |
| UI | Scaleform GFx (Flash/SWF) | Standard for this UE3 branch; `GFxUI.INT` localisation files present |
| Audio | Wwise | `Audio_Banks.pck` 306 MB, `Audio_Streaming.pck` 241 MB **[verified locally]** |
| Video | Bink (`.bik`) | incl. `MegaIntro_4K.bik`, 817 MB **[verified locally]** |
| Online layer | `bifrost.dll`, 8.5 MB | **[verified locally]** |
| **Editor** | **Stripped** | `Binaries/Win32/Editor/` and `UserCode/` exist and are **empty** **[verified locally]** |

Note on packages: map packages such as `HyperionCity_P.upk` report version 832/46, while core packages like `Startup.upk`, `Engine.upk` and `WillowGame.upk` report `0/2` — the UE3 *fully-compressed package* marker, i.e. they are stored wholly LZO-compressed and must be run through an Unreal Package Decompressor before inspection.

The `LARGE_ADDRESS_AWARE` flag deserves a callout: the community shorthand "BL2 is limited to 3.5 GB" is roughly right in effect, but the mechanism is that a 32-bit LAA process gets a **~4 GB user-mode address-space ceiling** on 64-bit Windows. That ceiling is the hard wall behind most "Ran out of video memory" crashes.

### 4.2 Data layout **[verified locally]**

```
WillowGame/CookedPCConsole/     4.1 GB   914 .upk packages
  Textures.tfc                  1066 MB  world texture streaming cache
  CharTextures.tfc               772 MB  character texture streaming cache
  Lighting.tfc                   154 MB  BAKED LIGHTMAPS  <-- the blocker
  RefShaderCache-PC-D3D-SM3.upk   58 MB  SM3 shader cache
  Audio_Banks.pck / Audio_Streaming.pck  Wwise
  HyperionCity_P.upk (24 MB), Interlude_P, PandoraPark_P, Ash_P, Caverns_P … (map packages)

DLC/                           19.4 GB
  Mancana/    9.2 GB  = Ultra HD Texture Pack
      Textures0..5_mancana.tfc  ~7.5 GB combined
      MegaIntro_4K.bik           817 MB
  Anemone/    3.7 GB  = Commander Lilith & the Fight for Sanctuary
  Aster/      1.6 GB  = Tiny Tina's Assault on Dragon Keep
  Orchid/     968 MB  = Captain Scarlett      Iris/ 922 MB = Mr. Torgue
  Sage/       705 MB  = Sir Hammerlock        Nasturtium/Allium/… = headhunter packs
```

Note the shape of that: **the Ultra HD pack alone is 9.2 GB, of which ~7.5 GB is streaming texture cache, fed into a process with a ~4 GB address ceiling.** That is the OOM crash stated as an equation. It is also the hardest constraint on any texture-based visual overhaul.

Relevant engine defaults **[verified locally]** from `DefaultEngine.ini`:
```
MaxShadowResolution=1024        MotionBlur=False        FogVolumes=False
AmbientOcclusion=True           PhysXLevel=0            ViewDistanceUltraHighScale=2.0
TEXTUREGROUP_Character / _Weapon : MaxLODSize=8192   (the UHD pack's headroom)
TEXTUREGROUP_World / _Lightmap   : MaxLODSize=4096
```
`MaxShadowResolution=1024` is a raisable config cap, but it will not give you modern shadowing — it is still SM3 shadow mapping.

### 4.3 The modding stack, in layers

**Layer 0 — Config.** `WillowEngine.ini` / `WillowGame.ini` tweaks. Free, safe, limited.

**Layer 1 — Console + text mods (the classic path).** Enable the console by hex-editing a decompressed `engine.upk` (BL2: replace `1B E9 47` → `1B A5 14`, then delete `engine.upk.uncompressed_size`). Mods are then `.txt` files of `set` / `SparkLevelPatchEntry` commands `exec`'d at the loading screen. Managed by **BLCMM / OpenBLCMM**. This layer can rebalance essentially *any* game-object property: weapon parts, drop pools, skills, enemy stats, loot weights.

**Layer 2 — PythonSDK (`unrealsdk` / `pyunrealsdk`).** The modern path. A `ddraw.dll` proxy in `Binaries/Win32` loads an embedded Python runtime binding directly to live Unreal objects, letting mods hook and call arbitrary game functions at runtime. Maintained under the `bl-sdk` GitHub org (`willow2-mod-manager`, primarily apple1417 and collaborators), with a companion `Live Object Explorer` reverse-engineering tool. **SDK mods are a strict superset of text mods.** Demonstrated ceiling, from the official mod database: new game modes (Roguelands, Mario Mode), loot/weapon/player randomisers, `Material Editor`, `Photo Mode`, `Audio Control`, `Bank & Stash Anywhere`, collision visualisers, movement tech, custom skins.

**Layer 3 — Asset replacement.** `UPK Explorer` (UE2/UE3) extracts and re-imports **textures, meshes, materials and audio**; `TFC Installer` injects the resulting packs while preserving UPK compression and backing up originals. Caveat from the tooling docs: new textures are **appended** rather than the TFC being rebuilt, so files bloat — Mass Effect's `MEM` has a proper TFC-rebuild path; BL2's tooling does not.

**Layer 4 — Injection / runtime.** ReShade for post-processing. **DXVK** (drop `d3d9.dll` from the x32 build into `Binaries/Win32`) translates D3D9→Vulkan and is widely reported to dramatically improve CPU-bound performance after shader-cache warm-up. Nucleus Co-op / Universal Split Screen for PC split-screen. `EasyHook32.dll` already ships with the game.

**Layer 5 — Hex patches.** `Borderlands Hex Multitool` raises level cap, backpack size, currency caps. Largely superseded by the SDK.

### 4.4 Proven large-scale overhauls (existence proofs)

| Project | Tech | Scale |
|---|---|---|
| **Unofficial Community Patch (UCP)** v5.0.3 | Text mods | Hundreds of bug/balance fixes; the de facto baseline |
| **Vanilla Enhanced** | Text mods | Bugfixes + cosmetics + QoL only, no balance changes |
| **BL2 Reborn** | Modpack (includes UCP) | **150+ mods, 750+ changes**; full character overhauls for Gaige and Maya, partial for Krieg/Salvador |
| **BL2 Exodus** | **PythonSDK** | **130+ entirely new gear pieces** (81 weapons, 49 items) that *add* rather than replace, plus custom bosses, new loot sources, altered elemental effects, expanded storage |
| **BL2Fix** | PythonSDK | Loot/XP/pacing/endgame rework; toggleable QoL (dialogue + cutscene skip, reward reroll, restored caps, unlimited bank) — fully modular via an in-game options menu |
| **UHD Texture Pack TFC Installer** | TFC injection | Installs a **~300-texture subset** of the official UHD pack specifically to dodge the memory ceiling |
| ReShade presets (Vivid Graphical Improvements, BetterLands, …) | Post-process | Lighting/shadow/colour/AA/faux-SSAO; the most popular "visual overhaul" route |

That last row is the important one. **The community already solved "how do we get better textures without crashing": be selective, not comprehensive.** Any texture overhaul must be built to a ~4 GB budget from day one.

### 4.5 What is genuinely blocked

**a) New maps, at scale — effectively blocked.** BL2 PC ships `CookedPCConsole`, which strips editor code. Some UDK-based custom-map workflows exist for *Borderlands 1* (and there are BL2 UDK tutorials plus a "Dr. Zed patch" claiming to unlock the editor), but there is no reliable, supported pipeline for authoring new BL2 levels and loading them in the retail game. A long-standing community petition asks Gearbox to release a BL2 level editor; it was never granted. This is why *Exodus* — the most ambitious content mod in existence — adds **gear, bosses and loot sources** rather than **areas**.

**b) Re-baked lighting — blocked.** Static lighting is Lightmass-baked into `Lighting.tfc` and per-map packages. Re-baking requires the editor and uncooked source assets. Neither is available. **You can grade the image; you cannot relight the world.**

**c) Renderer modernisation — blocked.** D3D9/SM3 is baked into the shipped binary and shader cache. Editing `.ini` files to "enable DX11" does nothing. There is no PBR, no deferred pipeline, no real-time GI. ReShade sits *after* the frame is rendered; it cannot add information the renderer never computed.

**d) 64-bit — blocked.** Requires recompiling from UE3 source. Only a licensee (Gearbox/Take-Two) can do it.

**e) Netcode replacement — blocked in practice.** The online layer lives in `bifrost.dll` and the shipped executable. Mods cannot replace SHiFT matchmaking — unfortunate, given it is the #1 complaint.

**f) Audio replacement — painful.** Wwise `.pck` repacking is possible but poorly tooled here.

**g) Multiplayer + mods — structurally awkward.** Every player in a session needs byte-identical mods. Any overhaul is de facto single-player or closed-group.

---

## 5. PART IV — Would a full overhaul, including graphics, actually work?

### 5.1 The three routes, assessed

#### Route A — In-engine modded overhaul (recommended)
Stay on the shipped BL2 binary. Combine PythonSDK (systems, gear, QoL, UI logic) + text mods (balance) + TFC texture injection (selective) + Scaleform SWF work (UI) + ReShade (grade) + DXVK (performance) + config tuning.

- **Graphics ceiling:** sharper textures on hero assets, materially better colour/contrast/sharpening/faux-AO, higher shadow resolution, better AA, unlocked framerate/FOV/ultrawide. Honest description: **"BL2 at its absolute best,"** not "BL2 remade."
- **Gameplay ceiling:** essentially unlimited. Everything in the Tier 1–3 demand list except working netcode is reachable.
- **Risk:** low. **Effort:** moderate-to-high but *proven* — Reborn and Exodus are existence proofs.
- **Fatal limitation:** cannot fix co-op, the #1 ask.

#### Route B — UE5 (or UE4) remake
- Requires re-authoring **every** asset — 914 packages, ~2 GB of world/character textures, all meshes, levels, VFX, animation, UI, audio integration. Ripping shipped assets and shipping them is straightforward copyright infringement.
- **Legal exposure is the highest of any option.** Take-Two acquired Gearbox from Embracer for $460M in 2024, so BL2's IP now sits with the publisher that DMCA'd the `re3`/`reVC` GTA reverse-engineering projects off GitHub twice and then *sued the developers* for filing a counter-notice. Take-Two is among the most aggressive litigants in the industry regarding fan projects. A public "Borderlands 2 Remastered in UE5" is a takedown magnet.
- **Verdict:** technically conceivable for a funded studio; not realistically achievable — or safely publishable — by a community project.

#### Route C — What an *official* remaster would do (the benchmark)
Worth stating, because it defines the gap. **Mass Effect Legendary Edition is the exact precedent**: BioWare took the same generation of UE3, upgraded it *in place* to **DirectX 11** and **64-bit**, then rebuilt shadows and added SSAO, improved AA, bokeh DoF, and back-ported newer engine features across the trilogy. That is the correct model for BL2 — but every step requires **UE3 engine source access**, which only Gearbox/Take-Two has. Gearbox has also proven it can still work in this codebase: it re-engineered BL2 on UE3 for **Borderlands 2 VR** (2018), and shipped **Borderlands GOTY Enhanced** (2019).

### 5.2 Direct answer to "would a full overhaul, even graphics, work?"

**Partially — and the part that doesn't work is smaller and less important than you'd expect.**

| Overhaul dimension | Feasible by modding? | Notes |
|---|---|---|
| Balance, loot, economy, XP, drop rates | ✅ Fully | Text mods / SDK |
| New weapons, gear, bosses, loot sources | ✅ Fully | Exodus: 130+ new items proven |
| Skills, character reworks, new mechanics | ✅ Fully | Reborn: full character overhauls proven |
| UVHM/slag/OP rework, endgame restructure | ✅ Fully | BL2Fix proven |
| QoL: bank space, sorting, skip, fast travel | ✅ Fully | Multiple proven mods |
| UI/HUD redesign | ✅ Largely | Scaleform SWF + SDK |
| **Texture upgrade** | ⚠️ **Selective only** | Hard ~4 GB address-space budget |
| Mesh replacement / new models | ⚠️ Possible, laborious | UPK Explorer; no modern pipeline |
| Post-processing, colour, AA, sharpening | ✅ Fully | ReShade |
| Performance / frametime | ✅ Meaningfully | DXVK |
| FOV, ultrawide, unlocked FPS | ✅ Fully | Existing fixes |
| **Re-baked lighting / relighting** | ❌ **Blocked** | Editor stripped, Lightmass unavailable |
| **Modern renderer (DX11+/PBR/GI)** | ❌ Blocked | D3D9/SM3 in binary |
| **64-bit** | ❌ Blocked | Needs engine source |
| **New maps at scale** | ❌ Effectively blocked | `CookedPCConsole`, no editor |
| **Working co-op / netcode** | ❌ Blocked | `bifrost.dll` + SHiFT backend |

So: a **"BL2 Remastered" that is ~80% of what the community is asking for is buildable today** — because the community is overwhelmingly asking for systems, stability and QoL, not pixels. The part you cannot reach is renderer, lighting, 64-bit, new maps and netcode.

The irony worth sitting with: **the single most-demanded fix (co-op) and the most-imagined fix (graphics) are both in the blocked column — and everything in between is wide open.**

---

## 6. Risks

1. **Legal (highest).** Take-Two owns Gearbox. Redistributing game assets, shipping a modified executable, or branding anything "Borderlands 2 Remastered" invites a takedown. Mitigation, descending safety: distribute **patches/mods requiring the user's own legally-owned copy**, never ship original assets, avoid official-looking branding, avoid monetisation entirely.
2. **Platform dependency.** The whole stack proxies `ddraw.dll`/`d3d9.dll`. A Gearbox patch — and they *are* still patching, as the 2025–26 SHiFT changes prove — can break everything overnight.
3. **The moving floor.** BL2 is being actively (and, per the reviews, harmfully) modified right now. Building on it is building on sand.
4. **Multiplayer fragmentation.** Byte-identical mods required; effectively single-player or closed-group.
5. **Memory ceiling.** Every texture competes for ~4 GB. Ignore this and you ship the exact crash people already blame the official UHD pack for.
6. **Scope death.** Reborn (750+ changes) and Exodus (2017→) each took *years*. Budget accordingly.

---

## 7. Recommended approach

**Phase 1 — "Make it work" (highest value-per-hour).**
DXVK integration + a curated ~300-texture UHD subset (the Nexus TFC-installer approach) + crash triage + FOV/ultrawide/framerate + a sane ReShade grade as an optional toggle. Addresses crashes (10.3% of negatives) and performance, and is nearly all off-the-shelf assembly.

**Phase 2 — "Make it modern."**
PythonSDK-based QoL: bank/backpack expansion, sorting/filtering/lock/junk, dialogue and cutscene skip, fast-travel-from-anywhere, reward reroll. Model it explicitly on **Borderlands GOTY Enhanced** — Gearbox already validated this exact list on BL1.

**Phase 3 — "Make it fair."**
The UVHM/slag/OP rework: slag tactical rather than mandatory, melee scaling fixed at OP levels, buffs to pearls and dead-weight legendaries, build diversity restored at endgame. The loudest *design* ask in the discourse. Start from BL2Fix's modular philosophy — **every change toggleable, defaults conservative.**

**Phase 4 — "Make it feel good" (optional module, default off).**
Slide, mantle, recoil-recovery tuning, ADS transitions. Ship it off by default; the purist faction is real and vocal.

**Phase 5 — Visual polish, continuous.**
Selective hero-asset retexturing within the memory budget. Set expectations honestly: this is *"BL2 at its sharpest,"* not a relight.

**Do not attempt:** a UE5 remake, a netcode replacement, or new campaign areas.

**Governing principle, drawn straight from the data:** the community does not want BL2 to look like a 2026 game. It wants BL2 to *work* like one.

---

## 8. Source data

Raw corpora and re-runnable scripts live in `research/`:

- `fetch_steam_reviews.py`, `fetch_neg.py` — Steam public review API collectors
- `steam_forum.py` — Steam Discussions topic scraper
- `ddg3.py` + `queries_full.txt` — search-index harvester (resumable)
- `analyze.py`, `topic_themes.py`, `aggregate.py` — theme frequency analysis
- `reviews_bl2_recent.jsonl` (1500), `reviews_bl2_helpful.jsonl` (477), `reviews_bl2_negative.jsonl` (126)
- `reviews_bl4_recent.jsonl` (1000), `reviews_bl3_helpful.jsonl` (242)
- `steam_topics_bl2.json` (247), `reddit_ddg_all.json`

### External references

**Community / modding:** [borderlandsmodding.com](https://borderlandsmodding.com/) · [Major Mod Packs](https://borderlandsmodding.com/mod-packs/) · [SDK Mods](https://borderlandsmodding.com/sdk-mods/) · [BLCM/BLCMods wiki](https://github.com/BLCM/BLCMods/wiki) · [bl-sdk on GitHub](https://github.com/bl-sdk) · [willow2-mod-manager](https://github.com/bl-sdk/willow2-mod-manager) · [BL2/TPS SDK Mod Database](https://bl-sdk.github.io/willow2-mod-db/) · [apple1417.dev](https://apple1417.dev/) · [apocalyptech legacy modding notes](https://apocalyptech.com/games/bl-modding/legacy.php)

**Tools:** [UPK Explorer for UE2–UE3](https://www.nexusmods.com/site/mods/587) · [TFC Installer for UE2–UE3](https://www.nexusmods.com/site/mods/588) · [UPK Explorer on PCGamingWiki](https://www.pcgamingwiki.com/wiki/UPK_Explorer) · [Universal Split Screen — BL2](https://universalsplitscreen.github.io/docs/guides/borderlands2/)

**Mods:** [BL2 Reborn](https://www.nexusmods.com/borderlands2/mods/115) · [BL2 Exodus (SDK)](https://www.nexusmods.com/borderlands2/mods/257) · [BL2Fix](https://www.nexusmods.com/borderlands2/mods/277) · [UHD Texture Pack TFC Installer](https://www.nexusmods.com/borderlands2/mods/522) · [Vivid Graphical Improvements](https://www.nexusmods.com/borderlands2/mods/293) · [BetterLands ReShade](https://www.nexusmods.com/borderlands2/mods/236)

**Precedent & context:** [Borderlands GOTY Enhanced (PCGamingWiki)](https://www.pcgamingwiki.com/wiki/Borderlands_GOTY_Enhanced) · [Borderlands GOTY Enhanced (wiki)](https://borderlands.fandom.com/wiki/Borderlands:_Game_of_the_Year_Enhanced) · [Mass Effect Legendary Edition — visual enhancements](https://blog.playstation.com/2021/04/13/mass-effect-legendary-edition-a-detailed-look-at-visual-enhancements-to-the-celebrated-trilogy/) · [BL2 Pushes New Boundaries with UE3](https://www.unrealengine.com/en-US/blog/borderlands-2) · [Tweaking BL2 for VR (Game Developer)](https://www.gamedeveloper.com/game-platforms/tweaking-the-original-i-borderlands-2-i-to-make-a-vr-friendly-game) · [BL2 Ultra HD Texture Pack (SteamDB)](https://steamdb.info/app/941170/)

**Community sentiment:** [Steam — BL2 discussions](https://steamcommunity.com/app/49520/discussions/) · [ResetEra — Gearbox broke BL2/TPS for crossplay + ads](https://www.resetera.com/threads/so-gearbox-apparently-broke-borderlands-2-presequel-on-pc-in-order-to-add-steam-egs-crossplay-and-bl3-ads.176602/) · [r/Borderlands2 — What I'd Want To See In A BL2 Remaster](https://www.reddit.com/r/Borderlands2/comments/uyg6cn/what_id_want_to_see_in_a_bl2_remaster/) · [Change.org — BL2 Remaster petition](https://www.change.org/p/borderlands-2-remaster) · [Change.org — release a BL2 level editor](https://www.change.org/p/gearbox-software-please-distribute-ue3-level-editor-for-borderlands-2-pc-a-cheaper-udk) · [BL2 Steam Charts](https://steamcharts.com/app/49520)

**Business / legal:** [Take-Two acquires Gearbox for $460M (Variety)](https://variety.com/2024/gaming/news/take-two-interactive-buys-borderlands-gearbox-embracer-group-460-million-1235954475/) · [GitHub removes re3/reVC after Take-Two DMCA (TorrentFreak)](https://torrentfreak.com/github-removes-gta-fan-projects-re3-revc-following-new-take-two-dmca-notice-211004/) · [Take-Two sues re3/reVC developers (TorrentFreak)](https://torrentfreak.com/take-two-sues-enthusiasts-behind-gta-fan-projects-re3-revc-210903/)
