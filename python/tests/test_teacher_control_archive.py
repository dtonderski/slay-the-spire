from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ARCHIVE = (
    Path(__file__).resolve().parents[2]
    / "docs"
    / "puct-teacher-control-v1"
    / "close_teacher_control.py"
)


def test_teacher_control_archive_self_test_and_committed_reports() -> None:
    subprocess.run([sys.executable, str(ARCHIVE), "--self-test"], check=True)
    subprocess.run([sys.executable, str(ARCHIVE), "--verify"], check=True)
