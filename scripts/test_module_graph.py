#!/usr/bin/env python3
"""moduleGraph 聚合纯函数测试（TDD）。

stop-line：只归并已有边，不新造边；模块边 count 之和 == 底层可聚合边数。
"""

from __future__ import annotations

import importlib.util
import pathlib
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "snapshot_gen", HERE / "codelattice-snapshot-gen.py"
)
gen = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(gen)


class ModuleIdTests(unittest.TestCase):
    def test_file_two_directory_levels(self):
        self.assertEqual(gen.module_id_from_file("crates/cli/src/main.rs"), "crates/cli")
        self.assertEqual(gen.module_id_from_file("src/lib.rs"), "src")
        self.assertEqual(gen.module_id_from_file("main.rs"), "(root)")

    def test_redacted_root_is_stripped(self):
        self.assertEqual(
            gen.module_id_from_file("<redacted-root>/crates/core/src/lib.rs"),
            "crates/core",
        )

    def test_empty_file_is_none(self):
        self.assertIsNone(gen.module_id_from_file(""))
        self.assertIsNone(gen.module_id_from_file(None))

    def test_rust_module_path_fallback_first_two_segments(self):
        self.assertEqual(gen.module_id_from_rust_module_path("crate::foo::bar"), "crate::foo")
        self.assertEqual(gen.module_id_from_rust_module_path("crate"), "crate")
        self.assertIsNone(gen.module_id_from_rust_module_path(""))


class ModuleGraphAggregateTests(unittest.TestCase):
    def test_aggregates_inter_module_edges_only_and_keeps_min_confidence(self):
        graph = {
            "nodes": [
                {"id": "a", "kind": "symbol", "file": "crates/cli/src/main.rs", "label": "a"},
                {"id": "b", "kind": "symbol", "file": "crates/core/src/lib.rs", "label": "b"},
                {"id": "c", "kind": "symbol", "file": "crates/core/src/lib.rs", "label": "c"},
            ],
            "edges": [
                {"source": "a", "target": "b", "kind": "calls", "confidence": 0.9, "reason": "r1"},
                {"source": "a", "target": "b", "kind": "imports", "confidence": 0.5, "reason": "r2"},
                {"source": "b", "target": "c", "kind": "calls", "confidence": 0.99, "reason": "same-mod"},
            ],
        }
        out = gen.build_module_graph(graph, "rust")
        self.assertEqual({m["id"] for m in out["modules"]}, {"crates/cli", "crates/core"})
        self.assertEqual(len(out["edges"]), 1)
        edge = out["edges"][0]
        self.assertEqual(edge["source"], "crates/cli")
        self.assertEqual(edge["target"], "crates/core")
        self.assertEqual(edge["count"], 2)
        self.assertEqual(sorted(edge["kinds"]), ["calls", "imports"])
        self.assertEqual(edge["minConfidence"], 0.5)
        self.assertEqual(edge["reasons"], ["r1", "r2"])
        self.assertEqual(sum(e["count"] for e in out["edges"]), 2)
        self.assertFalse(out["truncated"])

    def test_does_not_invent_edges_when_all_calls_are_internal(self):
        graph = {
            "nodes": [
                {"id": "a", "kind": "symbol", "file": "src/lib.rs", "label": "a"},
                {"id": "b", "kind": "symbol", "file": "src/main.rs", "label": "b"},
            ],
            "edges": [
                {"source": "a", "target": "b", "kind": "calls", "confidence": 0.8},
            ],
        }
        out = gen.build_module_graph(graph, "rust")
        self.assertEqual(len(out["modules"]), 1)
        self.assertEqual(out["modules"][0]["id"], "src")
        self.assertEqual(out["edges"], [])
        self.assertEqual(sum(e["count"] for e in out["edges"]), 0)

    def test_package_without_file_is_omitted_not_unknown_blob(self):
        graph = {
            "nodes": [
                {"id": "pkg", "kind": "package", "file": "", "label": "pkg"},
                {"id": "real", "kind": "symbol", "file": "src/lib.rs", "label": "real"},
            ],
            "edges": [
                {"source": "pkg", "target": "real", "kind": "owns"},
            ],
        }
        out = gen.build_module_graph(graph, "rust")
        self.assertEqual({m["id"] for m in out["modules"]}, {"src"})
        self.assertEqual(out["edges"], [])

    def test_missing_path_goes_to_unknown_not_guessed(self):
        graph = {
            "nodes": [
                {"id": "ghost", "kind": "symbol", "file": "", "label": "ghost"},
                {"id": "real", "kind": "symbol", "file": "src/api/mod.rs", "label": "real"},
            ],
            "edges": [
                {"source": "ghost", "target": "real", "kind": "calls", "confidence": 0.4},
            ],
        }
        out = gen.build_module_graph(graph, "python")
        ids = {m["id"] for m in out["modules"]}
        self.assertIn("(unknown)", ids)
        self.assertIn("src/api", ids)
        self.assertEqual(out["edges"][0]["source"], "(unknown)")
        self.assertEqual(out["edges"][0]["count"], 1)

    def test_rust_uses_module_path_only_when_file_missing(self):
        graph = {
            "nodes": [
                {"id": "x", "kind": "symbol", "file": "", "modulePath": "crate::calls::index", "label": "x"},
                {"id": "y", "kind": "symbol", "file": "", "modulePath": "crate::graph", "label": "y"},
            ],
            "edges": [{"source": "x", "target": "y", "kind": "calls", "confidence": 0.7}],
        }
        out = gen.build_module_graph(graph, "rust")
        ids = {m["id"] for m in out["modules"]}
        self.assertEqual(ids, {"crate::calls", "crate::graph"})
        self.assertEqual(out["edges"][0]["count"], 1)


if __name__ == "__main__":
    unittest.main()
