from kit.api.env import ApiEnv
from kit.api.validator import validate_env


def test_api_validate_ok():
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
    ok, issues = validate_env(env)
    assert ok
    assert not issues
