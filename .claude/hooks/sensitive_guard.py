"""PreToolUse guard: warn before editing sensitive areas; refuse game-file writes.

Reads the hook JSON on stdin. Emits a permission decision so the harness
prompts the user (not the model) before a lighter model touches fragile code.
"""
import fnmatch
import json
import os
import sys

# Paths where a wrong assumption cascades. Editing prompts for confirmation.
SENSITIVE = {
    "src/package.cpp": "binary reader / bounds checks (Reader, require(), limit)",
    "src/container.cpp": "LZO container decoding and size validation",
    "src/container.hpp": "container interface",
    "CMakeLists.txt": "build + GPL dependency wiring",
    "THIRD_PARTY.md": "provenance and license record",
    "LICENSE*": "project license is MIT; changing it is a maintainer decision",
    "COPYING*": "project license is MIT; changing it is a maintainer decision",
}
# Files that must never be written into the repo.
FORBIDDEN = ["*.upk", "*.tfc", "*.pck", "*.bik", "*.umap", "*.uncompressed_size",
             "*.gfx", "*.swf"]

try:
    payload = json.load(sys.stdin)
except Exception:
    sys.exit(0)

path = (payload.get("tool_input") or {}).get("file_path") or ""
root = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()
try:
    rel = os.path.relpath(os.path.abspath(path), os.path.abspath(root))
except ValueError:
    rel = path
rel = rel.replace("\\", "/")
base = os.path.basename(rel)


def out(decision, reason, message):
    print(json.dumps({
        "systemMessage": message,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
            "additionalContext": reason,
        },
    }))
    sys.exit(0)


for pat in FORBIDDEN:
    if fnmatch.fnmatch(base, pat):
        out("deny",
            f"Refusing to write '{rel}': game/asset files must never enter the repo "
            "(docs/LEGAL.md). Fixtures must be synthetic.",
            f"BLOCKED: attempted to write game asset file {rel}")

for pat, why in SENSITIVE.items():
    if fnmatch.fnmatch(rel, pat) or fnmatch.fnmatch(base, pat):
        out("ask",
            f"SENSITIVE AREA: '{rel}' — {why}. Before editing: state what is verified vs "
            "UNVERIFIED, keep bounds checks intact, run ctest + tools/verify_packages.py, "
            "and add a DECISIONS.md entry if parsing behavior changes.",
            f"⚠ Sensitive file: {rel} — {why}. Review this edit carefully.")

sys.exit(0)
