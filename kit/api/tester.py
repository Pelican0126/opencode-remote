from __future__ import annotations

import socket
import time
from dataclasses import dataclass, asdict
from typing import Any
from urllib.parse import urlparse

import httpx

from .env import ApiEnv


@dataclass
class ApiTestResult:
    provider: str
    base_url: str
    endpoint_path: str
    model: str
    dns_ok: bool | None
    tls_ok: bool | None
    http_status: int | None
    elapsed_ms: int
    error: str | None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def _dns_probe(base_url: str) -> bool:
    host = urlparse(base_url).hostname
    if not host:
        return False
    try:
        socket.getaddrinfo(host, None)
        return True
    except Exception:
        return False


def _post_chat(client: httpx.Client, url: str, api_key: str, model: str) -> httpx.Response:
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }
    payload = {
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1,
        "temperature": 0,
    }
    return client.post(url, headers=headers, json=payload, timeout=10.0)


def run_connectivity_test(env: ApiEnv) -> dict[str, Any]:
    results: list[ApiTestResult] = []

    def run_one(provider: str, base_url: str, api_key: str, model: str) -> None:
        endpoint = env.endpoint_path
        full = base_url.rstrip("/") + endpoint

        t0 = time.time()
        dns_ok = None
        tls_ok = None
        status = None
        err = None

        try:
            dns_ok = _dns_probe(base_url)
        except Exception:
            dns_ok = False

        try:
            with httpx.Client(http2=False, follow_redirects=True, trust_env=False) as client:
                resp = _post_chat(client, full, api_key, model)
                status = resp.status_code
                # TLS OK is inferred when request succeeded to the point of HTTP response.
                tls_ok = True
        except httpx.ConnectError as e:
            err = f"connect_error: {e}"
            tls_ok = False
        except httpx.ReadTimeout:
            err = "read_timeout"
        except Exception as e:
            err = str(e)

        elapsed = int((time.time() - t0) * 1000)
        results.append(
            ApiTestResult(
                provider=provider,
                base_url=base_url,
                endpoint_path=endpoint,
                model=model,
                dns_ok=dns_ok,
                tls_ok=tls_ok,
                http_status=status,
                elapsed_ms=elapsed,
                error=err,
            )
        )

    run_one("glm", env.glm_base_url(), env.glm_api_key, env.glm_model)
    run_one("kimi", env.kimi_base_url(), env.kimi_api_key, env.kimi_model)

    return {"region": env.region, "results": [r.to_dict() for r in results]}
