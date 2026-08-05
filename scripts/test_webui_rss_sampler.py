#!/usr/bin/env python3
"""Contract tests for PID-owned Workbench RSS sampling."""

import json
import os
import subprocess
import sys
import unittest
import tempfile
from pathlib import Path


SCRIPT = Path(__file__).with_name("webui-rss-sampler.py")


class RssSamplerContractTests(unittest.TestCase):
    def test_lists_webkit_baseline_for_controlled_launch(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--list-webkit-pids"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["schemaVersion"], "codelattice.webkit-baseline.v1")
        self.assertIsInstance(payload["pids"], list)

    def test_missing_root_pid_is_a_structured_failure(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--pid", "99999999", "--once"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["error"], "root_process_not_found")

    def test_current_pid_is_the_only_core_without_children(self):
        baseline = subprocess.run(
            [sys.executable, str(SCRIPT), "--list-webkit-pids"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        with tempfile.NamedTemporaryFile("w", suffix=".json") as handle:
            handle.write(baseline)
            handle.flush()
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--pid",
                    str(os.getpid()),
                    "--webkit-baseline-file",
                    handle.name,
                    "--once",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["rootPid"], os.getpid())
        self.assertIn(os.getpid(), payload["corePidList"])
        self.assertGreater(payload["coreMb"], 0)
        self.assertEqual(payload["ownershipMethod"], "controlled-launch-delta-with-bundle-anchor")
        self.assertEqual(payload["webviewPidList"], [])


if __name__ == "__main__":
    unittest.main()
