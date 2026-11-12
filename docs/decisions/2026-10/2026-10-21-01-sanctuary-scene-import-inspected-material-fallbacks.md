## 2026-10-21: Sanctuary scene import: inspected material fallbacks, component owners, doubled colour buffers

AI-assisted (Claude), lane A. Details and counts: `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`.

- **Reader (`src/assets.cpp`, sensitive).** Three of the Sanctuary packages' meshes (27 of 17,605 StaticMesh exports
  in the 914 base packages) store a colour buffer of stride 4 whose count and bulk count are exactly twice the vertex
  count. The reader now accepts `vertexCount` or `2 × vertexCount`, reads the first `vertexCount` entries and skips
  the rest. It also accepts the export class `GearboxFramework.WiringMesh` with the StaticMesh layout. Oracle: with
  both changes every one of the 17,605 StaticMesh/WiringMesh exports decodes and leaves the same 73-byte trailer
  after its LODs; no export failed. Which colour entries the original renderer binds is `UNVERIFIED` (colours are
  not exported). No bounds check was loosened.
- **Owners (`tools/prepare_level.py`).** Owner actors outside the bulk scene records are read per actor with the
  observed 26-byte actor prefix and must end at the object end. `WiringActor` and `WillowInteractiveObject` components
  are placed like a `StaticMeshActor`'s; mission pickup spawners and failsafes, whose component is the actor's
  `PreviewMesh`, are listed in `excluded_components` (`mission_item_preview_mesh`), as is the second exact copy of a
  component on the same interactive object (`duplicate_component`; the copy must also match on `HiddenGame`, shadow and collision flags, so a visible component is never dropped for a hidden one). Confirmed in the running game (Plan B save):
  spawners and failsafes have no primitive component; every placed interactive-object component is attached, not
  hidden, with the package's mesh and relative pose; each vault-symbol actor attaches one plane, not two. Their
  visibility in other mission or challenge states is `UNVERIFIED`. The rules are not map-specific: on Ash they place 21 interactive-object components and list 17 duplicates (271 unsupported owners left, from 325), on Southpaw 4 (1 left, from 10); placement on those maps was not compared with the running game and is `UNVERIFIED`.
- **Materials.** Five more exact `INSPECTED_COLOR_FALLBACKS` (icicles, two distant snow fields, distant mountains,
  Roland's chemical tank), each from its parameter chain, cooked list and the decoded candidates; the icicles'
  `Ice_Dif` is the named internal-glow sampler and is not used as colour. All `partial_unverified`. The spire
  instances stay with the `prepare_sanctuary_pillar.py` bake (2026-09-23). `Mat_WorldMap` stays a gap: in game its
  packed `_Comp` channels are recoloured (pale cyan land, red markers), so the texture is not an RGB colour.
- **Host (`host/ue5/import_level.py`).** A Modulate material with no recovered colour gets a white neutral instead of
  0.5 grey, so the newly placed vault symbols and blood decal do not darken what is behind them.
- **Audit.** Terrain weighted-sum materials are counted as `terrain_weighted_blend`, not as "no supported channels";
  exclusions are counted by reason.

