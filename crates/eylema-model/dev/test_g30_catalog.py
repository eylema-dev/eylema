import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path

import g30_catalog

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/g30"

class G30CatalogFixtureTests(unittest.TestCase):
    def test_provenance_and_native_manifest(self):
        provenance = json.loads((FIXTURES / "provenance.json").read_bytes())
        wire = (FIXTURES / "cpu-profile.v1.eyl").read_bytes()
        self.assertEqual(hashlib.sha256(wire).hexdigest(), provenance["manifest_sha256"])
        self.assertEqual(len(wire), provenance["manifest_bytes"])
        self.assertEqual(provenance["source_weight_sha256_publisher_advertised"], "f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b")
        self.assertEqual(provenance["role_count"], 4)
        self.assertFalse(provenance["grants_execution_authority"])
        result = g30_catalog.validate()
        self.assertEqual(result["roles"], ["reasoning", "planning", "specialist", "critic"])

    def test_source_hash_role_parent_and_unknown_fields_reject(self):
        mutations = [
            ("catalog-profile.json", lambda value: value["manifest"].update(source_repository="https://example.invalid/model")),
            ("catalog-profile.json", lambda value: next(c for c in value["manifest"]["components"] if c["name"] == "model.safetensors").update(sha256="0" * 64)),
            ("role-contract.json", lambda value: value["profiles"][-1].update(output_units="wrong-units.v1")),
            ("role-contract.json", lambda value: value.update(parent_manifest_sha256="0" * 64)),
            ("catalog-profile.json", lambda value: value["manifest"].update(unexpected="authority")),
        ]
        for filename, mutate in mutations:
            with self.subTest(filename=filename, mutation=mutate):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    for name in ("catalog-profile.json", "role-contract.json", "provenance.json", "cpu-profile.v1.eyl"):
                        shutil.copyfile(FIXTURES / name, root / name)
                    changed = json.loads((root / filename).read_bytes())
                    mutate(changed)
                    raw = (json.dumps(changed, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode()
                    (root / filename).write_bytes(raw)
                    provenance = json.loads((root / "provenance.json").read_bytes())
                    if filename == "catalog-profile.json":
                        provenance["catalog_profile_sha256"] = hashlib.sha256(raw).hexdigest()
                    else:
                        provenance["role_contract_sha256"] = hashlib.sha256(raw).hexdigest()
                        catalog = json.loads((root / "catalog-profile.json").read_bytes())
                        component = next(c for c in catalog["manifest"]["components"] if c["name"] == "qwen3-cpu-roles.v1.json")
                        component.update(size=len(raw), sha256=hashlib.sha256(raw).hexdigest())
                        catalog_raw = (json.dumps(catalog, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode()
                        (root / "catalog-profile.json").write_bytes(catalog_raw)
                        provenance["catalog_profile_sha256"] = hashlib.sha256(catalog_raw).hexdigest()
                    (root / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
                    with self.assertRaises(ValueError):
                        g30_catalog.validate(root)

if __name__ == "__main__":
    unittest.main()
