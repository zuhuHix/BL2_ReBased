## 2026-10-07: REA MCP server installed as an analysis front end over Ghidra (maintainer request)

AI-assisted (Claude). At the maintainer's request, `rea-agents` 4.1.0 (MIT, https://github.com/morluto/rea) is installed globally with npm,
outside the repository, and registered as an MCP server for Claude Code with the existing Ghidra 12.1.4 and Temurin JDK 21. Its doctor check
reports the Ghidra provider available; Hopper and IDA are not configured. Provenance and the package integrity hash are in THIRD_PARTY.md. The
clean-room rules for Ghidra apply unchanged to REA: no REA output enters the repository, and notes are written in our own words. Also recorded:
the maintainer prefers reading data and observable behaviour from the live game through the SDK (the approach of their BL2_RTSE mod) over
package decoding or disassembly where the answer is observable at runtime.

