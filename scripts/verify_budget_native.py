#!/usr/bin/env python3
"""Verify actual local-budget Rust transitions, including an unmodeled-branch control.

The runtime numeric functions have direct Verus dual specifications. Caller's
charge ownership, allocator and unsafe payload operations have separate checks.
"""
import argparse, fcntl, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HARNESSES = [
    'proofs::budget_arithmetic_and_raii',
    'budget::panic_proofs::reservation_and_commit_preserve_every_valid_state',
    'budget::panic_proofs::release_preserves_every_valid_owned_charge_partition',
    'budget::panic_proofs::every_other_owner_transition_preserves_existing_entitlements',
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mutations', action='store_true')
    args = parser.parse_args()
    out = ROOT / 'target/verification/budget-native'
    out.mkdir(parents=True, exist_ok=True)
    lock = (out / '.lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    (out / 'summary.json').unlink(missing_ok=True)
    files = sorted((ROOT / 'src').rglob('*.rs')) + [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__)]
    digest = lambda: hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode()+b'\0'+p.read_bytes() for p in files)).hexdigest()
    before = digest()
    env = dict(os.environ, RUSTFLAGS='--cfg jzap_loop_contracts')
    env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    command = ['cargo', 'kani', '--lib', '-Z', 'loop-contracts', '--exact']
    for harness in HARNESSES:
        command += ['--harness', harness]
    with (out / 'native.log').open('w') as log:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=log)
    text = (out / 'native.log').read_text()
    assert result.returncode == 0 and 'Complete - 4 successfully verified harnesses, 0 failures, 4 total.' in text, 'actual budget verification failed; see native.log'
    controls = []
    if args.mutations:
        with tempfile.TemporaryDirectory(prefix='jzap-budget-native-') as temporary:
            work = Path(temporary)
            for folder in ('src', 'examples'):
                shutil.copytree(ROOT / folder, work / folder)
            for name in ('Cargo.toml', 'Cargo.lock'):
                shutil.copy(ROOT / name, work / name)
            path = work / 'src/budget.rs'
            source = path.read_text()
            anchor = '        Ok(Reservation {\n'
            assert source.count(anchor) == 1, 'budget control anchor requires review'
            injected = '        if bytes == 1 { panic!("unmodeled admission side effect"); }\n'
            source = source.replace(anchor, injected + anchor)
            path.write_text(source)
            line = source[:source.index(injected)].count('\n') + 1
            mutant_env = dict(env, CARGO_TARGET_DIR=str(out / 'mutant-target'))
            mutation = ['cargo', 'kani', '--manifest-path', str(work / 'Cargo.toml'), '--lib', '-Z', 'loop-contracts', '--exact', '--harness', HARNESSES[1]]
            with (out / 'mutation.log').open('w') as log:
                result = subprocess.run(mutation, cwd=ROOT, env=mutant_env, stdout=log, stderr=log)
            text = (out / 'mutation.log').read_text()
            assert result.returncode != 0 and 'Complete - 0 successfully verified harnesses, 1 failures, 1 total.' in text, 'mutation did not reach the expected solver failure'
            assert re.search(r'Check \d+: budget::MemoryBudget::reserve.assertion.\d+\s+- Status: FAILURE', text)
            assert re.search(r'Location: src/budget\.rs:'+str(line)+r':\d+ in function budget::MemoryBudget::reserve', text), 'mutation failed at a different location'
            controls.append(dict(name='unmodeled-reserve-panic', detected=True, location=f'src/budget.rs:{line}'))
    assert digest() == before, 'budget verification inputs changed'
    report = dict(status='pass', source_sha256=before, source_unchanged=True,
                  kani_harnesses=HARNESSES, controls=controls, mutations_run=args.mutations,
                  scope='Actual Rust transitions over valid local-budget word states; charge ownership is a release precondition, with caller conservation checked separately.')
    (out / 'summary.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Four actual Rust budget harnesses passed; unmodeled-branch control='+str(bool(controls)), flush=True)


if __name__ == '__main__':
    main()
