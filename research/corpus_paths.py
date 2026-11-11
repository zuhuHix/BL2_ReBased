"""Where the fetched research corpus lives.

The corpus (Steam review text and similar third-party writing) is not tracked
in the repository. The fetch scripts write it, and the analysis scripts read
it, in one folder outside the tracked tree:

1. ``--corpus DIR`` on the command line, if given;
2. otherwise the ``OW_RESEARCH_CORPUS`` environment variable;
3. otherwise ``local/research_corpus`` under the repository root (ignored by git).
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
    for i, arg in enumerate(argv):
        if arg == "--corpus":
            if i + 1 >= len(argv):
                sys.exit("--corpus needs a folder")
            path = argv[i + 1]
            del argv[i:i + 2]
            return os.path.abspath(path)
        if arg.startswith("--corpus="):
            del argv[i]
            return os.path.abspath(arg.split("=", 1)[1])
    return os.path.abspath(os.environ.get(ENV_VAR) or DEFAULT_DIR)


def output_path(folder, name):
    """Path for a fetcher's output file; creates the folder if needed."""
    os.makedirs(folder, exist_ok=True)
    return os.path.join(folder, name)


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
