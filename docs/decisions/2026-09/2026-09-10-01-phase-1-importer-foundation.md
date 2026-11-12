## 2026-09-10: Phase 1 importer foundation

The Phase 0 command-line spikes are now split behind reusable C++ APIs. The
package reader retains decoded package bytes and export/import records;
`PackageStore` lazily indexes `.upk`, `.umap` and `.u` files, caches loaded
packages, and resolves negative imports by package-local outer chain. When a
cooked import's root name is not a disk filename, it falls back to the global
object path across the indexed tree and caches the successful match. This
behavior is based on the installed Ash package: `Common_Materials.Environment.Master_World`
resolved to the `WillowGame` package. Synthetic coverage includes direct and
local-export-nested imports.

The texture importer now returns every available resident mip, including
inline, TFC-streamed and compressed bulk, while retaining the explicit
PF_DXT1/PF_DXT5 and payload-at-end limits. `--mip` selects an output mip and
`--all-mips` writes the available set. The static-mesh importer now retains
all render LODs, all UV sets and either 16- or 32-bit indices; OBJ remains a
diagnostic output for one selected LOD and its first UV set. Source mesh data,
collision hulls, skeletal meshes and material translation are still future
work.

The refactor builds with CMake, all five synthetic CTest suites pass, the nine
code-package differential check remains byte-for-byte identical, and the real
Ash probe now reports 11 resident texture mips and 2 mesh UV sets. These are
still importer/parser checks, not proof of a runtime host-engine asset path.

