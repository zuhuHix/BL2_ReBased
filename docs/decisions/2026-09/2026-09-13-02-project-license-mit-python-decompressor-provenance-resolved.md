## 2026-09-13: Project license MIT; Python decompressor provenance resolved

The project-wide license is MIT (`LICENSE`), chosen by the maintainer. GPL-3
was the alternative and would have absorbed the miniLZO-derived research
decompressor without changes, but the host engine is Unreal Engine 5 and GPL
code cannot be distributed as a binary linked against it under Epic's EULA;
a GPL OpenWillow could never ship a runnable build. MIT is compatible with
UE5 and with the vendored lzokay, is the license used by comparable
reimplementations that sit on a proprietary host (Ship of Harkinian,
OpenGothic), and needs no contributor license agreement: contributions are
accepted under the same MIT terms (inbound = outbound), as stated in
CONTRIBUTING.md and docs/LEGAL.md.

The license does not change the project's exposure to the rights holder;
that is governed by the clean-room rules, the no-redistribution rule and the
original-game requirement, which are unchanged.

To make MIT honest, the miniLZO-derived `lzo1x_decompress` in
`research/native_count.py` was replaced with an independent implementation
(see THIRD_PARTY.md). Verification: `tools/verify_packages.py` reports
decoded bytes, counts and export fields matching the C++ reader on all nine
code packages; `research/native_count.py` reproduces 20,119 / 7,141 / 12,978
/ 2,453. The oracle is now less independent of lzokay than the miniLZO port
was, but the miniLZO-versus-lzokay byte-for-byte agreement was already
recorded on 2026-09-10 and stands as the cross-lineage check.

