"""Where the fetched research corpus lives.

The corpus (Steam review text and similar third-party writing) is not tracked
in the repository. The fetch scripts write it, and the analysis scripts read
it, in one folder outside the tracked tree:

1. ``--corpus DIR`` on the command line, if given;
2. otherwise the ``OW_RESEARCH_CORPUS`` environment variable;
3. otherwise ``local/research_corpus`` under the repository root (ignored by git).

A relative ``--corpus`` is taken from the current folder. Fetchers refuse to
write anywhere inside the repository except under the root ``local/`` folder,
so fetched text cannot land in a tracked or un-ignored place by accident.
"""
import os
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_DIR = os.path.join(REPO_ROOT, "local", "research_corpus")
ENV_VAR = "OW_RESEARCH_CORPUS"


def corpus_dir(argv=None):
    """Return the corpus folder, removing ``--corpus DIR`` from ``argv``.

    ``argv`` defaults to ``sys.argv`` and is edited in place, so scripts can
    keep reading their positional arguments as before.
    """
    if argv is None:
        argv = sys.argv
    path = None
    for i, arg in enumerate(argv):
        if arg == "--corpus":
            if i + 1 >= len(argv) or not argv[i + 1]:
                sys.exit("--corpus needs a folder")
            path = argv[i + 1]
            del argv[i:i + 2]
            break
        if arg.startswith("--corpus="):
            path = arg.split("=", 1)[1]
            if not path:
                sys.exit("--corpus= needs a folder")
            del argv[i]
            break
    if path is None:
        path = os.environ.get(ENV_VAR) or DEFAULT_DIR
    path = os.path.abspath(path)
    if os.path.exists(path) and not os.path.isdir(path):
        sys.exit("The research corpus location %s is a file, not a folder." % path)
    return path


def _plain(path):
    """Resolved, case-folded path with any Windows long-path prefix removed."""
    for _ in range(2):  # before and after resolving
        if path.startswith("\\\\?\\UNC\\"):
            path = "\\\\" + path[8:]
        elif path.startswith("\\\\?\\"):
            path = path[4:]
        path = os.path.realpath(path)
    return os.path.normcase(path)


def _within(path, root):
    # Not covered: a UNC path (\\host\share\...) that reaches the repository's disk
    # is not recognised as inside it, and a local/ folder that is itself a junction or
    # link into the tracked tree is trusted, because both resolve as written.
    path = _plain(path)
    root = _plain(root)
    return path == root or path.startswith(root.rstrip(os.sep) + os.sep)


def output_path(folder, name):
    """Path for a fetcher's output file; creates its folder if needed.

    Refuses a path inside the repository that is not under the root local/.
    """
    target = os.path.abspath(os.path.join(folder, name))
    if _within(target, REPO_ROOT) and not _within(target, os.path.join(REPO_ROOT, "local")):
        sys.exit("Refusing to write fetched text to %s: it is inside the repository but not under "
                 "%s, so git could track it. Use the default folder or one outside the "
                 "repository." % (target, os.path.join(REPO_ROOT, "local")))
    os.makedirs(os.path.dirname(target), exist_ok=True)
    return target


def resolve(folder, name):
    """A file in the corpus folder, or else a path that exists as given."""
    inside = os.path.join(folder, name)
    if not os.path.isfile(inside) and os.path.isfile(name):
        return name
    return inside


def missing_corpus(folder, names, fetcher):
    """Exit with a message saying the corpus is absent and how to fetch it."""
    sys.exit(
        "No research corpus found in %s (looked for: %s).\n"
        "The corpus is not tracked in the repository. Fetch it with %s "
        "(see research/README.md), or point --corpus or %s at a folder "
        "that holds it." % (folder, ", ".join(names), fetcher, ENV_VAR))
