"""Build source-inventory executables before read-only scanner use."""
from functools import cache
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


@cache
def _build():
    target = ROOT / 'target/panic-inventory'
    target.mkdir(parents=True, exist_ok=True)
    with (target / 'scanner-build.log').open('w') as log:
        subprocess.run(['cargo', 'build', '--locked', '--offline', '--manifest-path',
                        str(ROOT / 'tools/panic-audit/Cargo.toml'), '--bins',
                        '--target-dir', str(target)], cwd=ROOT, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    return target / 'debug'


def scanner(name):
    if name not in {'unsafe_sites', 'state_sites', 'memory'}:
        raise ValueError('Unknown source scanner: ' + name)
    return _build() / name
