#!/usr/bin/env python3
"""Install the pinned Verus Linux/x86-64 bundle and its compiler, without runtime dependencies."""
import argparse
import hashlib
import os
from pathlib import Path
import platform
import stat
import subprocess
import urllib.request
import zipfile

VERSION = '0.2026.11.02.8bf68ee'
SHA256 = 'f3a0119292df8855e4435da8130bee1873b0e6a60c306accf81a08ea687bba3c'
RUST = '1.99.1'
NAME = f'verus-{VERSION}-x86-linux.zip'
URL = f'https://github.com/verus-lang/verus/releases/download/release/{VERSION}/{NAME}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=Path('/workspace/.devtools/verus')
                        if Path('/workspace/.devtools').is_dir() else Path.home() / '.cache/jzap/verus')
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        parser.error('This pinned bundle supports Linux x86-64; install the matching official platform bundle separately')
    destination = args.directory.expanduser().resolve()
    destination.mkdir(parents=True, exist_ok=True)
    archive_path = destination / NAME
    if not archive_path.is_file() or hashlib.sha256(archive_path.read_bytes()).hexdigest() != SHA256:
        temporary = archive_path.with_suffix('.download')
        # urllib retains the environment proxy and certificate configuration.
        urllib.request.urlretrieve(URL, temporary)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != SHA256:
            raise RuntimeError('Verus release SHA-256 mismatch')
        temporary.replace(archive_path)
    with zipfile.ZipFile(archive_path) as archive:
        for entry in archive.infolist():
            if not (destination / entry.filename).resolve().is_relative_to(destination):
                raise RuntimeError('Archive path escapes installation directory')
            if stat.S_ISLNK(entry.external_attr >> 16):
                raise RuntimeError('Unexpected symbolic link in pinned bundle')
        archive.extractall(destination)
        for entry in archive.infolist():
            if not entry.is_dir():
                os.chmod(destination / entry.filename, (entry.external_attr >> 16) & 0o777)
    subprocess.run(['rustup', 'toolchain', 'install', RUST, '--profile', 'minimal'], check=True)
    executable = destination / 'verus-x86-linux/verus'
    subprocess.run([str(executable), '--version'], check=True)
    print('SHA256:', SHA256)
    print('JZAP_VERUS=' + str(executable))


if __name__ == '__main__':
    main()
