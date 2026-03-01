import respx
from httpx import Response

from kit.api.env import ApiEnv
from kit.api.tester import run_connectivity_test


@respx.mock
def test_api_tester_mock():
    env = ApiEnv(
        region="cn",
        glm_api_key="x",
        glm_base_url_cn="https://open.bigmodel.cn/api/coding/paas/v4",
        glm_base_url_intl="https://api.z.ai/api/coding/paas/v4",
        glm_model="GLM-5",
        kimi_api_key="y",
        kimi_base_url_cn="https://api.moonshot.cn/v1",
        kimi_base_url_intl="https://api.moonshot.ai/v1",
        kimi_model="kimi-k2.5",
        endpoint_path="/chat/completions",
    )

    respx.post("https://open.bigmodel.cn/api/coding/paas/v4/chat/completions").mock(
        return_value=Response(200, json={"id": "ok"})
    )
    respx.post("https://api.moonshot.cn/v1/chat/completions").mock(
        return_value=Response(200, json={"id": "ok"})
    )

    payload = run_connectivity_test(env)
    assert payload["region"] == "cn"
    assert len(payload["results"]) == 2
    assert all(r["http_status"] == 200 for r in payload["results"])
