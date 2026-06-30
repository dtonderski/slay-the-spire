import sqlite3
import tempfile
from pathlib import Path
import unittest

from sts.slaythedata_index import chunk_export_args, select_guided_collection_candidates


class SlayTheDataIndexTests(unittest.TestCase):
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
                ],
            )
            conn.executemany("INSERT INTO chunk_runs VALUES (?)", [(1,), (2,), (3,), (4,)])
            conn.commit()
            conn.close()

            rows = select_guided_collection_candidates(db, min_floor_reached=1, limit=10)

        self.assertEqual([row["id"] for row in rows], [4, 1])
        self.assertEqual(rows[0]["seed_played"], "D")
        self.assertTrue(rows[0]["victory"])

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


if __name__ == "__main__":
    unittest.main()

