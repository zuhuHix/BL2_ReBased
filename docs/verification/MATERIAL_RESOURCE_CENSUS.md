# Material resource structure investigation, 2026-09-14

AI-assisted implementation in `t3code-dc91995f`. This is diagnostic resource
parsing, not a recovered shader graph or an improvement to rendered materials.

The installed Sanctuary_P and Ash_P packages contain respectively 142 and 193
Material exports accepted by the existing cooked texture-list reader. All 335
remaining tails fit this observed structure: six 32-bit words, a signed record
count, that many 16-byte records, and a final 32-bit word. The decoder requires
nonnegative bounded counts and exact consumption. Field meanings remain
`UNVERIFIED`; words are deliberately unnamed and not consumed by UE rendering.

Tail sizes range from 32 to 176 bytes in 16-byte increments. Of the 335
materials, 151 have explicitly empty or all-null expression-slot arrays.
Absent arrays are counted separately. Surviving slots do not establish graph
connectivity. These observations cover two persistent packages, not all their
streamed sublevels or the installed package corpus.

The existing UE Viewer format reference recorded in DECISIONS.md was checked
again: `UMaterial3::Serialize` stops interpreting the resource at its texture
list. No external code was copied or translated. The new tail structure comes
from direct observation of the user's installed package payloads. It is not
claimed as a general UE3 schema.

`ow-package --payloads <index>...` supplies bounded batch extraction, avoiding
repeated decompression for every material. It validates all indices and export
bounds before output. Missing package files are reported without discarding
successful package observations. `Prop_Skybox` is not a standalone file in
this installation; the successful run used Sanctuary_P and Ash_P only.

## Validation

- Release build succeeded.
- Material resource synthetic tests: 4 passed, covering every truncation of
  a record-bearing tail, negative/oversized/inconsistent counts, trailing
  garbage, unaligned opaque bytes, and absent versus stripped expressions.
- Level tests: 15 passed.
- CTest: 5/5 passed, including new bulk CLI tests on compressed and plain
  synthetic packages, byte preservation, and rejection before partial output.
- `verify_packages.py`: all nine reference packages matched decoded bytes,
  counts and export fields.
- Live diagnostic census: 335 recognized tails, zero prefix errors.

No UE5 visual, original-game, or physical-input checks were performed. Material
graph/resource parsing remains incomplete. Next work requires evidence for
field semantics and the connection between native records and material inputs.

Reproduction is documented in [TOOLING.md](../TOOLING.md). The detailed report
is ignored at `local/material-resources/material_resources.json`; no game
payloads or dumps are included in this change.
