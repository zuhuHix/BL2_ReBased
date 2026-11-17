## 2026-11-17: Navmesh repeated edge endpoint refusal

AI-assisted (Codex / GPT-6.1 Sol).

- Review r2 of PR #373 found that the neighbour edge check accepted the same endpoint twice when two distinct
  vertex indices were shared. The reader now requires the unordered edge pair to equal both shared indices.
- The existing synthetic refusal table now includes an invented triangle with a repeated endpoint and a link on
  that degenerate edge. It was accepted before the fix and is refused with `NeighbourNotOnEdge` after it.
- This tightens the existing structural oracle; it adds no offset, field interpretation, dependency or tool.
  Valid edge orientation remains irrelevant. Existing bounds and finite-value checks stay in force.
- This is synthetic parsing evidence only. The fitted format and running-game compatibility remain UNVERIFIED;
  no host or real-game session was run for this correction.
- The two distinct November 17 record filenames already published on the map-load and navmesh branches are
  preserved. Their shared sequence number does not make their full paths identical, and the decision checker
  forbids renaming an existing record. This new correction uses the next number available on main.
