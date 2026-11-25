## 2026-11-25: Hook 3's array check is a static-mesh lightmap test

AI-assisted (Claude Code)

Lane ANALYST-LINK1-NEXT, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** a look-ahead for link 1. Past hook 2, the first unread item on `maya:8`'s path is hook 3's unidentified
  array check (`render_hooks::ARRAY_CHECK`).
- **What:** section 7 of `docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md`, with superseded markers in sections 1, 4
  and the open questions, plus one NATIVE_INDEX confidence entry and one GATES row. The evidence is a local read-only
  Ghidra reading under the machine lock, the layout tool, and `ow-package --def-dump` of Maya's body class and its
  component templates. Raw output stays under ignored `local/analysis/link1next/`.
- **Findings** (UNVERIFIED in game):
  - the helper's second class test is `StaticMeshComponent`, on the same component;
  - the array is its `LODData`, by fit, and a set `LightMap` in the first entry skips hook 3;
  - the lighting-channel step is static-mesh only too;
  - Maya's made mesh components are all skeletal (`GearLikenessMeshComponent` and `CustomSkeletalMeshComponent`
    extend `SkeletalMeshComponent`), and their templates have `bAcceptsLights` true. So they go straight to the setter.
- **Proposed Off answer:**
  - a made `StaticMeshComponent` is `NotRead` (its `LODData` is not decoded);
  - any other mesh component runs `plan_light_environment`, and the plan is applied in the write phase;
  - tests listed.
- **Prediction** (not a run): link 1's next stop is attachment 1's `Parent` link (hub ruling 3).
- **Review r1 (#715):** superseded markers added where the older text still calls the array check a stop for any
  mesh component: section 2's hook 3 line, section 3's hook 3 line, section 4's preflight step 3 and test 13, and
  section 6.1's prediction.
- **Clean room:** a different lane implements this note.
