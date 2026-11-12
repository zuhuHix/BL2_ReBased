## 2026-09-14: visible color for Unlit Material v1

Recovered diffuse or fallback color is also connected to Emissive Color for
Unlit materials when no explicit emissive texture exists. UE Unlit ignores
Base Color for visible shading. Explicit emissive keeps its existing masked
policy and takes precedence independent of channel order. The actual
Sanctuary sky instance uses this fallback path. No native graph or UV mapping
is inferred. See docs/verification/UNLIT_COLOR.md for evidence and limitations.

