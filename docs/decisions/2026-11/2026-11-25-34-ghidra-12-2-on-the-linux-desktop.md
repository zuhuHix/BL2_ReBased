## 2026-11-25: Ghidra 12.2 on the Linux desktop

AI-assisted (Claude Code). Maintainer decision, 2026-11-25: on the desktop, which runs CachyOS Linux since
2026-11-22, the executable-analysis tool is Ghidra **12.2**, from the distribution package `ghidra 12.2-1.1`
(CachyOS repository `cachyos-extra-v3`, Apache License 2.0). It needs Java 25 or newer and runs on the distribution's
OpenJDK 27 (`jdk-openjdk 27.0.1.1.u4-1`, GPL-2.0 with the Classpath Exception). The Windows machines keep Ghidra
12.1.4 with Temurin JDK 21 (decision of 2026-10-01). `THIRD_PARTY.md`, "Ghidra and Temurin JDK 21", records both.

- **Why.** 12.2 was already installed on the desktop when it moved to Linux, and the maintainer chose to keep it
  instead of installing 12.1.4 by hand.
- **What does not change.** The rules for analysing the executable (docs/LEGAL.md, "Analysing the executable";
  docs/NATIVE_ANALYSIS.md): local analysis of the player's own `Borderlands2.exe` only, raw output under ignored
  `local/analysis/` or the private store, notes written in our own words.
- **Not checked.** Whether a project saved by 12.2 opens in 12.1.4. Until it is, analysis databases are not moved
  from the desktop to the Windows machines. `tools/ghidra_import.ps1` is PowerShell and does not run on the Linux
  desktop; no Linux driver was written in this change.
