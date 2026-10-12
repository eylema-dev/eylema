"""Run browser WebGPU state fixtures with Node's built-in test runner."""
import shutil
import subprocess
import unittest
from pathlib import Path


class WebGpuViewTests(unittest.TestCase):
    def test_webgpu_view_fixtures(self):
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Node.js is required for the dependency-free WebGPU state fixtures')
        fixture = Path(__file__).with_name('webgpu-view.test.mjs')
        result = subprocess.run([node, '--test', str(fixture)], capture_output=True, text=True, check=False, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == '__main__':
    unittest.main()
