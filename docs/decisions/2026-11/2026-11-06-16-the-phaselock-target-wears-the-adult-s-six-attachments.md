## 2026-11-06: the Phaselock target wears the Adult's six attachments, placed from a live read (maya lane)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "The Adult's worn attachments". Host and tool change, no parsing change.

- The tool reads the default-on static meshes from the Adult's body composition, the editor step imports them with the body's zone material, and the host attaches them to their bones. Without the local import the host keeps the bare body.
- The list and the body's material are confirmed live on one Adult (one cast). The pieces' own material is archetype data.
- Placement: the live bone matrices show the imported bone frames are the live ones, so each piece uses the component's own transform and not the citizens' correction. A piece's world matrix could not be read, so the composition is assumed. UNVERIFIED until the import runs.
- The attachments do not explain the width gap. The default facing stays yaw +180.
