"""PreToolUse guard: warn before editing sensitive areas; refuse game-file writes.

Reads the hook JSON on stdin. Emits a permission decision so the harness
prompts the user (not the model) before a lighter model touches fragile code.
Input it cannot read (no JSON, no path) gets no decision and exit 0, as before: the hook never blocks by failing.
"""
import fnmatch
import json
import os
import posixpath
import re
import sys

# Paths where a wrong assumption cascades (AGENTS.md, "Sensitive files").
#  - EDIT_THEN_WARN: the maintainer (2026-09-30) allows editing these freely; the assistant must
#    say so plainly in its final message (what changed, what is verified vs UNVERIFIED). The Rust ports
#    and the workspace files joined on 2026-11-26. Matched against the repository-relative path only, so
#    only the root CMakeLists.txt, Cargo.toml and rust-toolchain.toml count, as AGENTS.md lists them.
#  - ASK: license/provenance files stay a maintainer decision and still prompt, wherever they lie.
EDIT_THEN_WARN = {
    "src/package.cpp": "binary reader / bounds checks (Reader, require(), limit)",
    "src/container.cpp": "LZO container decoding and size validation",
    "src/container.hpp": "container interface",
    "CMakeLists.txt": "build + dependency wiring",
    "crates/ow-container/*": "Rust port of the LZO container decoding and size validation",
    "crates/ow-package/*": "Rust port of the package reader / bounds checks",
    "Cargo.toml": "workspace manifest (a new third-party crate is a dependency, AGENTS.md rule 6)",
    "rust-toolchain.toml": "pinned Rust toolchain",
}
ASK = {
    "THIRD_PARTY.md": "provenance and license record",
    "LICENSE*": "project license is MIT; changing it is a maintainer decision",
    "COPYING*": "project license is MIT; changing it is a maintainer decision",
}
# Files that must never be written into the repo. The last group is decompiler and disassembler
# databases (docs/LEGAL.md, "Analysing the executable"): game-derived, kept under local/ or a private store.
FORBIDDEN = ["*.upk", "*.tfc", "*.pck", "*.bik", "*.umap", "*.uncompressed_size",
             "*.gfx", "*.swf",
             "*.gzf", "*.gpr", "*.rep", "*.idb", "*.i64", "*.bndb"]

DRIVE = re.compile(r"^[A-Za-z]:/")


def normalise(path):
    """One spelling for a path, whatever the platform: no Windows long-path prefix, forward slashes,
    then `.` and `..` resolved. In that order, so `a\\..\\b` is resolved after its slashes are turned."""
    if path.startswith("\\\\?\\UNC\\"):
        path = "\\\\" + path[8:]
    elif path.startswith("\\\\?\\"):
        path = path[4:]
    path = path.replace("\\", "/")
    if not (path.startswith("/") or DRIVE.match(path)):
        path = os.getcwd().replace("\\", "/") + "/" + path
    return posixpath.normpath(path)


def relative(path, root):
    """The path below the repository root, or the whole path when it lies elsewhere."""
    same = (lambda a, b: a.lower() == b.lower()) if os.name == "nt" or DRIVE.match(root) else (lambda a, b: a == b)
    stem = root.rstrip("/") + "/"
    if same(path[:len(stem)], stem):
        return path[len(stem):]
    return path


try:
    payload = json.load(sys.stdin)
except Exception:
    sys.exit(0)

tool_input = payload.get("tool_input") if isinstance(payload, dict) else None
path = tool_input.get("file_path") if isinstance(tool_input, dict) else None
if not isinstance(path, str) or not path:
    sys.exit(0)
rel = relative(normalise(path), normalise(os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()))
base = posixpath.basename(rel)


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

for pat, why in ASK.items():
    if fnmatch.fnmatch(rel, pat) or fnmatch.fnmatch(base, pat):
        out("ask",
            f"SENSITIVE AREA: '{rel}' - {why}. Maintainer confirmation required.",
            f"Sensitive file: {rel} - {why}. Review this edit carefully.")

# After ASK, so a licence or provenance file inside one of these folders still prompts.
for pat, why in EDIT_THEN_WARN.items():
    if fnmatch.fnmatch(rel, pat):
        # Kept short: this text enters the session on every edit of these files. AGENTS.md has the long form.
        out("allow",
            f"SENSITIVE (edit allowed): '{rel}' - {why}. AGENTS.md \"Sensitive files\": no loosened bounds check, gates "
            "green, a decision file if parsing changes, and WARN THE MAINTAINER in your final message, owed even when "
            "this hook is silent.",
            f"Sensitive file edited: {rel} ({why}); report it at the end.")

sys.exit(0)
