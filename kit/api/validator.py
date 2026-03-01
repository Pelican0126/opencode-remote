from __future__ import annotations

from dataclasses import dataclass, asdict
from typing import Any
from urllib.parse import urlparse

from .env import ApiEnv


@dataclass
class ApiValidationIssue:
    field: str
    message: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def _valid_url(u: str) -> bool:
    try:
        p = urlparse(u)
        return p.scheme in {"http", "https"} and bool(p.netloc)
    except Exception:
        return False


def validate_env(env: ApiEnv) -> tuple[bool, list[ApiValidationIssue]]:
    issues: list[ApiValidationIssue] = []

    if env.region not in {"cn", "intl"}:
        issues.append(ApiValidationIssue("REGION", "must be 'cn' or 'intl'"))

    for f in [
        ("GLM_BASE_URL_CN", env.glm_base_url_cn),
        ("GLM_BASE_URL_INTL", env.glm_base_url_intl),
        ("KIMI_BASE_URL_CN", env.kimi_base_url_cn),
        ("KIMI_BASE_URL_INTL", env.kimi_base_url_intl),
    ]:
        if not _valid_url(f[1]):
            issues.append(ApiValidationIssue(f[0], "invalid http(s) URL"))

    if not env.glm_model:
        issues.append(ApiValidationIssue("GLM_MODEL", "missing"))
    if not env.kimi_model:
        issues.append(ApiValidationIssue("KIMI_MODEL", "missing"))

    if not env.glm_api_key:
        issues.append(ApiValidationIssue("GLM_API_KEY", "missing"))
    if not env.kimi_api_key:
        issues.append(ApiValidationIssue("KIMI_API_KEY", "missing"))

    if not env.endpoint_path.startswith("/"):
        issues.append(ApiValidationIssue("API_ENDPOINT_PATH", "must start with '/'"))

    return len(issues) == 0, issues
