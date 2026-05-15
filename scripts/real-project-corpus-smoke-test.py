#!/usr/bin/env python3
"""Unit tests for real-project-corpus-smoke.py baseline behavior."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parent / "real-project-corpus-smoke.py"
SPEC = importlib.util.spec_from_file_location("real_project_corpus_smoke", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class BaselineComparisonTests(unittest.TestCase):
    def test_compare_baseline_passes_within_budget(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
            },
            "targets": {
                "redis-c": {
                    "metrics": {
                        "nodeCount": 1000,
                        "edgeCount": 2000,
                        "symbolCount": 900,
                        "sourceFileCount": 100,
                    },
                    "elapsedSeconds": 10.0,
                }
            },
        }
        result = {
            "id": "redis-c",
            "status": "pass",
            "metrics": {
                "nodeCount": 960,
                "edgeCount": 1900,
                "symbolCount": 870,
                "sourceFileCount": 99,
            },
            "elapsedSeconds": 12.0,
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "pass")
        self.assertEqual(comparison["issues"], [])

    def test_compare_baseline_fails_large_count_drop(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
            },
            "targets": {
                "catch2-cpp": {
                    "metrics": {
                        "nodeCount": 1000,
                        "edgeCount": 2000,
                        "symbolCount": 1000,
                        "sourceFileCount": 200,
                    },
                    "elapsedSeconds": 20.0,
                }
            },
        }
        result = {
            "id": "catch2-cpp",
            "status": "pass",
            "metrics": {
                "nodeCount": 950,
                "edgeCount": 1400,
                "symbolCount": 990,
                "sourceFileCount": 200,
            },
            "elapsedSeconds": 21.0,
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "fail")
        self.assertIn("edgeCount dropped 30.0% from baseline", comparison["issues"][0])

    def test_compare_baseline_warns_elapsed_regression(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
            },
            "targets": {
                "pip-python": {
                    "metrics": {
                        "nodeCount": 1000,
                        "edgeCount": 2000,
                        "symbolCount": 1000,
                        "sourceFileCount": 200,
                    },
                    "elapsedSeconds": 30.0,
                }
            },
        }
        result = {
            "id": "pip-python",
            "status": "pass",
            "metrics": {
                "nodeCount": 1000,
                "edgeCount": 2000,
                "symbolCount": 1000,
                "sourceFileCount": 200,
            },
            "elapsedSeconds": 50.0,
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "warn")
        self.assertIn("elapsedSeconds increased 66.7% from baseline", comparison["issues"][0])

    def test_accept_baseline_writes_successful_results_only(self) -> None:
        results = [
            {
                "id": "redis-c",
                "name": "Redis",
                "language": "c",
                "status": "pass",
                "metrics": {
                    "nodeCount": 10967,
                    "edgeCount": 11486,
                    "symbolCount": 10751,
                    "sourceFileCount": 133,
                },
                "elapsedSeconds": 3.0,
            },
            {
                "id": "broken-target",
                "name": "Broken",
                "language": "python",
                "status": "fail",
                "metrics": {
                    "nodeCount": 0,
                    "edgeCount": 0,
                    "symbolCount": 0,
                    "sourceFileCount": 0,
                },
                "elapsedSeconds": 0.1,
            },
        ]

        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "baseline.json"
            smoke.write_baseline(output, results, existing=None)
            data = smoke.load_baseline(output)

        self.assertIn("redis-c", data["targets"])
        self.assertNotIn("broken-target", data["targets"])
        self.assertEqual(data["targets"]["redis-c"]["metrics"]["symbolCount"], 10751)


if __name__ == "__main__":
    unittest.main()
