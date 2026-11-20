## 2026-11-19: Translated UE3 shaders may be committed (shaders option C-public)

AI-assisted (Claude Code, Opus 5.5). Maintainer decision (zuhu, in chat on 2026-11-19): "C, confirmed", then
"C-public approve draft". The renderer's shaders may be exact translations of the game's own shader source, and those
translations may be committed to the public repository.

**What changes**

- WGSL translated from the UE3 shader source that ships with Borderlands 2 (`Engine/Shaders/*.usf` in the player's
  install) may be committed under `crates/ow-render/shaders/ue3/`. Each such file is headed with its source file name
  and a notice that it is derived from Epic Games' UE3 shaders and is not under the MIT licence.
- The `.usf` files themselves are never committed. The fitted shaders stay as the fallback.
- AGENTS.md rule 2, `docs/LANE_BRIEF.md`, `docs/LEGAL.md` ("What this repository contains", "does not contain",
  "License"), `LICENSE`'s closing note, `README.md` ("Is this legal?") and `THIRD_PARTY.md` (a new entry) now say so.

**Scope, narrower than the draft in one point.** The hub read the first lines of the 163 `.usf` files on the zuhu
desktop's install. Most carry an Epic Games copyright notice, but a few carry a Gearbox Software, NVIDIA or Scaleform
notice, and some carry none. The draft named only Epic Games' shaders, so the exception covers only the Epic-headed
files. Translating any of the others into the tree needs its own decision.

**Risk, accepted by the maintainer.** The repository no longer "ships no Epic code": the translated shaders are code
derived from a rights holder's source, published without a licence grant. A rights-holder takedown of the public
repository is possible. The files are removed at the rights holder's request. The alternative, C-private (the same
translations kept in the private store and never published), was offered and not chosen.

**Next.** A translation lane starts with the lighting path (light-map decode and the shadow term), then fog and
material blending. Each translated shader gets a synthetic render test against the fitted one. The open H9 lightmap
question is answered from the source.

**Verification.** This is a policy change; nothing was built or run. Whether the translated shaders match the game's
rendering stays UNVERIFIED until it is compared against the running game.
