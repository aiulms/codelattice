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
                "qualityMetrics": {
                    "danglingEdgeCount": 0,
                    "lowConfidenceCallRate": 0.12,
                    "unknownConfidenceEdgeRate": 0.05,
                    "callEdgeCount": 8500,
                    "lowConfidenceEdgeRate": 0.08,
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
        redis_qm = data["targets"]["redis-c"]["qualityMetrics"]
        self.assertAlmostEqual(redis_qm["lowConfidenceCallRate"], 0.12)
        self.assertEqual(redis_qm["danglingEdgeCount"], 0)
        self.assertEqual(redis_qm["callEdgeCount"], 8500)

    def test_compare_baseline_passes_with_good_quality(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
                "qualityRateWarnThreshold": 0.30,
                "qualityRateFailThreshold": 0.50,
                "danglingEdgeFailThreshold": 0,
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
                "nodeCount": 1000,
                "edgeCount": 2000,
                "symbolCount": 900,
                "sourceFileCount": 100,
            },
            "elapsedSeconds": 10.0,
            "qualityMetrics": {
                "danglingEdgeCount": 0,
                "lowConfidenceCallRate": 0.15,
                "lowConfidenceEdgeRate": 0.10,
                "unknownConfidenceEdgeRate": 0.05,
            },
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "pass")

    def test_compare_baseline_fails_on_dangling_edges(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
                "qualityRateWarnThreshold": 0.30,
                "qualityRateFailThreshold": 0.50,
                "danglingEdgeFailThreshold": 0,
            },
            "targets": {
                "catch2-cpp": {
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
            "id": "catch2-cpp",
            "status": "pass",
            "metrics": {
                "nodeCount": 1000,
                "edgeCount": 2000,
                "symbolCount": 900,
                "sourceFileCount": 100,
            },
            "elapsedSeconds": 10.0,
            "qualityMetrics": {
                "danglingEdgeCount": 5,
                "lowConfidenceCallRate": 0.10,
                "lowConfidenceEdgeRate": 0.08,
                "unknownConfidenceEdgeRate": 0.02,
            },
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "fail")
        self.assertTrue(
            any("danglingEdgeCount" in issue for issue in comparison["issues"])
        )

    def test_compare_baseline_warns_on_high_low_confidence_rate(self) -> None:
        baseline = {
            "budgets": {
                "countDropWarnPercent": 10,
                "countDropFailPercent": 20,
                "elapsedIncreaseWarnPercent": 50,
                "elapsedIncreaseFailPercent": 150,
                "qualityRateWarnThreshold": 0.30,
                "qualityRateFailThreshold": 0.50,
                "danglingEdgeFailThreshold": 0,
            },
            "targets": {
                "pip-python": {
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
            "id": "pip-python",
            "status": "pass",
            "metrics": {
                "nodeCount": 1000,
                "edgeCount": 2000,
                "symbolCount": 900,
                "sourceFileCount": 100,
            },
            "elapsedSeconds": 10.0,
            "qualityMetrics": {
                "danglingEdgeCount": 0,
                "lowConfidenceCallRate": 0.35,
                "lowConfidenceEdgeRate": 0.25,
                "unknownConfidenceEdgeRate": 0.10,
            },
        }

        comparison = smoke.compare_result_to_baseline(result, baseline, strict=False)

        self.assertEqual(comparison["status"], "warn")
        self.assertTrue(
            any("lowConfidenceCallRate" in issue for issue in comparison["issues"])
        )

    def test_accept_baseline_includes_quality_metrics(self) -> None:
        results = [
            {
                "id": "pip-python",
                "name": "pip",
                "language": "python",
                "status": "pass",
                "metrics": {
                    "nodeCount": 5000,
                    "edgeCount": 8000,
                    "symbolCount": 4500,
                    "sourceFileCount": 200,
                },
                "qualityMetrics": {
                    "danglingEdgeCount": 0,
                    "lowConfidenceCallRate": 0.20,
                    "unknownConfidenceEdgeRate": 0.05,
                    "callEdgeCount": 6000,
                    "lowConfidenceEdgeRate": 0.10,
                },
                "elapsedSeconds": 5.0,
            },
        ]

        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "baseline.json"
            smoke.write_baseline(output, results, existing=None)
            data = smoke.load_baseline(output)

        target = data["targets"]["pip-python"]
        self.assertIn("qualityMetrics", target)
        qm = target["qualityMetrics"]
        self.assertAlmostEqual(qm["lowConfidenceCallRate"], 0.20)
        self.assertEqual(qm["danglingEdgeCount"], 0)
        self.assertEqual(qm["callEdgeCount"], 6000)


if __name__ == "__main__":
    unittest.main()
