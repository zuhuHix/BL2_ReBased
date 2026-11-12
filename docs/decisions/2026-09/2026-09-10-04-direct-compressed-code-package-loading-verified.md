## 2026-09-10: Direct compressed code-package loading verified

Supersedes the compressed-loading status above. The C++ reader now validates
fully compressed container headers and block sizes, decodes through upstream
`lzo1x_decompress_safe`, and verifies each output length. The dependency is
optional and disabled by default. See THIRD_PARTY.md for the pinned archive,
license, and remaining Python provenance uncertainty. This is not a permanent
host-engine dependency decision or a project-wide license selection.

The nine-package comparison now passes original compressed paths to C++ and
compares the complete decoded buffer to Python's output before reading tables.
All nine match byte-for-byte. No decoded assets are saved in the repository.
The comparison is agreement between two execution paths related to miniLZO,
not an independent proof of compression-format correctness.

Container decoding rejects trailing bytes, inconsistent totals, invalid block
sizes, malformed streams, and decoded lengths over 512 MiB. Partial package
compression and all-package coverage remain unimplemented. Next work is object
records and tagged properties; host rendering and Phase 0's gate remain open.

