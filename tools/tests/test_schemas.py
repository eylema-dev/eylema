"""Failure-oriented checks; all model/release data below is synthetic."""
import copy
import sys
import tempfile
import unittest
from pathlib import Path
from jsonschema import Draft202012Validator, ValidationError
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_pr import SECTIONS, validate_body
from check_repository import (ROOT, CheckFailure, load_json, load_yaml, local_link_errors,
                              validate_api, validate_form, validate_schema, validate_workflow,
                              verify_translations)


def candidate():
    return {
        'schema_version': '0.1.0', 'runtime_protocol_version': '0.1.0',
        'release_id': 'synthetic-candidate', 'status': 'candidate',
        'behavior_contract_ref': 'synthetic-v1', 'behavior_contract_sha256': '0' * 64,
        'engine': {'id': 'synthetic', 'version': 'test-only', 'build_sha256': '0' * 64},
        'models': [{'id': 'synthetic-policy', 'kind': 'policy', 'source_ref': 'synthetic-source-v1',
                    'artifact_ref': 'synthetic-artifact-v1', 'artifact_sha256': '0' * 64,
                    'architecture': 'test-only', 'format': 'test-only', 'quantization': 'none',
                    'required_operators': ['synthetic-op']}],
        'hardware_profiles': [{'id': 'synthetic-cpu', 'target': 'native_linux',
                               'backend': 'native_cpu', 'capabilities': []}],
        'assignments': [{'role': 'synthetic', 'hardware_profile_id': 'synthetic-cpu',
                         'model_ids': ['synthetic-policy'], 'max_concurrency': 1, 'fallback': 'fail',
                         'budget': {'max_latency_ms': 1, 'max_resident_bytes': 1},
                         'contract_ref': 'synthetic-v1'}],
        'evidence': [], 'objectives': ['quality'],
    }


class SchemaTests(unittest.TestCase):
    def test_remote_reference_fails_without_network(self):
        with self.assertRaises(CheckFailure): validate_schema({'$ref': 'https://example.org/schema.json'})

    def test_request_positive_and_negative_cases(self):
        schema = load_json(ROOT / 'ai-team-protocol/request.schema.json')
        validate_schema(schema)
        validator = Draft202012Validator(schema)
        example = load_json(ROOT / 'ai-team-protocol/request.example.json')
        validator.validate(example)
        variants = []
        x = copy.deepcopy(example); del x['execution']; variants.append(x)
        x = copy.deepcopy(example); x['execution']['budget']['max_model_calls'] = 0; variants.append(x)
        x = copy.deepcopy(example); x['execution']['publish_target'] = 'merge'; variants.append(x)
        x = copy.deepcopy(example); x['constraints']['dependency_policy'] = 'anything'; variants.append(x)
        x = copy.deepcopy(example); x['acceptance_criteria'] = []; variants.append(x)
        x = copy.deepcopy(example); x['unexpected'] = True; variants.append(x)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValidationError): validator.validate(value)

    def test_release_positive_and_negative_cases(self):
        schema = load_json(ROOT / 'ai-team-protocol/model-set-release.schema.json')
        validate_schema(schema)
        validator = Draft202012Validator(schema)
        fixture = candidate()
        validator.validate(fixture)
        variants = []
        x = copy.deepcopy(fixture); x['status'] = 'active'; variants.append(x)
        x = copy.deepcopy(fixture); x['models'][0]['artifact_sha256'] = 'invalid'; variants.append(x)
        x = copy.deepcopy(fixture); x['assignments'][0]['max_concurrency'] = 0; variants.append(x)
        x = copy.deepcopy(fixture); x['models'][0]['kind'] = 'universal'; variants.append(x)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValidationError): validator.validate(value)


if __name__ == '__main__':
    unittest.main()
