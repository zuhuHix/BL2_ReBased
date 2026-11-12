## 2026-09-14: diagnostic cooked Material tail structure

The user authorized work on the next high-complexity tasks. Added an
independent diagnostic decoder for the directly observed native tail:
six words, bounded count, 16-byte records and final word, with exact
consumption required. All field semantics remain UNVERIFIED. This does not
change scene material selection or reconstruct stripped graphs. Existing
property, package and container validation is unchanged. Bulk CLI payload
extraction reuses existing export bounds checks and validates every requested
index before output. No dependency or license changes; no external code
copied. See docs/verification/MATERIAL_RESOURCE_CENSUS.md for evidence and limits.

