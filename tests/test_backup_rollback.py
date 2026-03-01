from pathlib import Path

from kit.json.backup import create_backup, rollback_by_id


def test_backup_and_rollback(tmp_path: Path):
    p = tmp_path / "a.json"
    p.write_text('{"x":1}', encoding="utf-8")
    backup_id, _ = create_backup(tmp_path, [p])

    p.write_text('{"x":2}', encoding="utf-8")
    payload = rollback_by_id(tmp_path, backup_id)
    assert payload["backup_id"] == backup_id
    assert p.read_text(encoding="utf-8") == '{"x":1}'
