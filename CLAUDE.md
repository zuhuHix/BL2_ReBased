# BL2_ReBased: notes for Claude Code

The project brief is `AGENTS.md`. The line below imports it, so its text is normally already in your context. If you
cannot see a section headed "Rules that do not bend", read `AGENTS.md` before doing anything else.

@AGENTS.md

## Claude Code specifics

- **Guard hook.** `.claude/settings.json` runs `.claude/hooks/sensitive_guard.py` before every Edit and Write. It
  lets edits to the sensitive files through with a reminder (you still owe the warning in your final message), prompts
  the maintainer for `THIRD_PARTY.md` and the licence files, and blocks game files (`.upk`, `.tfc`, `.pck`, `.bik` and
  similar) and decompiler or disassembler databases (`.gzf`, `.gpr`, `.rep`, `.idb`, `.i64`, `.bndb`). A block means
  the file does not belong in the repository. Put it under ignored `local/`; do not write it another way. The hook
  only knows file names and extensions, so a write it allows is not thereby safe: it cannot see a pasted listing,
  pseudo-code or game data, and it does not see shell writes at all.
- **Briefing sub-agents and worker chats.** They start with none of your context. Point them at
  `docs/LANE_BRIEF.md` and `docs/verification/GATES.md` instead of pasting rules, and always give them: the tools line
  (bl-sdk live reads first, REA for decoding), the process-safety rule (rule 10), the files they own, and how to hand
  back (`docs/LANE_BRIEF.md`, "How to finish a lane").
