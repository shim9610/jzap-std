#!/usr/bin/env python3
"""Auxiliary VeriFast check of the Capacity subset crate (local only).

Steps, all required (exit status 1 on any failure):
  setup     fresh unpack of the pinned VeriFast release with the trusted overlay
  assemble  original and verified subset crates under target/verification/verifast/
            from the current src/ and the stored annotated/rewritten files
  identity  every product file equals its pinned sha256; original-side files are
            the product bytes (or the pinned cut of them)
  strip     every annotations-only file equals its original side once VeriFast
            comments (/*@ ... @*/, //@ ...) are removed, compared token by token
  inventory every `fn` of the subset is classified in the manifest
  verifast  VeriFast on the verified crate
  refine    the refinement checker, original -> verified
  kani      the Kani spec harnesses of layout::array on an assembled product copy
  controls  negative controls; each must be rejected by the named check

This is supplementary evidence. It does not change any certificate disposition
(docs/verification/verifast.md, docs/premise-proofs.md).
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PROOF = HERE / 'proofs' / 'capacity'
OUT_DEFAULT = ROOT / 'target' / 'verification' / 'verifast'
# Exactly the flags of the evaluation spikes; -verbose 1 only adds the per-function log.
VERIFAST_FLAGS = ['-skip_specless_fns', '-allow_assume', '-allow_dead_code']
FLAG_EFFECTS = {
    '-skip_specless_fns': 'Functions without a req/ens specification are not verified at all (no safety check '
                          'against their generated spec); they are listed as skipped-specless.',
    '-allow_assume': 'Permits assume(..) in ghost code. Every assume must be inside a lemma on the trusted list; '
                     'check.py fails otherwise.',
    '-allow_dead_code': 'VeriFast does not report code it finds unreachable during symbolic execution as an error.',
    '-verbose 1': 'Output only: logs each verified function so check.py can report coverage.',
}
KANI_HARNESS = HERE / 'kani' / 'array_spec.rs'
KANI_POSITIVE = ['vf_spec_array_u8', 'vf_spec_array_u64', 'vf_spec_array_1600', 'vf_spec_array_align64']
KANI_NEGATIVE = 'vf_negative_array_size_is_count_u64'
GENERATED_LEMMA = re.compile(r'(_upcast|^is_Send_|^is_Sync_|^open_ref_init_perm_|^init_ref_padding_|'
                             r'^close_ref_initialized_|^open_ref_initialized_|^end_ref_padding_|_lemma$|_def$)')


class CheckError(Exception):
    pass


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


# ---------------------------------------------------------------- Rust lexing
def lex(text):
    """Split Rust source into (kind, text) tokens following rustc's comment and literal rules.

    Kinds: ws, line_comment, block_comment, str, char, lifetime, ident, punct.
    Block comments nest. A run of consecutive punctuation is one token so that
    inserting whitespace inside `->` or `::` is a difference.
    """
    tokens, i, n = [], 0, len(text)
    def ident_end(j):
        while j < n and (text[j].isalnum() or text[j] == '_'):
            j += 1
        return j
    def quoted_end(j, quote):
        j += 1
        while j < n:
            if text[j] == '\\':
                j += 2
                continue
            if text[j] == quote:
                return j + 1
            j += 1
        raise CheckError('unterminated literal')
    while i < n:
        c = text[i]
        if c.isspace():
            j = i
            while j < n and text[j].isspace():
                j += 1
            tokens.append(('ws', text[i:j])); i = j; continue
        if text.startswith('//', i):
            j = text.find('\n', i)
            j = n if j < 0 else j
            tokens.append(('line_comment', text[i:j])); i = j; continue
        if text.startswith('/*', i):
            depth, j = 1, i + 2
            while depth:
                if j >= n:
                    raise CheckError('unterminated block comment')
                if text.startswith('/*', j):
                    depth += 1; j += 2
                elif text.startswith('*/', j):
                    depth -= 1; j += 2
                else:
                    j += 1
            tokens.append(('block_comment', text[i:j])); i = j; continue
        if c == '"':
            j = quoted_end(i, '"'); tokens.append(('str', text[i:j])); i = j; continue
        if c == "'":
            if i + 1 < n and text[i + 1] == '\\':
                j = quoted_end(i, "'"); tokens.append(('char', text[i:j])); i = j; continue
            if i + 2 < n and text[i + 2] == "'":
                tokens.append(('char', text[i:i + 3])); i += 3; continue
            j = ident_end(i + 1); tokens.append(('lifetime', text[i:j])); i = j; continue
        if c.isalnum() or c == '_':
            j = ident_end(i)
            word = text[i:j]
            if word in ('r', 'br', 'cr') and j < n and text[j] in '#"':
                k = j
                while k < n and text[k] == '#':
                    k += 1
                if k < n and text[k] == '"':
                    close = '"' + '#' * (k - j)
                    end = text.find(close, k + 1)
                    if end < 0:
                        raise CheckError('unterminated raw string')
                    end += len(close)
                    tokens.append(('str', text[i:end])); i = end; continue
            if word in ('b', 'c') and j < n and text[j] == '"':
                end = quoted_end(j, '"'); tokens.append(('str', text[i:end])); i = end; continue
            if word == 'b' and j < n and text[j] == "'":
                end = quoted_end(j, "'"); tokens.append(('char', text[i:end])); i = end; continue
            tokens.append(('ident', word)); i = j; continue
        j = i
        while j < n and not (text[j].isspace() or text[j].isalnum() or text[j] in '_"\'' or
                             text.startswith('//', j) or text.startswith('/*', j)):
            j += 1
        tokens.append(('punct', text[i:j])); i = j
    return tokens


def is_annotation(kind, text):
    return (kind == 'line_comment' and text.startswith('//@')) or (kind == 'block_comment' and text.startswith('/*@'))


def strip_annotations(text):
    """Remove VeriFast annotation comments, keeping every other byte."""
    return ''.join(t for k, t in lex(text) if not is_annotation(k, t))


def code_tokens(text):
    """Tokens rustc sees (whitespace dropped; ordinary comments kept so they must match too)."""
    return [(k, t) for k, t in lex(text) if k != 'ws']


def annotation_only_equal(verified, original):
    """True when `verified` differs from `original` only by VeriFast comments and whitespace."""
    return code_tokens(strip_annotations(verified)) == code_tokens(original)


def first_difference(verified, original):
    a, b = code_tokens(strip_annotations(verified)), code_tokens(original)
    for index, (x, y) in enumerate(zip(a, b)):
        if x != y:
            return f'token {index}: {x[1]!r} vs product {y[1]!r}'
    return f'token count {len(a)} vs product {len(b)}'


def fn_names(text):
    """Count named `fn` items outside comments and literals (a `$name` macro fragment counts)."""
    toks = [t for t in lex(text) if t[0] not in ('ws', 'line_comment', 'block_comment')]
    counts = {}
    for index, (kind, word) in enumerate(toks):
        if kind == 'ident' and word == 'fn' and index + 1 < len(toks):
            k, name = toks[index + 1]
            if k == 'punct' and name == '$' and index + 2 < len(toks) and toks[index + 2][0] == 'ident':
                name = '$' + toks[index + 2][1]
            elif k != 'ident':
                continue
            counts[name] = counts.get(name, 0) + 1
    return counts


def assumes_in_lemmas(text):
    """Return the lemma name enclosing each assume(..) in the annotation comments of `text`."""
    owners = []
    for kind, body in lex(text):
        if not is_annotation(kind, body):
            continue
        for match in re.finditer(r'\bassume\s*\(', body):
            lemmas = re.findall(r'\blem(?:_auto)?\s+([A-Za-z_][\w:]*)', body[:match.start()])
            owners.append(lemmas[-1] if lemmas else None)
    return owners


# ---------------------------------------------------------------- assembly
def load_manifest(proof=PROOF):
    return json.loads((proof / 'manifest.json').read_text())


def original_side(module, product_root):
    data = (product_root / module['product']).read_bytes()
    spec = module['original']
    if spec['kind'] == 'product':
        return data
    if spec['kind'] == 'product-cut':
        lines = data.splitlines(keepends=True)
        first, last = spec['keep_lines']
        if first != 1:
            raise CheckError('a cut keeps a prefix of the product file')
        if sha256_bytes(b''.join(lines[last:])) != spec['dropped_sha256']:
            raise CheckError(f'{module["product"]}: the dropped part changed; re-review the cut')
        return b''.join(lines[:last])
    raise CheckError('unknown original kind ' + spec['kind'])


def verified_side(module, product_root, proof):
    spec = module['verified']
    if spec['kind'] == 'product':
        return original_side(module, product_root)
    if spec['kind'] in ('annotations-only', 'rewrite'):
        return (proof / spec['stored']).read_bytes()
    raise CheckError('unknown verified kind ' + spec['kind'])


def assemble(out, manifest, product_root=ROOT, proof=PROOF):
    """Write out/original/src and out/verified/src; return {side: {path: bytes}}."""
    sides = {}
    for side in ('original', 'verified'):
        base = out / side / 'src'
        if base.parent.exists():
            shutil.rmtree(base.parent)
        files = {manifest['root']['path']: (proof / manifest['root']['stored']).read_bytes()}
        for module in manifest['modules']:
            files[module['path']] = (original_side if side == 'original' else
                                     lambda m, r: verified_side(m, r, proof))(module, product_root)
        for rel, data in files.items():
            (base / rel).parent.mkdir(parents=True, exist_ok=True)
            (base / rel).write_bytes(data)
        sides[side] = files
    return sides


def check_identity(out, manifest, product_root=ROOT):
    """Product pins and byte identity of every original-side file. Returns a list of errors."""
    errors = []
    for module in manifest['modules']:
        product = product_root / module['product']
        actual = sha256_bytes(product.read_bytes())
        if actual != module['product_sha256']:
            errors.append(f'{module["product"]} changed (sha256 {actual}, pinned {module["product_sha256"]}); '
                          're-review the proof and update the manifest')
            continue
        try:
            expected = original_side(module, product_root)
        except CheckError as error:
            errors.append(str(error)); continue
        for side in ('original',) + (('verified',) if module['verified']['kind'] == 'product' else ()):
            if (out / side / 'src' / module['path']).read_bytes() != expected:
                errors.append(f'{side}/{module["path"]} is not the product bytes of {module["product"]}')
    return errors


def check_strip(manifest, sides):
    """Annotations-only files must equal their original side after removing VeriFast comments."""
    results = {}
    for module in manifest['modules']:
        kind = module['verified']['kind']
        if kind == 'product':
            continue
        verified = sides['verified'][module['path']].decode()
        original = sides['original'][module['path']].decode()
        same = annotation_only_equal(verified, original)
        if kind == 'annotations-only':
            results[module['path']] = dict(kind=kind, passed=same, detail=None if same else first_difference(verified, original))
        else:
            results[module['path']] = dict(kind=kind, passed=True, code_differs=not same,
                                           detail='code rewrite; connected by the refinement checker')
    return results


def check_inventory(manifest, sides):
    errors, listed = [], {}
    for entry in manifest['functions']:
        listed[(entry['file'], entry['name'])] = entry['count']
    found = {}
    for path, data in sides['original'].items():
        if path == manifest['root']['path']:
            continue
        for name, count in fn_names(data.decode()).items():
            found[(path, name)] = count
    for key in sorted(set(found) | set(listed)):
        if found.get(key) != listed.get(key):
            errors.append(f'{key[0]}: fn {key[1]} found {found.get(key, 0)} time(s), manifest lists {listed.get(key, 0)}')
    return errors


def check_trusted_lemmas(manifest, sides):
    errors, trusted = [], {t['name'].split('::')[-1] for t in manifest['trusted_lemmas']}
    seen = []
    for path, data in sides['verified'].items():
        for owner in assumes_in_lemmas(data.decode()):
            short = owner.split('::')[-1] if owner else None
            seen.append(short)
            if short not in trusted:
                errors.append(f'{path}: assume(..) outside a trusted lemma (in {owner})')
    for name in trusted:
        if seen.count(name) != 1:
            errors.append(f'trusted lemma {name} has {seen.count(name)} assume(..), expected 1')
    return errors


# ---------------------------------------------------------------- tools
def run(cmd, cwd, log, timeout=1800, env=None):
    started = time.monotonic()
    with open(log, 'w') as f:
        try:
            proc = subprocess.run(cmd, cwd=cwd, stdout=f, stderr=subprocess.STDOUT, timeout=timeout,
                                  env=dict(os.environ, **(env or {})))
            code = proc.returncode
        except subprocess.TimeoutExpired:
            code = 'timeout'
    return code, Path(log).read_text(errors='replace'), round(time.monotonic() - started, 2)


def run_verifast(bindir, crate, manifest, log):
    cmd = [str(bindir / 'verifast'), *VERIFAST_FLAGS, '-verbose', '1', '-rustc_args', manifest['rustc_args'], 'src/lib.rs']
    code, text, wall = run(cmd, crate, log)
    match = re.search(r'^0 errors found \((\d+) statements verified\)', text, re.M)
    functions = sorted(set(re.findall(r"^\s*[\d.]+s: src/[^:]*: Verifying function '([^']+)'", text, re.M)))
    return dict(command=cmd, cwd=str(crate), exit=code, passed=code == 0 and match is not None,
                statements=int(match.group(1)) if match else None, functions=functions, wall_seconds=wall,
                log=str(log), error=None if match else next((l for l in text.splitlines() if re.search(r'\): error: ', l)),
                                              text.strip().splitlines()[-1] if text.strip() else 'no output'))


def run_refinement(bindir, original, verified, manifest, log):
    cmd = [str(bindir / 'refinement-checker'), '--rustc-args', manifest['rustc_args'],
           str(original / 'src' / 'lib.rs'), str(verified / 'src' / 'lib.rs')]
    code, text, wall = run(cmd, original.parent, log)
    passed = code == 0 and 'No refinement errors found' in text
    return dict(command=cmd, exit=code, passed=passed, wall_seconds=wall, log=str(log),
                error=None if passed else next((l for l in text.splitlines() if l.strip()), '')[:300])


def assemble_kani_crate(out, product_root=ROOT):
    crate = out / 'kani-crate'
    if crate.exists():
        shutil.rmtree(crate)
    crate.mkdir(parents=True)
    for name in ('Cargo.toml', 'Cargo.lock', 'clippy.toml'):
        shutil.copy2(product_root / name, crate / name)
    # The manifest declares the examples, so cargo needs them to parse it; only --lib is built.
    for name in ('src', 'examples'):
        shutil.copytree(product_root / name, crate / name)
    layout = (crate / 'src' / 'layout.rs').read_text()
    if '#[cfg(kani)]\nmod proofs;' not in layout:
        raise CheckError('src/layout.rs no longer declares `#[cfg(kani)] mod proofs;`')
    proofs = crate / 'src' / 'layout' / 'proofs.rs'
    proofs.write_text(proofs.read_text() + '\n' + KANI_HARNESS.read_text())
    return crate


def kani_summary(text):
    found = re.findall(r'Complete - (\d+) successfully verified harnesses, (\d+) failures, (\d+) total\.', text)
    return tuple(map(int, found[-1])) if found else None


def run_kani(out, log_dir):
    crate = assemble_kani_crate(out)
    env = {'CARGO_NET_OFFLINE': 'true', 'CARGO_TARGET_DIR': str(out / 'kani-target')}
    results = {}
    cmd = ['cargo', 'kani', '--lib'] + [x for h in KANI_POSITIVE for x in ('--harness', 'layout::proofs::' + h)] + ['--exact']
    code, text, wall = run(cmd, crate, log_dir / 'kani-positive.log', timeout=3600, env=env)
    summary = kani_summary(text)
    results['positive'] = dict(command=cmd, exit=code, summary=summary, wall_seconds=wall, log=str(log_dir / 'kani-positive.log'),
                               passed=code == 0 and summary == (len(KANI_POSITIVE), 0, len(KANI_POSITIVE)))
    cmd = ['cargo', 'kani', '--lib', '--harness', 'layout::proofs::' + KANI_NEGATIVE, '--exact']
    code, text, wall = run(cmd, crate, log_dir / 'kani-negative.log', timeout=3600, env=env)
    summary = kani_summary(text)
    results['negative'] = dict(command=cmd, exit=code, summary=summary, wall_seconds=wall, log=str(log_dir / 'kani-negative.log'),
                               rejected=code != 0 and summary == (0, 1, 1))
    return results


# ---------------------------------------------------------------- controls
def mutate(files, path, old, new):
    text = files[path].decode()
    if text.count(old) != 1:
        raise CheckError(f'control anchor not unique in {path}: {old[:60]!r}')
    files = dict(files)
    files[path] = text.replace(old, new).encode()
    return files


def write_crate(base, files):
    if base.exists():
        shutil.rmtree(base)
    for rel, data in files.items():
        (base / 'src' / rel).parent.mkdir(parents=True, exist_ok=True)
        (base / 'src' / rel).write_bytes(data)
    return base


def run_controls(out, manifest, sides, bindir, log_dir):
    controls = json.loads((PROOF / 'controls.json').read_text())
    results = []
    for control in controls:
        name = control['name']
        row = dict(name=name, description=control['description'], expect=control['expect'])
        if control.get('product_edit'):
            # Change a temporary copy of the product file; identity must fail.
            temp = out / 'controls' / name / 'product'
            if temp.exists():
                shutil.rmtree(temp)
            for module in manifest['modules']:
                (temp / module['product']).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / module['product'], temp / module['product'])
            edit = control['product_edit']
            target = temp / edit['path']
            text = target.read_text()
            if text.count(edit['old']) != 1:
                raise CheckError(f'control anchor not unique in {edit["path"]}')
            target.write_text(text.replace(edit['old'], edit['new']))
            assembled = out / 'controls' / name / 'assembled'
            assemble(assembled, manifest, product_root=temp)
            errors = check_identity(assembled, manifest, product_root=temp)
            row['identity'] = dict(passed=not errors, errors=errors)
        else:
            files = mutate(sides['verified'], control['file'], control['old'], control['new'])
            crate = write_crate(out / 'controls' / name / 'verified', files)
            if 'strip' in control['expect']:
                module = next(m for m in manifest['modules'] if m['path'] == control['file'])
                same = annotation_only_equal(files[control['file']].decode(), sides['original'][control['file']].decode())
                row['strip'] = dict(passed=same and module['verified']['kind'] == 'annotations-only')
            if 'verifast' in control['expect']:
                vf = run_verifast(bindir, crate, manifest, log_dir / f'control-{name}-verifast.log')
                row['verifast'] = dict(passed=vf['passed'], error=vf['error'], log=vf['log'])
            if 'refinement' in control['expect']:
                rc = run_refinement(bindir, out / 'original', crate, manifest, log_dir / f'control-{name}-refinement.log')
                row['refinement'] = dict(passed=rc['passed'], error=rc['error'], log=rc['log'])
        row['rejected'] = all(not row[check]['passed'] for check in control['expect'])
        results.append(row)
    return results


# ---------------------------------------------------------------- main
def tool_versions(bindir):
    def out(cmd):
        try:
            return subprocess.run(cmd, capture_output=True, text=True).stdout.strip().splitlines()[0]
        except (OSError, IndexError):
            return None
    pin = json.loads((HERE / 'pin.json').read_text())
    return dict(verifast=(bindir / 'VERSION').read_text().strip(), verifast_pin=pin,
                rustc_toolchain=out(['rustc', '+' + pin['toolchain'], '--version']),
                kani=out(['kani', '--version']), python=sys.version.split()[0])


def overlay_items():
    items = []
    for line in (HERE / 'std-spec-overlay.patch').read_text().splitlines():
        m = re.match(r'^\+\s*(fn|lem)\s+(\w+)', line)
        if m:
            items.append(('axiom ' if m.group(1) == 'lem' else 'spec of ') + m.group(2))
        elif re.match(r'^\+\s*//@ on_unwind_ens typeid', line):
            items.append('from_residual: on_unwind_ens true replaced by typeid(E) != typeid(F)')
    return items


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--output', type=Path, default=OUT_DEFAULT)
    p.add_argument('--cache', type=Path, help='VeriFast cache directory (setup.py)')
    p.add_argument('--offline', action='store_true', help='use only the cached release archive')
    p.add_argument('--install-toolchain', action='store_true')
    a = p.parse_args()
    out = a.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        p.error('keep generated output under the ignored target/ directory')
    if out.exists():
        shutil.rmtree(out)
    logs = out / 'logs'
    logs.mkdir(parents=True)
    sys.path.insert(0, str(HERE))
    import setup as vf_setup
    summary = dict(scope='Supplementary VeriFast evidence for a subset crate; not a certificate disposition.',
                   source=dict(commit=subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True).stdout.strip(),
                               dirty=bool(subprocess.run(['git', 'status', '--porcelain', '--', 'src', 'tools/verifast'], cwd=ROOT,
                                                         capture_output=True, text=True).stdout.strip())),
                   checks={}, passed=False)
    failures = []
    def record(name, ok, **data):
        data.pop('passed', None)
        summary['checks'][name] = dict(data, passed=ok)
        if not ok:
            failures.append(name)
        print(('PASS ' if ok else 'FAIL ') + name, flush=True)
    def save():
        (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    try:
        bindir = vf_setup.setup(a.cache, a.offline, a.install_toolchain)
        summary['tools'] = tool_versions(bindir)
        summary['verifast_flags'] = dict(flags=VERIFAST_FLAGS + ['-verbose', '1'], rustc_args=load_manifest()['rustc_args'], effects=FLAG_EFFECTS)
        record('setup', True, bindir=str(bindir))
        manifest = load_manifest()
        sides = assemble(out, manifest)
        record('assemble', True, original=str(out / 'original'), verified=str(out / 'verified'))
        errors = check_identity(out, manifest)
        record('identity', not errors, errors=errors)
        strip = check_strip(manifest, sides)
        record('strip', all(r['passed'] for r in strip.values()), files=strip)
        errors = check_inventory(manifest, sides)
        record('inventory', not errors, errors=errors)
        errors = check_trusted_lemmas(manifest, sides)
        record('trusted-lemmas', not errors, errors=errors)
        save()
        vf = run_verifast(bindir, out / 'verified', manifest, logs / 'verifast.log')
        expected = {f['vf_name'] for f in manifest['functions'] if 'vf_name' in f}
        declared = set()
        for data in sides['verified'].values():
            for kind, body in lex(data.decode()):
                if is_annotation(kind, body):
                    declared |= set(re.findall(r'\blem(?:_auto)?\s+([A-Za-z_][\w:]*)', body))
        unexpected = [f for f in vf['functions'] if f not in expected and f.split('::')[-1] not in
                      {d.split('::')[-1] for d in declared} and not GENERATED_LEMMA.search(f.split('::')[-1])]
        missing = sorted(expected - set(vf['functions']))
        vf.update(missing_expected=missing, unexpected=unexpected)
        record('verifast', vf['passed'] and not missing and not unexpected, **vf)
        rc = run_refinement(bindir, out / 'original', out / 'verified', manifest, logs / 'refinement.log')
        record('refinement', rc['passed'], **rc)
        kani = run_kani(out, logs)
        record('kani-array-spec', kani['positive']['passed'], **kani['positive'])
        controls = run_controls(out, manifest, sides, bindir, logs)
        controls.append(dict(name='kani-wrong-size-postcondition', description='Kani harness asserting size == count for u64',
                             expect=['kani'], rejected=kani['negative']['rejected'], kani=kani['negative']))
        record('controls', all(c['rejected'] for c in controls), controls=controls)
        coverage = []
        for f in manifest['functions']:
            row = {'file': f['file'], 'name': f['name'], 'count': f['count'], 'class': f['class'], 'note': f.get('note')}
            if 'vf_name' in f:
                row['vf_name'] = f['vf_name']; row['vf_verified'] = f['vf_name'] in vf['functions']
            coverage.append(row)
        summary['coverage'] = coverage
        summary['derived_not_checked'] = manifest['derived_not_checked']
        summary['trusted'] = dict(lemmas=manifest['trusted_lemmas'], overlay=overlay_items(),
                                  overlay_patch=str(HERE / 'std-spec-overlay.patch'))
    except (CheckError, OSError, subprocess.SubprocessError) as error:
        failures.append('error'); summary['error'] = str(error)
        print('FAIL error: ' + str(error), flush=True)
    except SystemExit as error:
        failures.append('setup'); summary['error'] = str(error)
        print('FAIL setup: ' + str(error), flush=True)
    summary['passed'] = not failures
    summary['failed'] = failures
    save()
    print(('ALL PASSED' if not failures else 'FAILED: ' + ', '.join(failures)) + '; ' + str(out / 'summary.json'))
    return 0 if not failures else 1


if __name__ == '__main__':
    raise SystemExit(main())
