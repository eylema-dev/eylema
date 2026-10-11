"""Offline documentation-stage gates. No application, GPU or model is executed."""
import argparse
import copy
import hashlib
import json
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

import yaml
from jsonschema import Draft202012Validator
from check_pr import validate_body

ROOT = Path(__file__).resolve().parents[1]
ACTIONS = {
    "actions/checkout": "3d3c42e5aac5ba805825da76410c181273ba90b1",
    "actions/setup-python": "5fda3b95a4ea91299a34e894583c3862153e4b97",
}


class CheckFailure(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"Duplicate mapping key: {key}")
        result[key] = value
    return result


def load_json(path):
    return json.loads(path.read_text(), object_pairs_hook=unique_pairs)


class WorkflowLoader(yaml.SafeLoader):
    """Use true/false booleans; YAML 1.1 must not turn the Actions 'on' key into True."""
    yaml_implicit_resolvers = copy.deepcopy(yaml.SafeLoader.yaml_implicit_resolvers)


for first, resolvers in WorkflowLoader.yaml_implicit_resolvers.items():
    WorkflowLoader.yaml_implicit_resolvers[first] = [r for r in resolvers if r[0] != "tag:yaml.org,2002:bool"]
WorkflowLoader.add_implicit_resolver("tag:yaml.org,2002:bool", re.compile(r"^(?:true|false)$", re.I), list("tTfF"))


def yaml_mapping(loader, node):
    return unique_pairs((loader.construct_object(k), loader.construct_object(v)) for k, v in node.value)


WorkflowLoader.add_constructor("tag:yaml.org,2002:map", yaml_mapping)


def load_yaml(path):
    return yaml.load(path.read_text(), Loader=WorkflowLoader)


def repository_files(root):
    result = subprocess.run(["git", "ls-files", "-c", "-o", "--exclude-standard", "-z"], cwd=root, check=True, capture_output=True)
    files = sorted(set(result.stdout.decode().split("\0")) - {""})
    for name in files:
        path = root / name
        require(not path.is_symlink(), f"Symlink is not permitted in the documentation pack: {name}")
        require(path.is_file(), f"Missing tracked file: {name}")
    return files


def local_link_errors(root, document, text):
    errors = []
    # Code examples are not navigable Markdown links.
    text = re.sub(r"^```.*?^```\s*$", "", text, flags=re.M | re.S)
    for raw in re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", text):
        target = raw.strip().strip("<>")
        parsed = urlsplit(target)
        if parsed.scheme in {"https", "http", "mailto"} or target.startswith("#"):
            continue
        if parsed.scheme or target.startswith(("/", "\\")):
            errors.append(f"Nonportable link in {document}: {target}")
            continue
        path = (root / document).parent / unquote(parsed.path)
        if not path.resolve().is_relative_to(root.resolve()) or not path.is_file():
            errors.append(f"Broken or escaping link in {document}: {target}")
    return errors


def walk(value):
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)


def validate_schema(schema):
    for value in walk(schema):
        if isinstance(value, dict) and "$ref" in value:
            require(value["$ref"].startswith("#"), "Schemas must resolve locally; remote references are forbidden")
    Draft202012Validator.check_schema(schema)


def validate_form(form):
    require(isinstance(form, dict) and all(isinstance(form.get(k), str) and form[k].strip() for k in ["name", "description", "title"]), "Issue form needs name, description and title")
    require(isinstance(form.get("body"), list), "Issue form body must be a list")
    fields = {}
    for item in form["body"]:
        require(isinstance(item, dict), "Issue form item must be a mapping")
        kind = item.get("type")
        require(kind in {"markdown", "textarea", "input", "dropdown", "checkboxes"}, "Unsupported issue field type")
        attrs = item.get("attributes", {})
        if kind == "markdown":
            require(bool(attrs.get("value")), "Empty issue-form markdown")
            continue
        key = item.get("id", "")
        require(bool(re.fullmatch(r"[a-z][a-z0-9_-]*", key)) and key not in fields, "Invalid or duplicate issue field ID")
        require(bool(attrs.get("label")), "Issue field has no label")
        fields[key] = item
        if kind in {"dropdown", "checkboxes"}:
            require(bool(attrs.get("options")), "Choice field has no options")
    for key in ["need", "targets", "acceptance", "scope", "verification"]:
        require(fields.get(key, {}).get("validations", {}).get("required") is True, f"Issue form must require {key}")


def validate_workflow(workflow):
    require(workflow.get("permissions") == {"contents": "read"}, "Workflow token must only have contents: read")
    events = workflow.get("on", {})
    require(set(events) == {"push", "pull_request", "workflow_dispatch"}, "Unexpected or privileged workflow trigger")
    require(events["push"].get("branches") == ["main"], "Push CI must cover main")
    required_events = {"opened", "synchronize", "reopened", "edited", "ready_for_review"}
    require(set(events["pull_request"].get("types", [])) == required_events, "PR body and head changes must retrigger CI")
    require(set(workflow.get("jobs", {})) == {"repository-checks"}, "Review new job boundaries before extending CI")
    job = workflow["jobs"]["repository-checks"]
    require(job.get("runs-on") == "ubuntu-24.04" and job.get("timeout-minutes") == 10, "CI needs bounded hosted runner")
    require(not any(k in job for k in ["permissions", "environment", "container", "services", "uses"]), "Unexpected CI trust boundary")
    used = set()
    commands = []
    for step in job.get("steps", []):
        if "uses" in step:
            name, sep, sha = step["uses"].partition("@")
            require(sep and ACTIONS.get(name) == sha, "Action must use a reviewed full commit SHA")
            used.add(name)
            if name == "actions/checkout":
                require(step.get("with", {}).get("persist-credentials") is False, "Checkout credentials must not persist")
        if "run" in step:
            require("${{" not in step["run"], "Pass expressions via environment, never interpolate into shell code")
            commands.append(step["run"])
        require("secrets." not in json.dumps(step), "Public PR CI must not consume secrets")
    require(used == set(ACTIONS), "Missing pinned provisioning actions")
    joined = "\n".join(commands)
    for command in ["--require-hashes", "unittest discover", "tools/check_repository.py", "tools/check_pr.py"]:
        require(command in joined, f"CI missing required command: {command}")
    pr_steps = [s for s in job["steps"] if "tools/check_pr.py" in s.get("run", "")]
    require(len(pr_steps) == 1 and pr_steps[0].get("if") == "github.event_name == 'pull_request'", "Real PR gate must run on PR events")
    require("--allow-template" not in joined, "CI must not treat actual PR bodies as blank templates")


def verify_translations(root, manifest, write=False):
    pairs = manifest.get("pairs", [])
    require(bool(pairs), "Missing translation pairs")
    seen = set()
    for pair in pairs:
        for language in ["en", "tr"]:
            name = pair[language]
            path = root / name
            require(name not in seen and path.resolve().is_relative_to(root.resolve()) and path.is_file(), f"Invalid translation path: {name}")
            seen.add(name)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            key = language + "_sha256"
            if write:
                pair[key] = digest
            else:
                require(pair.get(key) == digest, f"Translation review fingerprint is stale: {name}")
    return len(pairs)


def validate_api(en, tr):
    pattern = r"^\| `([^`]+)` \| `(GET|POST) ([^`]+)` \|"
    operations = re.findall(pattern, en, re.M)
    require(bool(operations) and operations == re.findall(pattern, tr, re.M), "EN/TR API operation mismatch")
    require(set(re.findall(r"`([^`\n]+)`", en)) == set(re.findall(r"`([^`\n]+)`", tr)), "EN/TR API inline identifier mismatch")
    require(re.findall(r"^source_revision: (.+)$", en, re.M) == re.findall(r"^source_revision: (.+)$", tr, re.M), "API source revision mismatch")
    return len(operations)


def check(root, event=None):
    names = repository_files(root)
    for name in names:
        path = root / name
        if path.suffix in {".md", ".json", ".yml", ".yaml"}:
            text = path.read_text()
            require(not re.search(r"/Users/[^/\s]+/|/home/[^/\s]+/|file://|codex://", text), f"Private/nonportable machine path: {name}")
        if path.suffix == ".json":
            load_json(path)
        if path.suffix in {".yml", ".yaml"}:
            load_yaml(path)
        if path.suffix == ".md":
            errors = local_link_errors(root, name, path.read_text())
            require(not errors, "; ".join(errors))
    require(not any(Path(n).suffix == ".rs" or Path(n).name == "Cargo.toml" for n in names), "Documentation-only gate cannot accept Rust source; implement EYL-003 Rust CI and replace this guard together")
    schemas = [n for n in names if n.endswith(".schema.json")]
    require(bool(schemas), "No schemas found")
    for name in schemas:
        validate_schema(load_json(root / name))
    for name in names:
        if name.endswith(".example.json"):
            schema = root / name.replace(".example.json", ".schema.json")
            require(schema.is_file(), f"Missing schema for {name}")
            Draft202012Validator(load_json(schema)).validate(load_json(root / name))
    manifest = load_json(root / "tools/translation-pairs.json")
    pair_count = verify_translations(root, manifest)
    api_count = validate_api((root / "docs/en/api-reference.md").read_text(), (root / "docs/tr/api-reference.md").read_text())
    ids = lambda name: re.findall(r"^\| (M\d+|EYL-\d+) \|", (root / name).read_text(), re.M)
    require(bool(ids("ROADMAP.md")) and ids("ROADMAP.md") == ids("ROADMAP.tr.md"), "Roadmap ID mismatch")
    forms = [n for n in names if n.startswith(".github/ISSUE_TEMPLATE/") and n.endswith(".yml") and Path(n).name != "config.yml"]
    require(len(forms) == 3, "Expected feature, bug and architecture issue forms")
    for name in forms:
        validate_form(load_yaml(root / name))
    require(not validate_body((root / ".github/PULL_REQUEST_TEMPLATE.md").read_text(), allow_template=True), "PR template section contract is incomplete")
    workflows = [n for n in names if n.startswith(".github/workflows/")]
    require(workflows == [".github/workflows/ci.yml"], "New workflows require an explicit trust-boundary review and checker update")
    validate_workflow(load_yaml(root / workflows[0]))
    base = None
    if event:
        payload = load_json(event)
        base = payload.get("pull_request", {}).get("base", {}).get("sha") or payload.get("before")
        if base:
            require(bool(re.fullmatch(r"[0-9a-f]{40}", base)), "Invalid event base SHA")
            if base == "0" * 40:
                base = None
    command = ["git", "diff", "--check", base, "HEAD"] if base else ["git", "show", "--format=", "--check", "HEAD"]
    subprocess.run(command, cwd=root, check=True)
    subprocess.run(["git", "diff", "--check"], cwd=root, check=True)
    print(f"PASS: {len(names)} files, {len(schemas)} schemas, {pair_count} translation pairs, {api_count} bilingual API operations, {len(forms)} issue forms")
    print("Scope: documentation and repository tooling only; Rust/GPU/model execution is NOT RUN")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", type=Path)
    parser.add_argument("--write-translation-hashes", action="store_true")
    args = parser.parse_args()
    try:
        if args.write_translation_hashes:
            path = ROOT / "tools/translation-pairs.json"
            manifest = load_json(path)
            verify_translations(ROOT, manifest, write=True)
            path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
            print("Updated translation fingerprints. This records reviewed bytes, not semantic proof.")
        else:
            check(ROOT, args.event)
    except (CheckFailure, ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"FAIL: {error}") from error


if __name__ == "__main__":
    main()
