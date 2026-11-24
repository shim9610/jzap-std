"""Transfer fresh certificate logs and receipts between Actions jobs."""

import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path


def identity(root, fingerprint):
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    assert commit == os.environ['GITHUB_SHA'], 'checkout does not match the Actions commit'
    return dict(
        commit=commit,
        repository=os.environ['GITHUB_REPOSITORY'],
        run_id=os.environ['GITHUB_RUN_ID'],
        run_attempt=os.environ['GITHUB_RUN_ATTEMPT'],
        source_sha256=fingerprint,
    )


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def publish(bundle, stage, context, steps, checkout, output):
    bundle.mkdir(parents=True)
    records = []
    for step in steps:
        name = step['name']
        sources = {output / (name + '.log'): step['log_sha256']}
        sources.update({Path(path): expected for path, expected in step['receipts'].items()})
        files = {}
        for source, expected in sources.items():
            if source.is_relative_to(output):
                key = 'output/' + source.relative_to(output).as_posix()
            else:
                key = 'checkout/' + source.relative_to(checkout).as_posix()
            target = bundle / key
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            assert digest(target) == expected, (name, 'evidence changed while exporting', key)
            files[key] = expected
        command = [
            '{output}' + part[len(str(output)):] if part.startswith(str(output) + '/') else part
            for part in step['command']
        ]
        record = {key: step[key] for key in ('name', 'environment', 'exit_code', 'started_ns', 'finished_ns')}
        records.append(dict(record, command=command, files=files))
    (bundle / 'manifest.json').write_text(json.dumps(
        dict(stage=stage, context=context, checkout=str(checkout), steps=records), indent=2) + '\n')


def collect(directory, context, stages, receipts, commands, profiles):
    expected_bundles = {'certificate-' + name for name in stages}
    assert {p.name for p in directory.iterdir()} == expected_bundles, 'missing or unexpected stage artifacts'
    steps = {}
    files = {}
    for stage, (_, expected_steps) in stages.items():
        bundle = directory / ('certificate-' + stage)
        paths = list(bundle.rglob('*'))
        assert not bundle.is_symlink() and not any(p.is_symlink() for p in paths), (stage, 'symlink in artifact')
        manifest = json.loads((bundle / 'manifest.json').read_text())
        assert manifest['stage'] == stage and manifest['context'] == context, (stage, 'stale or mismatched evidence')
        checkout = Path(manifest['checkout'])
        assert checkout.is_absolute() and '..' not in checkout.parts, (stage, 'invalid producer checkout')
        assert tuple(row['name'] for row in manifest['steps']) == expected_steps, (stage, 'incomplete steps')
        expected_paths = {bundle / 'manifest.json'}
        for row in manifest['steps']:
            name = row['name']
            command, environment = commands[name]
            assert row['command'] == command and row['environment'] == environment, (name, 'unexpected command')
            assert 0 < row['started_ns'] <= row['finished_ns'], (name, 'invalid step times')
            assert type(row['exit_code']) is int and row['exit_code'] in ((0, 1) if name == 'link' else (0,)), (
                name, 'failed step')
            required = {'output/' + name + '.log'} | {'checkout/' + path for path in receipts.get(name, ())}
            optional = set()
            if name == 'link':
                required.add('output/link/summary.json')
                optional = {f'output/link/{profile}/product/retained-panic-calls.json' for profile in profiles}
            if name in ('bplus-release', 'bplus-strided-release', 'bplus-partial-release'):
                required.add(f'output/{name}/summary.json')
            actual = set(row['files'])
            assert required <= actual <= required | optional and actual.isdisjoint(files), (
                name, 'missing, unexpected or duplicate evidence files')
            for key, expected in row['files'].items():
                path = bundle / key
                assert digest(path) == expected, (name, 'evidence digest differs', key)
                files[key] = path
                expected_paths.add(path)
            assert name not in steps, (name, 'duplicate step')
            steps[name] = dict(row, checkout=str(checkout))
        assert {path for path in paths if path.is_file()} == expected_paths, (stage, 'unlisted artifact files')
    return steps, files
