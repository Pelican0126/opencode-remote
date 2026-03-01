from pathlib import Path

from kit.json.fixer import fix_files
from kit.json.validator import validate_json_file


def test_fix_json(tmp_path: Path):
    p = tmp_path / "bad.json"
    p.write_text("\ufeff{\n //c\n 'a': 'b',\n}\n", encoding="utf-8")

    out = fix_files(tmp_path, [p], apply=True, backup=True)
    assert out["backup_id"]
    assert out["results"][0]["success"] is True

    ok, issue = validate_json_file(p)
    assert ok, issue
