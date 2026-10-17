"""Paths the analysis and training scripts share.

REPO is the repository root (the parent of scripts/), or $YSE_REPO if set.

YANE is the YaneuraOu binary used as teacher and judge, from $YSE_YANE if set; otherwise
the first YaneuraOu build found in a `yaneuraou-*` directory next to any ancestor of the
repository (the development machine keeps it at ../yaneuraou-V900-git-mac-all). Scripts
that need it call require_yane(), which stops with instructions when it is missing.

    python3 scripts/paths.py        # print what was found
    python3 scripts/paths.py yane   # print the YaneuraOu path only (for shell scripts)
"""
import glob, os, platform, sys

REPO = os.environ.get("YSE_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# The network the development machine judges with; other builds are accepted.
_PREFERRED = "NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-*"


def _find_yane():
    if os.environ.get("YSE_YANE"):
        return os.environ["YSE_YANE"]
    for base in [REPO, *list(_ancestors(REPO))]:
        for pattern in (_PREFERRED, "*/YaneuraOu*", "YaneuraOu*"):
            hits = sorted((p for p in glob.glob(os.path.join(base, "yaneuraou-*", pattern))
                           if os.path.isfile(p) and os.access(p, os.X_OK)),
                          key=lambda p: (not _native(p), p))
            if hits:
                return hits[0]
    return None


def _native(path):
    """A build named for this CPU (the YaneuraOu release ships one binary per target)."""
    name = os.path.basename(path).upper()
    if platform.machine().lower() in ("arm64", "aarch64"):
        return "M1" in name or "ARM" in name
    return "AVX2" in name


def _ancestors(path):
    while True:
        parent = os.path.dirname(path)
        if parent == path:
            return
        yield parent
        path = parent


YANE = _find_yane()


def require_yane():
    if not YANE or not os.path.isfile(YANE):
        sys.exit("YaneuraOu not found: set YSE_YANE to the engine binary "
                 "(see docs/REPOSITORY.md, 'Setting up a machine').")
    return YANE


if __name__ == "__main__":
    if sys.argv[1:] == ["yane"]:
        print(require_yane())
    else:
        print("REPO", REPO)
        print("YANE", YANE)
