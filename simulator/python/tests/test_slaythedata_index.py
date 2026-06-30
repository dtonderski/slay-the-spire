import sqlite3
import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest

from sts.slaythedata_index import (
    chunk_export_args,
    export_guided_run_script,
    select_guided_collection_candidates,
    slaythedata_index_status,
)


class SlayTheDataIndexTests(unittest.TestCase):
    def test_slaythedata_index_status_reports_missing_database(self):
        with tempfile.TemporaryDirectory() as tmp:
            status = slaythedata_index_status(Path(tmp) / "missing.sqlite3")

        self.assertFalse(status["ok"])
        self.assertFalse(status["exists"])
        self.assertIn("missing", status["problems"][0])

    def test_slaythedata_index_status_summarizes_exportable_candidates(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                CREATE TABLE chunk_files (id INTEGER);
                CREATE TABLE archive_files (source_file TEXT, status TEXT);
                """
            )
            conn.executemany(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                [
                    (1, "IRONCLAD", 0, 50, 0, 0, 0, 0, "A", 1, 50, 10, 2, 1, 0),
                    (2, "IRONCLAD", 0, 50, 0, 0, 0, 1, "B", 0, 50, 10, 2, 1, 0),
                ],
            )
            conn.execute("INSERT INTO chunk_runs VALUES (1)")
            conn.execute("INSERT INTO chunk_files VALUES (1)")
            conn.executemany(
                "INSERT INTO archive_files VALUES (?, ?)",
                [("a.json", "indexed"), ("b.json", "pending")],
            )
            conn.commit()
            conn.close()

            status = slaythedata_index_status(db)

        self.assertTrue(status["ok"])
        self.assertFalse(status["counts_included"])
        self.assertNotIn("runs_count", status)
        self.assertNotIn("chunk_runs_count", status)
        self.assertTrue(status["exportable_candidate_available"])
        self.assertEqual(status["archive_status_counts"], {"indexed": 1, "pending": 1})
        self.assertIn("partial", status["warnings"][0])

    def test_slaythedata_index_status_can_include_exact_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                CREATE TABLE chunk_files (id INTEGER);
                """
            )
            conn.execute(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (1, "IRONCLAD", 0, 50, 0, 0, 0, 0, "A", 1, 50, 10, 2, 1, 0),
            )
            conn.execute("INSERT INTO chunk_runs VALUES (1)")
            conn.execute("INSERT INTO chunk_files VALUES (1)")
            conn.commit()
            conn.close()

            status = slaythedata_index_status(db, include_counts=True)

        self.assertTrue(status["counts_included"])
        self.assertEqual(status["runs_count"], 1)
        self.assertEqual(status["chunk_runs_count"], 1)
        self.assertEqual(status["chunk_files_count"], 1)

    def test_slaythedata_index_status_reports_missing_required_tables(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.execute("CREATE TABLE runs (id INTEGER PRIMARY KEY)")
            conn.commit()
            conn.close()

            status = slaythedata_index_status(db)

        self.assertFalse(status["ok"])
        self.assertIn("chunk_runs", status["problems"][0])

    def test_select_guided_collection_candidates_filters_exportable_supported_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                """
            )
            conn.executemany(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                [
                    (1, "IRONCLAD", 0, 20, 0, 0, 0, 0, "A", 0, 20, 3, 2, 1, 0),
                    (2, "IRONCLAD", 0, 40, 0, 0, 0, 1, "B", 0, 40, 9, 4, 2, 1),
                    (3, "THE_SILENT", 0, 50, 0, 0, 0, 0, "C", 0, 50, 9, 4, 2, 1),
                    (4, "IRONCLAD", 0, 30, 0, 0, 0, 0, "D", 1, 30, 5, 1, 0, 0),
                    (5, "IRONCLAD", 0, 99, 1, 0, 0, 0, "E", 0, 99, 5, 1, 0, 0),
                    (6, "IRONCLAD", 0, 55, 0, 0, 0, 0, "F", 0, 10, 5, 1, 0, 0),
                ],
            )
            conn.executemany("INSERT INTO chunk_runs VALUES (?)", [(1,), (2,), (3,), (4,), (6,)])
            conn.commit()
            conn.close()

            rows = select_guided_collection_candidates(db, min_floor_reached=1, limit=10)

        self.assertEqual([row["id"] for row in rows], [4, 1, 6])
        self.assertEqual(rows[0]["seed_played"], "D")
        self.assertTrue(rows[0]["victory"])

    def test_candidate_selection_can_require_long_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                """
            )
            conn.executemany(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                [
                    (1, "IRONCLAD", 0, 55, 0, 0, 0, 0, "SHORT", 0, 20, 3, 2, 1, 0),
                    (2, "IRONCLAD", 0, 50, 0, 0, 0, 0, "LONG", 0, 50, 9, 4, 2, 1),
                ],
            )
            conn.executemany("INSERT INTO chunk_runs VALUES (?)", [(1,), (2,)])
            conn.commit()
            conn.close()

            rows = select_guided_collection_candidates(
                db,
                min_floor_reached=45,
                min_path_length=45,
                limit=10,
            )

        self.assertEqual([row["id"] for row in rows], [2])

    def test_candidate_selection_can_skip_global_ranking_for_fast_ui_loads(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                """
            )
            conn.executemany(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                [
                    (1, "IRONCLAD", 0, 45, 0, 0, 0, 0, "FIRST", 0, 45, 8, 1, 1, 0),
                    (2, "IRONCLAD", 0, 55, 0, 0, 0, 0, "BETTER", 1, 55, 20, 10, 10, 5),
                ],
            )
            conn.executemany("INSERT INTO chunk_runs VALUES (?)", [(1,), (2,)])
            conn.commit()
            conn.close()

            rows = select_guided_collection_candidates(
                db,
                min_floor_reached=45,
                min_path_length=45,
                min_card_choices=8,
                min_event_choices=1,
                min_shop_purchases=1,
                ranked=False,
                limit=10,
            )

        self.assertEqual([row["id"] for row in rows], [1, 2])

    def test_candidate_selection_filters_and_scores_guided_decisions(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "runs.sqlite3"
            conn = sqlite3.connect(db)
            conn.executescript(
                """
                CREATE TABLE runs (
                    id INTEGER PRIMARY KEY,
                    character_chosen TEXT,
                    ascension_level INTEGER,
                    floor_reached INTEGER,
                    is_daily INTEGER,
                    is_endless INTEGER,
                    is_trial INTEGER,
                    unsupported_any INTEGER,
                    seed_played TEXT,
                    victory INTEGER,
                    path_length INTEGER,
                    card_choice_count INTEGER,
                    event_choice_count INTEGER,
                    shop_purchase_count INTEGER,
                    potion_usage_count INTEGER
                );
                CREATE TABLE chunk_runs (run_id INTEGER);
                """
            )
            conn.executemany(
                "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                [
                    (1, "IRONCLAD", 0, 50, 0, 0, 0, 0, "LOW", 0, 50, 10, 0, 1, 0),
                    (2, "IRONCLAD", 0, 50, 0, 0, 0, 0, "RICH", 0, 50, 10, 2, 2, 3),
                    (3, "IRONCLAD", 0, 50, 0, 0, 0, 0, "SPARSE", 0, 50, 2, 0, 0, 0),
                ],
            )
            conn.executemany("INSERT INTO chunk_runs VALUES (?)", [(1,), (2,), (3,)])
            conn.commit()
            conn.close()

            rows = select_guided_collection_candidates(
                db,
                min_floor_reached=45,
                min_path_length=45,
                min_event_choices=1,
                min_shop_purchases=1,
                limit=10,
            )

        self.assertEqual([row["id"] for row in rows], [2])
        self.assertEqual(rows[0]["guided_score"], 10 + 2 * 2 + 2 * 3 + 3)

    def test_chunk_export_args_builds_indexer_invocation_for_run_ids(self):
        args = chunk_export_args(
            db_path="runs.sqlite3",
            chunks_dir="chunks",
            output_path="export.jsonl",
            run_ids=[4, 1],
            indexer_path="tools/slaythedata/index_slaythedata.py",
        )

        self.assertEqual(
            args,
            [
                "tools/slaythedata/index_slaythedata.py",
                "chunk-export",
                "--db",
                "runs.sqlite3",
                "--chunks-dir",
                "chunks",
                "--where",
                "id IN (4,1)",
                "--out",
                "export.jsonl",
            ],
        )

    def test_chunk_export_args_rejects_empty_run_ids(self):
        with self.assertRaises(ValueError):
            chunk_export_args(output_path="export.jsonl", run_ids=[])

    def test_export_guided_run_script_invokes_chunk_export_and_loads_script(self):
        calls = []

        def runner(command, *, cwd, capture_output, text, check, timeout):
            calls.append(
                {
                    "command": command,
                    "cwd": cwd,
                    "capture_output": capture_output,
                    "text": text,
                    "check": check,
                    "timeout": timeout,
                }
            )
            output = Path(command[command.index("--out") + 1])
            output.write_text(
                '{"run_id": 7, "event": {"character_chosen": "IRONCLAD", "ascension_level": 0, "seed_played": "ABC", "card_choices": [{"floor": 1, "picked": "Inflame"}]}}\n',
                encoding="utf-8",
            )
            return SimpleNamespace(returncode=0, stdout="chunk-exported runs: 1", stderr="")

        script = export_guided_run_script(
            7,
            db_path="runs.sqlite3",
            chunks_dir="chunks",
            indexer_path="tools/slaythedata/index_slaythedata.py",
            timeout_seconds=3,
            runner=runner,
        )

        self.assertEqual(script["source"]["run_id"], 7)
        self.assertEqual(script["config"]["seed_played"], "ABC")
        self.assertEqual(calls[0]["command"][1], "tools/slaythedata/index_slaythedata.py")
        self.assertEqual(calls[0]["command"][2], "chunk-export")
        self.assertEqual(calls[0]["timeout"], 3)

    def test_export_guided_run_script_reports_failed_export(self):
        def runner(*_args, **_kwargs):
            return SimpleNamespace(returncode=2, stdout="", stderr="boom")

        with self.assertRaisesRegex(RuntimeError, "boom"):
            export_guided_run_script(7, runner=runner)


if __name__ == "__main__":
    unittest.main()
