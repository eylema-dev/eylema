"""macOS contract structure only; every candidate/hash/budget is synthetic."""
import copy
import sys
import unittest
from pathlib import Path
from jsonschema import Draft202012Validator, ValidationError
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_repository import ROOT, load_json, validate_schema


class MacOSSchemaTests(unittest.TestCase):
    def setUp(self):
        self.schema = load_json(ROOT / 'ai-team-protocol/v0.2/model-set-release.schema.json')
        validate_schema(self.schema)
        self.validator = Draft202012Validator(self.schema)
        self.example = load_json(ROOT / 'ai-team-protocol/v0.2/model-set-release.example.json')

    def test_one_physical_device_multiple_workloads_candidate_passes(self):
        self.validator.validate(self.example)
        self.assertEqual(len(self.example['hardware_profiles']), 1)
        hardware = self.example['hardware_profiles'][0]['id']
        for item in self.example['workload_profiles'] + self.example['assignments']:
            self.assertEqual(item['hardware_profile_id'], hardware)
        self.assertEqual(self.example['status'], 'candidate')
        self.assertEqual(self.example['evidence'], [])

    def test_native_macos_and_browser_backend_pairs(self):
        profile = copy.deepcopy(self.example['workload_profiles'][0])
        pairs = [('native_macos','native_cpu','none'), ('native_macos','native_gpu','metal'),
                 ('browser','wasm_cpu','none'), ('browser','webgpu','webgpu')]
        for target, backend, gpu in pairs:
            value = copy.deepcopy(self.example)
            value['workload_profiles'][0].update(target=target, backend=backend, gpu_api=gpu)
            self.validator.validate(value)
        for target,backend,gpu in [('native_linux','native_cpu','none'), ('browser','native_gpu','metal'),
                                  ('native_macos','webgpu','webgpu'), ('native_macos','native_gpu','cuda'),
                                  ('native_macos','native_cpu','metal')]:
            value = copy.deepcopy(self.example)
            value['workload_profiles'][0].update(target=target,backend=backend,gpu_api=gpu)
            with self.subTest(target=target,backend=backend,gpu=gpu):
                with self.assertRaises(ValidationError): self.validator.validate(value)

    def test_versions_physical_count_and_workload_bindings_fail(self):
        variants = []
        x=copy.deepcopy(self.example); x['schema_version']='0.1.0'; variants.append(x)
        x=copy.deepcopy(self.example); x['runtime_protocol_version']='0.2.0'; variants.append(x)
        x=copy.deepcopy(self.example); x['hardware_profiles'].append(dict(x['hardware_profiles'][0],id='second-device')); variants.append(x)
        x=copy.deepcopy(self.example); x['hardware_profiles'][0]['os']='linux'; variants.append(x)
        x=copy.deepcopy(self.example); del x['workload_profiles']; variants.append(x)
        x=copy.deepcopy(self.example); x['workload_profiles']=x['workload_profiles'][:1]; variants.append(x)
        x=copy.deepcopy(self.example); del x['assignments'][0]['workload_profile_id']; variants.append(x)
        x=copy.deepcopy(self.example); x['workload_profiles'][0]['workload_sha256']='not-a-hash'; variants.append(x)
        x=copy.deepcopy(self.example); x['status']='active'; variants.append(x)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValidationError): self.validator.validate(value)

    def test_evidence_requires_explicit_workload_and_old_version_rejects_new(self):
        value=copy.deepcopy(self.example)
        value['evidence']=[{'check_id':'synthetic','status':'not_run','report_ref':'synthetic-reason',
                           'hardware_profile_id':value['hardware_profiles'][0]['id'],
                           'workload_profile_id':value['workload_profiles'][0]['id'],'model_ids':[]}]
        self.validator.validate(value)
        del value['evidence'][0]['workload_profile_id']
        with self.assertRaises(ValidationError): self.validator.validate(value)
        old=Draft202012Validator(load_json(ROOT / 'ai-team-protocol/model-set-release.schema.json'))
        with self.assertRaises(ValidationError): old.validate(self.example)
