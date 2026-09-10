# Dependency provenance

## Optional research decoder: miniLZO 2.10

- Author: Markus Franz Xaver Johannes Oberhumer, copyright 1996-2017.
- Primary source: https://www.oberhumer.com/opensource/lzo/
- Archive: https://www.oberhumer.com/opensource/lzo/download/minilzo-2.10.tar.gz
- SHA-256: `eb4ce543aad19533c83550746e0e9d7bcf716b35a42429e3ba17d60fa0f3e47a`
- License: GPL-2.0-or-later, confirmed in the archive's `minilzo.h` and `COPYING`
  on 2026-09-10. The full upstream archive, including notices and COPYING, is
  fetched into the ignored build directory; upstream sources are not modified.
- API used: `lzo_init`, `lzo1x_decompress_safe`. The bounded decoder rejects
  malformed streams and reports input/output errors to the container reader.

Enable explicitly with `-DOPENWILLOW_RESEARCH_LZO=ON`. The default build has no
miniLZO dependency and rejects compressed inputs with an actionable message.
The enabled executable links GPL code. Calling it a research build does not
exempt distribution from that license. There is no release/installation target;
resolve the combined work's license and host-engine compatibility before release.
This dependency is not a decision to link miniLZO into a future host engine.

## Existing Python research decoder

`research/native_count.py` describes its decompressor as a faithful minilzo port.
The precise upstream version and authorship history of that translation remain
unknown. Treat its provenance as unresolved; it is only a local comparison
oracle. The native decoder uses the pinned original upstream implementation
instead of translating that Python code.

The container/table layouts are recorded in the existing workspace research.
No game binaries, leaked source, or third-party UE3 source were used for this change.
