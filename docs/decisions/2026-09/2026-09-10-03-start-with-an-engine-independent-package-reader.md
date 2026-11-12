## 2026-09-10: Start with an engine-independent package reader

C++20 and CMake provide a small working build before a host-engine installation.
UE5 versus Godot remains undecided. The current executable is a research tool,
not the future runtime, and accepts decompressed packages for local verification.

The table layout comes from this workspace's research/native_count.py. The new
C++ reader implements bounds validation independently and does not include its
decompressor. That Python function explicitly describes a minilzo port: its
provenance and applicable license must be established before distribution or
translation into the engine. Project license remains pending.

The initial differential check matches all nine installed code-package table
counts. This does not validate the native/script classification heuristic,
bytecode execution, all content packages, or the plan's effort estimates.

Phase 0 remains incomplete: compressed loading, property parsing, full census,
texture/mesh extraction and host rendering are outstanding. The Sanctuary
weekly gate becomes applicable once a map loader exists.

