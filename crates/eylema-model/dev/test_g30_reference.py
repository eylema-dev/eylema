import hashlib
import json
import unittest
from pathlib import Path

import g30_reference

ROOT = Path(__file__).parent
EXPECTED_SHA256 = "38a4986871276927cd60eab39773d394f238d83edf1f98d92e028c992d9436a7"
PROTOCOL_SHA256 = "2824b75b5e24a47427d41ead074bc29981eb355b02f705f032eb11d41d45c747"

class G30ReferenceTests(unittest.TestCase):
    def test_reference_descriptor_detects_changed_file(self):
        import tempfile
        with tempfile.TemporaryFile('w+b') as source:
            source.write(b'x'); source.flush()
            identity = g30_reference.file_identity(source)
            g30_reference.require_unchanged(source, identity)
            source.write(b'y'); source.flush()
            with self.assertRaises(ValueError):
                g30_reference.require_unchanged(source, identity)
    def test_independent_f64_bf16_rows_and_nonfinite_rejection(self):
        import struct
        row = struct.pack('<HHH', 0x3f80, 0xc000, 0x3f00)
        self.assertEqual(g30_reference.f64_dot(memoryview(row).cast('H'), [2., 3., 4.]), -2.)
        with self.assertRaises(ValueError):
            g30_reference.f64_dot(memoryview(struct.pack('<H', 0x7f80)).cast('H'), [1.])
        with self.assertRaises(ValueError):
            g30_reference.f64_dot(memoryview(row).cast('H'), [1.])
    def test_frozen_hashes_and_shapes(self):
        expected_path = ROOT / "../tests/fixtures/g30/synthetic-reference.json"
        expected_bytes = expected_path.read_bytes()
        self.assertEqual(hashlib.sha256(expected_bytes).hexdigest(), EXPECTED_SHA256)
        protocol_path = ROOT / "protocol.json"
        protocol_bytes = protocol_path.read_bytes()
        self.assertEqual(hashlib.sha256(protocol_bytes).hexdigest(), PROTOCOL_SHA256)
        expected = json.loads(expected_bytes)
        actual = g30_reference.generate()
        self.assertEqual(actual, expected)
        self.assertEqual([case["count"] for case in actual["cases"]], [126, 48, 1044])
        self.assertEqual([case["greedy"] for case in actual["cases"]], [[0, 0, 5], [0, 1, 2], [1, 5]])

if __name__ == "__main__":
    unittest.main()
