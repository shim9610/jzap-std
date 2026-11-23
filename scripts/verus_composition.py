"""Compose pending caller proofs with the original product crate, unchanged.

This helper supplies paths and compiler flags. It never extracts or rewrites a
Rust body. The supplied caller file can still contain a legacy projection; that
remaining boundary must be reported separately from the direct state methods.
"""
import os
from pathlib import Path
import subprocess

import verus_std_specs


def run(binary, source_root, proof, crate_name, env=None):
    root = Path(source_root).resolve()
    command = [str(binary), str(root / 'src/lib.rs'), '--crate-type', 'lib',
               '--crate-name', crate_name, '--edition=2024', '--cfg', 'feature="alloc"',
               '--cfg', 'jzap_proof_composition', '--no-cheating']
    # The product crate imports the checked core specifications vstd lacks.
    command += verus_std_specs.build(binary)
    environment = dict(os.environ if env is None else env,
                       JZAP_PROOF_COMPOSITION=str(Path(proof).resolve()))
    result = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
    return command, result
