#!/usr/bin/env python3
"""Install the pinned VeriFast release outside the repository.

The archive is downloaded once into a cache directory (default
~/.cache/jzap/verifast/<tag>) and checked against the sha256 recorded in
pin.json before every use. Each run unpacks it into a fresh directory, applies
the trusted std-spec overlay kept in this directory (std-spec-overlay.patch)
and checks that the rustup toolchain VeriFast is built against is installed.
VeriFast is a verification tool dependency only; nothing here touches the
product build.
"""
import argparse, hashlib, json, os, shutil, subprocess, sys, tarfile, time, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
PIN = json.loads((HERE / 'pin.json').read_text())


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def default_cache():
    base = os.environ.get('XDG_CACHE_HOME') or str(Path.home() / '.cache')
    return Path(base) / 'jzap' / 'verifast' / PIN['tag']


def download(url, dest, attempts=4):
    tmp = dest.with_suffix(dest.suffix + '.part')
    for attempt in range(1, attempts + 1):
        try:
            with urllib.request.urlopen(url, timeout=120) as response, open(tmp, 'wb') as out:
                shutil.copyfileobj(response, out, 1 << 20)
            tmp.replace(dest)
            return
        except OSError as error:
            tmp.unlink(missing_ok=True)
            if attempt == attempts:
                raise
            wait = 2 ** attempt
            print(f'download failed ({error}); retry {attempt}/{attempts - 1} in {wait}s', file=sys.stderr)
            time.sleep(wait)


def fetch(cache, offline):
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / PIN['archive']
    if not archive.exists():
        if offline:
            raise SystemExit(f'{archive} is not cached and --offline was given')
        print(f'downloading {PIN["url"]}')
        download(PIN['url'], archive)
    actual = sha256(archive)
    if actual != PIN['sha256']:
        raise SystemExit(f'sha256 mismatch for {archive}: {actual} != {PIN["sha256"]} (pinned)')
    return archive


def parse_patch(text):
    """Parse a unified diff into {path: [(old_start, old_lines, new_lines)]}."""
    files, current, lines, i = {}, None, text.splitlines(keepends=True), 0
    while i < len(lines):
        line = lines[i]
        if line.startswith('--- '):
            target = lines[i + 1]
            if not target.startswith('+++ b/'):
                raise ValueError('malformed patch header: ' + target)
            current = target[6:].strip()
            files[current] = []
            i += 2
            continue
        if line.startswith('@@ '):
            old_start = int(line.split()[1][1:].split(',')[0])
            old, new = [], []
            i += 1
            while i < len(lines) and lines[i][:1] in (' ', '-', '+', '\n') and not lines[i].startswith('--- '):
                body = lines[i]
                tag, rest = (body[0], body[1:]) if body != '\n' else (' ', '\n')
                if tag in (' ', '-'):
                    old.append(rest)
                if tag in (' ', '+'):
                    new.append(rest)
                i += 1
            files[current].append((old_start, old, new))
            continue
        i += 1
    return files


def apply_patch(root, patch_text):
    """Apply with exact context and position; any mismatch is an error."""
    for rel, hunks in parse_patch(patch_text).items():
        path = root / rel
        expected = PIN['overlay_base_sha256'].get(rel)
        if expected is None or sha256(path) != expected:
            raise SystemExit(f'overlay base {rel} does not match the pinned release file')
        content = path.read_text().splitlines(keepends=True)
        offset = 0
        for old_start, old, new in hunks:
            start = old_start - 1 + offset
            if content[start:start + len(old)] != old:
                raise SystemExit(f'overlay hunk at {rel}:{old_start} does not apply exactly')
            content[start:start + len(old)] = new
            offset += len(new) - len(old)
        path.write_text(''.join(content))


def check_toolchain(install):
    toolchain = PIN['toolchain']
    def installed():
        result = subprocess.run(['rustup', 'component', 'list', '--toolchain', toolchain, '--installed'],
                                capture_output=True, text=True)
        return result.returncode == 0, result.stdout.split()
    ok, components = installed()
    missing = [c for c in PIN['toolchain_components'] if not any(x.startswith(c + '-') for x in components)]
    if ok and not missing:
        return
    if not install:
        raise SystemExit(f'rustup toolchain {toolchain} with {", ".join(PIN["toolchain_components"])} is required '
                         f'(missing: {"toolchain" if not ok else ", ".join(missing)}); rerun with --install-toolchain')
    subprocess.run(['rustup', 'toolchain', 'install', toolchain, '--profile', 'minimal',
                    '--component', 'rustc-dev,llvm-tools-preview'], check=True)
    ok, components = installed()
    missing = [c for c in PIN['toolchain_components'] if not any(x.startswith(c + '-') for x in components)]
    if not ok or missing:
        raise SystemExit(f'toolchain {toolchain} still lacks {missing}')


def setup(cache=None, offline=False, install_toolchain=False):
    """Return the bin directory of a freshly unpacked, overlaid VeriFast."""
    cache = (Path(cache) if cache else default_cache()).expanduser().resolve()
    if cache.is_relative_to(HERE.parents[1]):
        raise SystemExit(f'{cache} is inside the repository; keep the tool cache outside it')
    archive = fetch(cache, offline)
    check_toolchain(install_toolchain)
    fresh = cache / 'install'
    if fresh.exists():
        shutil.rmtree(fresh)
    fresh.mkdir()
    with tarfile.open(archive) as tar:
        tar.extractall(fresh, filter='data')
    root = fresh / PIN['top_directory']
    apply_patch(root, (HERE / PIN['overlay']).read_text())
    version = (root / 'bin' / 'VERSION').read_text().strip()
    if not version.startswith(PIN['tag']):
        raise SystemExit(f'unpacked VERSION {version!r} does not match tag {PIN["tag"]}')
    (fresh / 'setup.json').write_text(json.dumps(dict(pin=PIN, version=version, archive=str(archive),
        overlay_sha256=sha256(HERE / PIN['overlay'])), indent=2) + '\n')
    return root / 'bin'


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--cache', type=Path, help='cache directory outside the repository')
    p.add_argument('--offline', action='store_true', help='use only a cached archive')
    p.add_argument('--install-toolchain', action='store_true', help=f'install rustup {PIN["toolchain"]} if missing')
    a = p.parse_args()
    print(setup(a.cache, a.offline, a.install_toolchain))


if __name__ == '__main__':
    main()
