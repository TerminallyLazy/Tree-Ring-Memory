import os
import subprocess
import sys
from pathlib import Path


PROJECT_SRC = Path(__file__).resolve().parents[1] / "src"


def run_cli(*args, cwd):
    env = os.environ.copy()
    pythonpath = env.get("PYTHONPATH")
    env["PYTHONPATH"] = str(PROJECT_SRC) if not pythonpath else f"{PROJECT_SRC}{os.pathsep}{pythonpath}"
    return subprocess.run(
        [sys.executable, "-m", "tree_ring_memory.cli", *args],
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def test_cli_init_creates_store(tmp_path):
    result = run_cli("init", cwd=tmp_path)

    assert result.returncode == 0
    assert (tmp_path / ".tree-ring" / "memory.sqlite").exists()
    assert "Tree Ring Memory initialized" in result.stdout


def test_cli_remember_and_recall(tmp_path):
    init = run_cli("init", cwd=tmp_path)
    assert init.returncode == 0

    remembered = run_cli("remember", "Use protocol-first design.", "--event-type", "decision", cwd=tmp_path)
    assert remembered.returncode == 0
    assert "mem_" in remembered.stdout

    recalled = run_cli("recall", "protocol", cwd=tmp_path)
    assert recalled.returncode == 0
    assert "Use protocol-first design." in recalled.stdout


def test_cli_remember_secret_tag_returns_policy_error(tmp_path):
    secret_token = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"

    remembered = run_cli(
        "remember",
        "Facade should guard indexed tags.",
        "--event-type",
        "lesson",
        "--tag",
        secret_token,
        cwd=tmp_path,
    )

    assert remembered.returncode == 2
    assert "blocked" in remembered.stderr
    assert "Traceback" not in remembered.stderr


def test_cli_forget_blank_reason_returns_controlled_error(tmp_path):
    forgotten = run_cli("forget", "mem_missing", "--reason", "   ", cwd=tmp_path)

    assert forgotten.returncode == 2
    assert "forget reason is required" in forgotten.stderr
    assert "Traceback" not in forgotten.stderr
