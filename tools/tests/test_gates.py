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


def body():
    return '\n\n'.join(f'## {s}\n\nSpecific command, environment and observed result.' for s in SECTIONS)


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


class PRTests(unittest.TestCase):
    def test_populated_body_passes(self):
        self.assertEqual(validate_body(body()), [])

    def test_blank_template_fails_actual_gate(self):
        text = (ROOT / '.github/PULL_REQUEST_TEMPLATE.md').read_text()
        self.assertEqual(validate_body(text, allow_template=True), [])
        self.assertEqual(len(validate_body(text)), len(SECTIONS))

    def test_missing_duplicate_and_placeholder_sections_fail(self):
        for text in [body().replace('## Need', '## Other'),
                     body() + '\n## Need\nDuplicate need details.',
                     body().replace('Specific command, environment and observed result.', 'TODO: write actual evidence later')]:
            with self.subTest(body=text): self.assertTrue(validate_body(text))
        self.assertTrue(validate_body(None))

    def test_comments_and_fenced_headings_do_not_count(self):
        for text in ['<!--' + body() + '-->', '```md\n' + body() + '\n```', '~~~md\n' + body() + '\n~~~']:
            with self.subTest(body=text): self.assertEqual(len(validate_body(text)), len(SECTIONS))

    def test_shell_payload_is_only_data(self):
        self.assertEqual(validate_body(body() + '\nLiteral example: $(exit 99); `touch /tmp/do-not-run`'), [])

    def test_bilingual_heading_suffixes_do_not_replace_english_sections(self):
        old_style = '\n\n'.join(f'## {section} / Translated heading\n\nSpecific evidence for the task.' for section in SECTIONS)
        self.assertEqual(len(validate_body(old_style)), len(SECTIONS))


class RepositoryTests(unittest.TestCase):
    def test_portable_and_nonportable_links(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'My File.md').write_text('# Target\n')
            self.assertEqual(local_link_errors(root, 'README.md', '[ok](My%20File.md) [web](https://example.org)\n```md\n[example](absent.md)\n```\n'), [])
            for value in ['missing.md', '../outside.md', '/tmp/file.md', 'file:///tmp/file.md', '%2e%2e/outside.md']:
                with self.subTest(target=value):
                    self.assertTrue(local_link_errors(root, 'README.md', f'[bad]({value})'))

    def test_duplicate_mapping_keys_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            for name, content, loader in [('bad.json', '{"a":1,"a":2}', load_json), ('bad.yml', 'a: 1\na: 2\n', load_yaml)]:
                p = Path(temp) / name
                p.write_text(content)
                with self.assertRaises(CheckFailure): loader(p)

    def test_actions_yaml_on_key_and_boolean(self):
        workflow = load_yaml(ROOT / '.github/workflows/ci.yml')
        self.assertIn('on', workflow)
        self.assertIs(workflow['jobs']['repository-checks']['steps'][0]['with']['persist-credentials'], False)
        validate_workflow(workflow)

    def test_privileged_workflow_variants_fail(self):
        original = load_yaml(ROOT / '.github/workflows/ci.yml')
        variants = []
        x = copy.deepcopy(original); x['on']['pull_request_target'] = {}; variants.append(x)
        x = copy.deepcopy(original); x['permissions']['contents'] = 'write'; variants.append(x)
        x = copy.deepcopy(original); x['jobs']['repository-checks']['steps'][0]['uses'] = 'actions/checkout@main'; variants.append(x)
        x = copy.deepcopy(original); x['jobs']['repository-checks']['steps'][0]['with']['persist-credentials'] = True; variants.append(x)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(CheckFailure): validate_workflow(value)

    def test_issue_acceptance_and_unique_ids(self):
        original = load_yaml(ROOT / '.github/ISSUE_TEMPLATE/feature.yml')
        validate_form(original)
        missing = copy.deepcopy(original)
        missing['body'] = [x for x in missing['body'] if x.get('id') != 'acceptance']
        with self.assertRaises(CheckFailure): validate_form(missing)
        original['body'].append(original['body'][1])
        with self.assertRaises(CheckFailure): validate_form(original)

    def test_translation_edit_invalidates_fingerprint(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'en.md').write_text('English')
            (root / 'tr.md').write_text('Translated content')
            manifest = {'pairs': [{'en': 'en.md', 'tr': 'tr.md'}]}
            verify_translations(root, manifest, write=True)
            self.assertEqual(verify_translations(root, manifest), 1)
            (root / 'en.md').write_text('Changed')
            with self.assertRaises(CheckFailure): verify_translations(root, manifest)

    def test_api_method_drift_fails(self):
        en = (ROOT / 'docs/en/api-reference.md').read_text()
        tr = (ROOT / 'docs/tr/api-reference.md').read_text()
        self.assertEqual(validate_api(en, tr), 20)
        with self.assertRaises(CheckFailure): validate_api(en, tr.replace('`GET ', '`POST ', 1))


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
