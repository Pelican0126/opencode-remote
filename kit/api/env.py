from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from dotenv import dotenv_values


@dataclass
class ApiEnv:
    region: str

    glm_api_key: str
    glm_base_url_cn: str
    glm_base_url_intl: str
    glm_model: str

    kimi_api_key: str
    kimi_base_url_cn: str
    kimi_base_url_intl: str
    kimi_model: str

    endpoint_path: str

    @classmethod
    def load(cls, root: Path) -> "ApiEnv":
        vals = dotenv_values(root / ".env")
        endpoint = (vals.get("API_ENDPOINT_PATH") or "/chat/completions").strip()
        if not endpoint.startswith("/"):
            endpoint = "/" + endpoint

        return cls(
            region=(vals.get("REGION") or "").strip(),
            glm_api_key=(vals.get("GLM_API_KEY") or "").strip(),
            glm_base_url_cn=(vals.get("GLM_BASE_URL_CN") or "").strip(),
            glm_base_url_intl=(vals.get("GLM_BASE_URL_INTL") or "").strip(),
            glm_model=(vals.get("GLM_MODEL") or "").strip(),
            kimi_api_key=(vals.get("KIMI_API_KEY") or "").strip(),
            kimi_base_url_cn=(vals.get("KIMI_BASE_URL_CN") or "").strip(),
            kimi_base_url_intl=(vals.get("KIMI_BASE_URL_INTL") or "").strip(),
            kimi_model=(vals.get("KIMI_MODEL") or "").strip(),
            endpoint_path=endpoint,
        )

    def glm_base_url(self) -> str:
        return self.glm_base_url_cn if self.region == "cn" else self.glm_base_url_intl

    def kimi_base_url(self) -> str:
        return self.kimi_base_url_cn if self.region == "cn" else self.kimi_base_url_intl
