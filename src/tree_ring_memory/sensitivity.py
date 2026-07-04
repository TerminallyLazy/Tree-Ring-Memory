from __future__ import annotations

from dataclasses import dataclass
import re


class SensitiveMemoryBlocked(ValueError):
    """Raised when policy blocks storing sensitive memory."""


@dataclass(slots=True)
class SensitivityResult:
    sensitivity: str
    redacted_text: str
    findings: list[str]
    requires_explicit_approval: bool = False


class SensitivityGuard:
    SECRET_PATTERNS = [
        ("openai_key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b")),
        ("github_token", re.compile(r"\bgh[pousr]_[A-Za-z0-9_]{20,}\b")),
        ("github_fine_grained_token", re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b")),
        ("aws_access_key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
        ("bearer_token", re.compile(r"\bBearer\s+[A-Za-z0-9._~+/=-]{20,}\b", re.IGNORECASE)),
        (
            "private_key",
            re.compile(
                r"-----BEGIN (?P<pem_label>(?:(?:RSA|EC|OPENSSH|DSA|ENCRYPTED) )?PRIVATE KEY)-----.*?"
                r"-----END (?P=pem_label)-----",
                re.DOTALL,
            ),
        ),
        (
            "secret_assignment",
            re.compile(
                r"(?i)\b(?:[A-Z0-9_]*(?:API[_-]?KEY|PRIVATE[_-]?KEY|SECRET(?:_ACCESS_KEY)?|TOKEN|PASSWORD|PASSWD|PWD)|"
                r"DATABASE_URL)\s*=\s*(?:\"[^\"\n]*\"|'[^'\n]*'|\S+)"
            ),
        ),
    ]
    CATEGORY_PATTERNS = [
        ("health", re.compile(r"(?i)\b(diagnosis|diagnosed|medical|medication|prescription|therapy|hospital)\b")),
        ("financial", re.compile(r"(?i)\b(bank account|routing number|credit card|tax return|paystub|salary)\b")),
        ("legal", re.compile(r"(?i)\b(lawsuit|attorney|legal advice|court order|subpoena|contract dispute)\b")),
        ("personal_identifier", re.compile(r"(?i)\b(ssn|social security|passport|driver'?s license)\b")),
    ]

    def __init__(self, *, block_secret_storage: bool = True) -> None:
        self.block_secret_storage = block_secret_storage

    def inspect(self, text: str) -> SensitivityResult:
        findings: list[str] = []
        redacted = text
        for name, pattern in self.SECRET_PATTERNS:
            if pattern.search(redacted):
                findings.append(name)
                redacted = pattern.sub("[REDACTED_SECRET]", redacted)
        if findings:
            return SensitivityResult("secret", redacted, findings)
        for category, pattern in self.CATEGORY_PATTERNS:
            if pattern.search(text):
                return SensitivityResult(category, text, [category], requires_explicit_approval=True)
        return SensitivityResult("normal", text, [])

    def check_or_raise(self, text: str) -> SensitivityResult:
        result = self.inspect(text)
        if result.sensitivity == "secret" and self.block_secret_storage:
            raise SensitiveMemoryBlocked("secret-like memory is blocked by policy")
        return result
