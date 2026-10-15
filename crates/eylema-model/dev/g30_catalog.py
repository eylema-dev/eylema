"""Offline audit of the narrow public CPU profile fixtures; grants no authority."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/g30'
MAX_JSON_BYTES = 65536

def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result

def _load(root, name):
    path = root / name
    if path.is_symlink():
        raise ValueError('symlink fixture')
    raw = path.read_bytes()
    if len(raw) > MAX_JSON_BYTES:
        raise ValueError('fixture size')
    value = json.loads(raw, object_pairs_hook=_pairs)
    if name != 'provenance.json' and raw != (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode():
        raise ValueError('non-canonical JSON')
    return raw, value

def validate(fixture_root=FIXTURES):
    prov_raw, provenance = _load(fixture_root, 'provenance.json')
    del prov_raw
    catalog_raw, catalog = _load(fixture_root, 'catalog-profile.json')
    roles_raw, roles = _load(fixture_root, 'role-contract.json')
    manifest = catalog['manifest']
    expected_manifest_keys = {'format','id','revision','source_repository','source_commit','architecture','artifact_format','numeric_format','quantization','components','operators','rights','configuration','profiles'}
    if set(catalog) != {'evidence','manifest'} or set(manifest) != expected_manifest_keys:
        raise ValueError('closed catalog manifest')
    if hashlib.sha256(catalog_raw).hexdigest() != provenance['catalog_profile_sha256']:
        raise ValueError('catalog fixture digest')
    if hashlib.sha256(roles_raw).hexdigest() != provenance['role_contract_sha256']:
        raise ValueError('role fixture digest')
    wire = (fixture_root / 'cpu-profile.v1.eyl').read_bytes()
    if hashlib.sha256(wire).hexdigest() != provenance['manifest_sha256'] or len(wire) != provenance['manifest_bytes']:
        raise ValueError('native manifest fixture digest')
    if manifest['format'] != 'model-profile.v1' or manifest['id'] != 'qwen3-0.6b.cpu' or manifest['revision'] != 2:
        raise ValueError('CPU profile identity')
    if manifest['source_repository'] != 'https://huggingface.co/Qwen/Qwen3-0.6B' or manifest['source_commit'] != 'c1899de289a04d12100db370d81485cdf75e47ca':
        raise ValueError('immutable source identity')
    components = {entry['name']: entry for entry in manifest['components']}
    if len(components) != len(manifest['components']):
        raise ValueError('duplicate component')
    weights = components['model.safetensors']
    if (weights['size'], weights['sha256'], weights['digest_source']) != (1503300328, provenance['source_weight_sha256_publisher_advertised'], 'publisher_advertised'):
        raise ValueError('publisher-only weight identity')
    if components['tokenizer.json']['sha256'] != provenance['tokenizer_json_sha256_publisher_advertised']:
        raise ValueError('tokenizer parent drift')
    if components['tokenizer_config.json']['sha256'] != provenance['tokenizer_config_sha256_measured']:
        raise ValueError('tokenizer config drift')
    if manifest['configuration']['template_sha256'] != provenance['template_sha256_measured']:
        raise ValueError('template drift')
    role_component = components['qwen3-cpu-roles.v1.json']
    if (role_component['size'], role_component['sha256']) != (len(roles_raw), hashlib.sha256(roles_raw).hexdigest()):
        raise ValueError('role component identity')
    if manifest['configuration']['preprocessing_component'] != 'qwen3-cpu-roles.v1.json':
        raise ValueError('role component binding')
    if roles['format'] != 'qwen3-cpu-roles.v1' or roles['revision'] != 1 or roles['authority'] != 'role intent only; no execution authority, model quality claim, or permissions':
        raise ValueError('role contract identity')
    if roles['parent_manifest_sha256'] != provenance['source_model_manifest_wire_sha256']:
        raise ValueError('parent G27 manifest binding')
    profiles = roles['profiles']
    if [p['role'] for p in profiles] != ['reasoning','planning','specialist','critic'] or manifest['profiles'] != profiles:
        raise ValueError('four role profile set/order')
    critic = profiles[-1]
    if (critic['input_schema'],critic['output_schema'],critic['input_units'],critic['output_units'],critic['preprocessing']) != ('candidate.evidence.v1','evaluation.proposal.v1','declared-units.v1','declared-units.v1','qwen3.critic-evidence.v1'):
        raise ValueError('critic contract')
    if roles['role_limits'] != {'critic_candidate_utf8_bytes':1024,'critic_evidence_utf8_bytes':1024,'maximum_context_positions':64,'maximum_new_tokens':8,'proposal_only':True,'reset_between_roles':True,'unsupported_role':'reject'}:
        raise ValueError('bounded role limits')
    if any(('permission' in key or key == 'execution') for key in manifest):
        raise ValueError('catalog must not add authority fields')
    if catalog['evidence'] != {'authority':'none; model execution remains not_run until separately demonstrated','cpu_acceptance':'not_run','parent_manifest_sha256':roles['parent_manifest_sha256'],'weights_downloaded':False}:
        raise ValueError('catalog evidence overclaim')
    if provenance['grants_execution_authority'] is not False or provenance['weights_downloaded'] is not False or provenance['model_execution'] != 'not_run':
        raise ValueError('provenance overclaim')
    return {'manifest_sha256': hashlib.sha256(wire).hexdigest(), 'roles': [p['role'] for p in profiles]}

if __name__ == '__main__':
    print(json.dumps(validate(), sort_keys=True))
