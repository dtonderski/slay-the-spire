import contextlib
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import archive_schema_v0_traces as archive


class ArchiveDiscoveryTests(unittest.TestCase):
    def make_roots(self, repo: Path) -> None:
        for relative in archive.ACTIVE_ROOTS:
            (repo / relative).mkdir(parents=True, exist_ok=True)

    def test_missing_active_root_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "required active discovery root"):
                archive.candidate_paths(Path(directory))

    def test_non_communication_jsonl_is_forbidden_in_active_roots(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            self.make_roots(repo)
            path = repo / archive.ACTIVE_ROOTS[0] / "not-a-communication-trace.jsonl"
            records = [
                {"type": "metadata", "schema": 1, "source": "live_trace"},
                {"type": "action", "step": 1, "command": "STATE"},
                {
                    "type": "state",
                    "step": 1,
                    "message": {"boundary_schema": 1},
                },
            ]
            path.write_text("\n".join(json.dumps(record) for record in records) + "\n")
            with contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, "unsupported files"):
                    archive.audit_active(repo)

    def test_strict_v1_is_the_only_accepted_active_schema(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            self.make_roots(repo)
            path = repo / archive.ACTIVE_ROOTS[0] / "strict.jsonl"
            records = [
                {
                    "type": "metadata",
                    "schema": 1,
                    "boundary_schema": 1,
                    "source": "communication_mod",
                },
                {"type": "action", "step": 1, "command": "START IRONCLAD 0 1"},
                {
                    "type": "state",
                    "step": 1,
                    "message": {"boundary_schema": 1},
                },
            ]
            path.write_text("\n".join(json.dumps(record) for record in records) + "\n")
            with contextlib.redirect_stdout(io.StringIO()):
                archive.audit_active(repo)


class AggregateManifestTests(unittest.TestCase):
    def assert_rejected(self, manifest: dict, message: str) -> None:
        repo = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "aggregate-manifest.json"
            raw = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
            path.write_bytes(raw)
            path.with_suffix(".json.sha256").write_text(
                f"{hashlib.sha256(raw).hexdigest()}  aggregate-manifest.json\n"
            )
            result = subprocess.run(
                [
                    "uv",
                    "run",
                    "--python",
                    "3.12",
                    str(repo / "tools/communication/verify_schema_v0_aggregate.py"),
                    str(path),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(message, result.stdout)

    def manifest(self) -> dict:
        repo = Path(__file__).resolve().parents[2]
        source = (
            repo
            / "simulator/verification/schema_v0_archive_2026-08-07/aggregate-manifest.json"
        )
        return json.loads(source.read_text())

    def test_aggregate_entry_omission_is_rejected_before_byte_scan(self) -> None:
        manifest = self.manifest()
        manifest["entries"].pop()
        self.assert_rejected(manifest, "exactly cover")

    def test_aggregate_archive_declaration_omission_is_rejected(self) -> None:
        manifest = self.manifest()
        manifest["archives"].pop()
        self.assert_rejected(manifest, "exactly cover")


if __name__ == "__main__":
    unittest.main()
