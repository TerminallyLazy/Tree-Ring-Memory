import pytest

from tree_ring_memory.sensitivity import SensitivityGuard, SensitiveMemoryBlocked


def test_blocks_openai_style_secret():
    guard = SensitivityGuard()

    with pytest.raises(SensitiveMemoryBlocked):
        guard.check_or_raise("Use key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890")


def test_redacts_password_assignment():
    guard = SensitivityGuard(block_secret_storage=False)

    result = guard.inspect("password = hunter2")

    assert result.sensitivity == "secret"
    assert "hunter2" not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


def test_redacts_whole_private_key_block():
    guard = SensitivityGuard(block_secret_storage=False)
    text = "\n".join([
        "keep this context",
        "-----BEGIN PRIVATE KEY-----",
        "not-a-real-key-body",
        "-----END PRIVATE KEY-----",
    ])

    result = guard.inspect(text)

    assert result.sensitivity == "secret"
    assert "not-a-real-key-body" not in result.redacted_text
    assert "-----END PRIVATE KEY-----" not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


@pytest.mark.parametrize("label", ["ENCRYPTED PRIVATE KEY", "DSA PRIVATE KEY"])
def test_redacts_common_private_key_block_labels(label):
    guard = SensitivityGuard(block_secret_storage=False)
    text = "\n".join([
        f"-----BEGIN {label}-----",
        "not-a-real-key-body",
        f"-----END {label}-----",
    ])

    result = guard.inspect(text)

    assert result.sensitivity == "secret"
    assert "not-a-real-key-body" not in result.redacted_text
    assert f"-----END {label}-----" not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


def test_redacts_fine_grained_github_token():
    guard = SensitivityGuard(block_secret_storage=False)
    token = "github_pat_abcdefghijklmnopqrstuvwxyz1234567890"

    result = guard.inspect(f"GITHUB_TOKEN={token}")

    assert result.sensitivity == "secret"
    assert token not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


@pytest.mark.parametrize(
    ("assignment", "secret_value"),
    [
        ("API_KEY=abc123def456ghi789", "abc123def456ghi789"),
        ("DATABASE_URL=postgres://user:pass@localhost:5432/app", "postgres://user:pass@localhost:5432/app"),
        ("AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
        ("PRIVATE_KEY=inline-private-key", "inline-private-key"),
        ("JWT_PRIVATE_KEY=inline-jwt-private-key", "inline-jwt-private-key"),
    ],
)
def test_redacts_env_style_secret_assignments(assignment, secret_value):
    guard = SensitivityGuard(block_secret_storage=False)

    result = guard.inspect(assignment)

    assert result.sensitivity == "secret"
    assert secret_value not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


def test_detects_health_text_as_sensitive():
    guard = SensitivityGuard()

    result = guard.inspect("User mentioned a private diagnosis during setup.")

    assert result.sensitivity == "health"
    assert result.requires_explicit_approval is True


def test_normal_text_passes():
    guard = SensitivityGuard()

    result = guard.inspect("Use SQLite for local storage.")

    assert result.sensitivity == "normal"
    assert result.redacted_text == "Use SQLite for local storage."
