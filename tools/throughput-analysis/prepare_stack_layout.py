#!/usr/bin/env python3
"""Seal one benchmark binary behind deterministic child-only stack layout shifts."""
import argparse, hashlib, json, platform, shutil, subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--shifts', default='0,16')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / 'target'):
        parser.error('Generated launchers belong under ignored target/')
    if output.exists() and any(output.iterdir()):
        parser.error('Use a fresh output directory')
    shifts = [int(x) for x in args.shifts.split(',')]
    if len(set(shifts)) != len(shifts) or not all(0 <= x <= 4096 and x % 16 == 0 for x in shifts):
        parser.error('Shifts must be distinct multiples of 16 between 0 and 4096')
    setarch = shutil.which('setarch')
    if setarch is None:
        parser.error('setarch is required for this Linux layout control')
    architecture = platform.machine()
    subprocess.run([setarch, architecture, '-R', 'true'], check=True)
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    output.mkdir(parents=True, exist_ok=True)
    variants = {}
    for shift in shifts:
        launcher = output / ('stack_' + str(shift))
        launcher.write_text(f'''#!/usr/bin/env python3
import hashlib, os, sys
from pathlib import Path
binary = {str(binary)!r}
if hashlib.sha256(Path(binary).read_bytes()).hexdigest() != {digest!r}:
    raise SystemExit('Sealed benchmark changed; prepare new launchers')
environment = dict(os.environ)
environment['JZAP_BENCH_STACK_SHIFT'] = 'x' * {shift}
os.execve({setarch!r}, [{setarch!r}, {architecture!r}, '-R', binary, *sys.argv[1:]], environment)
''')
        launcher.chmod(0o755)
        variants['stack_' + str(shift)] = str(launcher)
    metadata = dict(binary=str(binary), binary_sha256=digest, aslr=False,
                    shifts=shifts, variants=variants,
                    control='Same binary; only unused environment string length changes. No product or instruction padding.')
    (output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(json.dumps(metadata, indent=2))

if __name__ == '__main__':
    main()
