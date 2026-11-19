#!/usr/bin/env python3
"""Reproduce fixed-instruction and real-consumer Heap layout diagnostics.

Only ignored target/ storage is mutated. Product candidates are source forms in
a pinned Git archive, never runtime patches or substitutes for product proofs.
Run prepare before timing; no compilation takes place during measurements.
"""
import argparse
from collections import Counter
import csv
import hashlib
import json
import math
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
PIN = "38e4b655470ccb2f1e4d770b1a485e5943c5e7a5"
FORMS = ("base", "caller_inline", "caller_outline", "sift_outline", "push_inline", "growth_not_cold")
LINK_SEEDS = (617, 6203, 96011, 104729, 130363, 155921, 196613, 262147)


def command(args, **kw):
    return subprocess.check_output([str(x) for x in args], text=True, **kw)


def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def assembly():
    code, declarations, names = [".text"], [], []
    for replica in range(2):
        for form, index, end in (("std", "%r10", "%r11"), ("jzap", "%r9", "%r8")):
            for offset in (0, 16, 32, 48):
                name = f"scan_{form}_{replica}_{offset}"
                loop = name + "_loop"
                names.append(name)
                declarations.append(f"fn {name}(data: *const u64, len: usize) -> u64;")
                code += [".p2align 6", f".global {name}", f".type {name}, @function", name + ":",
                         f"mov %rsi, {end}", f"xor {index}d, {index}d",
                         "pxor %xmm0, %xmm0", "pxor %xmm1, %xmm1",
                         # Fixed-width jump skips every padding byte.
                         ".byte 0xe9", f".long {loop} - . - 4", ".p2align 6",
                         f".fill {offset}, 1, 0x90", f".global {loop}", loop + ":", ".L" + loop + ":",
                         f"movdqu (%rdi,{index},8), %xmm2", "paddq %xmm2, %xmm1",
                         f"movdqu 16(%rdi,{index},8), %xmm2", "paddq %xmm2, %xmm0",
                         f"add $4, {index}", f"cmp {index}, {end}", f"jne .L{loop}",
                         "paddq %xmm1, %xmm0", "movdqa %xmm0, %xmm1",
                         "psrldq $8, %xmm1", "paddq %xmm1, %xmm0",
                         "movq %xmm0, %rax", "ret", f".size {name}, . - {name}"]
    code += ['.section .note.GNU-stack,"",@progbits']
    decl = 'type Kernel = unsafe extern "C" fn(*const u64, usize) -> u64;\n'
    decl += 'unsafe extern "C" {\n' + "\n".join(declarations) + "\n}\n"
    decl += f"static KERNELS: [(&str, Kernel); {len(names)}] = [\n"
    decl += "\n".join(f'("{name}", {name}),' for name in names) + "\n];\n"
    return "\n".join(code) + "\n", decl, names


def inspect_kernel(binary, names):
    dump = command(["objdump", "-d", binary])
    labels = {m[2]: int(m[1], 16) for m in re.finditer(r"^([0-9a-f]+) <([^>]+)>:", dump, re.M)}
    instructions = {}
    for line in dump.splitlines():
        m = re.match(r"\s*([0-9a-f]+):\s*((?:[0-9a-f]{2}\s+)+)\s*(.*)", line)
        if m and m[3].strip():
            instructions[int(m[1], 16)] = (bytes.fromhex(m[2]), m[3])
    rows, by_form = [], {}
    for name in names:
        start = labels[name + "_loop"]
        pc, body = start, []
        for _ in range(7):
            encoded, op = instructions[pc]
            body.append(encoded)
            pc += len(encoded)
        encoded = b"".join(body)
        form = name.split("_")[1]
        expected = {
            "std": "f3420f6f14d7660fd4caf3420f6f54d710660fd4c24983c2044d39d375e2",
            "jzap": "f3420f6f14cf660fd4caf3420f6f54cf10660fd4c24983c1044d39c875e2",
        }
        assert len(encoded) == 30 and encoded[-2:] == bytes.fromhex("75e2")
        assert encoded.hex() == expected[form], "observed scan instruction bytes changed"
        assert start % 64 == int(name.rsplit("_", 1)[1])
        assert by_form.setdefault(form, encoded) == encoded, "instruction drift"
        rows.append(dict(kernel=name, address=start, offset=start % 64,
                         bytes=encoded.hex(), instructions=7, length=30))
    return rows


def function_shapes(binary):
    """Compare instruction forms, masking relocations and legacy Rust hashes.

    This is a layout diagnostic, not binary equivalence: RIP-relative data
    addresses and external call specializations are not validated by this mask.
    Exact hot-loop bytes and equal-work results are checked separately.
    """
    symbols = {}
    for line in command(["nm", "-S", "--defined-only", binary]).splitlines():
        row = line.split()
        if len(row) == 4 and row[2].lower() == "t":
            symbols[int(row[0], 16)] = (int(row[1], 16), row[3])
    functions, current, start, end = {}, None, 0, 0
    for line in command(["objdump", "-d", "--no-show-raw-insn", binary]).splitlines():
        label = re.match(r"^([0-9a-f]+) <([^>]+)>:", line)
        if label:
            address = int(label[1], 16)
            if address in symbols:
                start = address
                length, name = symbols[start]
                end = start + length
                current = name, length
                functions[current] = []
            elif not (current is not None and start <= address < end):
                current = None
            continue
        instruction = re.match(r"\s*([0-9a-f]+):\s+(.*)", line)
        if current is None or not instruction or int(instruction[1], 16) >= end:
            continue
        op = instruction[2].split("#")[0].strip()
        op = re.sub(r"-?0x[0-9a-f]+\(%rip\)", "RIP", op)
        branch = re.match(r"(j\w+|call)\s+([0-9a-f]+)(.*)", op)
        if branch:
            target = int(branch[2], 16)
            if start <= target < end:
                op = branch[1] + " internal+" + hex(target - start)
            else:
                tail = re.sub(r"17h[0-9a-f]{16}E", "17hHASH_E", branch[3])
                op = branch[1] + re.sub(r"\+0x[0-9a-f]+", "+OFFSET", tail)
        functions[current].append(op)
    return Counter(
        ((re.sub(r"17h[0-9a-f]{16}E$", "17hHASH_E", name), length),
         hashlib.sha256("\n".join(ops).encode()).hexdigest())
        for (name, length), ops in functions.items())


def inspect_links(out, reference, seeds):
    baseline = function_shapes(reference)
    results = {}
    for seed in seeds:
        name = f"link_{seed}"
        candidate = function_shapes(out / name)
        results[name] = dict(reference_functions=sum(baseline.values()),
                             matching_functions=sum((baseline & candidate).values()),
                             different_reference_functions=sum((baseline - candidate).values()),
                             different_candidate_functions=sum((candidate - baseline).values()))
    save(out / "function-shapes.json", dict(reference=str(reference),
         reference_sha256=digest(reference), comparisons=results,
         scope="Normalized instruction forms only; masked addresses are not a binary-equivalence proof"))
    print(json.dumps(results, indent=2))


def prepare(out, source_ref, forms, link_seeds):
    out.mkdir(parents=True, exist_ok=True)
    source = command(["git", "rev-parse", source_ref + "^{commit}"], cwd=ROOT).strip()
    work = out / "work"
    work.mkdir(exist_ok=True)
    archive = out / "source.tar"
    with archive.open("wb") as stream:
        subprocess.run(["git", "archive", source], cwd=ROOT, stdout=stream, check=True)
    subprocess.run(["tar", "-xf", archive, "-C", work], check=True)
    archive.unlink()
    (out / "scan.S").write_text(assembly()[0])
    (out / "scan_declarations.rs").write_text(assembly()[1])
    shutil.copyfile(HERE / "experiments/heap_layout.rs", out / "probe.rs")
    command(["rustc", "--edition=2024", "-Copt-level=3", "-Ccodegen-units=1", "-Cpanic=abort",
             out / "probe.rs", "-o", out / "kernel"])
    loops = inspect_kernel(out / "kernel", assembly()[2])
    files = ["tools/throughput-analysis/src/bin/binary_heap.rs", "src/collections/binary_heap/core.rs", "src/collections/binary_heap.rs", "src/vec.rs"]
    originals = {name: (work / name).read_text() for name in files}
    plan = [(form, None) for form in forms] + [(f"link_{s}", s) for s in link_seeds]
    previous = out / "preparation.json"
    prior = json.loads(previous.read_text()) if previous.exists() else {}
    assert not prior or prior["source_commit"] == source, "source reference changed"
    binaries = prior.get("binaries", {})
    for form, seed in plan:
        for name, contents in originals.items():
            (work / name).write_text(contents)
        if form in ("caller_inline", "caller_outline"):
            file = work / files[0]
            mode = "always" if form == "caller_inline" else "never"
            before = "fn action<'a, T: Item, H: Owner<'a, T>, const OP: usize>("
            assert originals[files[0]].count(before) == 1
            file.write_text(originals[files[0]].replace(before, f"#[inline({mode})]\n" + before))
        elif form == "sift_outline":
            file = work / files[1]
            before = "#[inline]\npub(super) unsafe fn sift_up"
            assert originals[files[1]].count(before) == 1
            file.write_text(originals[files[1]].replace(before, "#[inline(never)]\npub(super) unsafe fn sift_up"))
        elif form == "push_inline":
            file = work / files[2]
            before = "#[inline]\n    pub fn try_push"
            assert originals[files[2]].count(before) == 1
            file.write_text(originals[files[2]].replace(before, "#[inline(always)]\n    pub fn try_push"))
        elif form == "growth_not_cold":
            file = work / files[3]
            before = "#[cold]\n    #[inline(never)]\n    fn grow_for_push"
            assert originals[files[3]].count(before) == 1
            file.write_text(originals[files[3]].replace(before, "#[inline(never)]\n    fn grow_for_push"))
        env = os.environ.copy()
        env["CARGO_BUILD_JOBS"] = "2"
        env.pop("RUSTFLAGS", None)
        if seed is not None:
            env["RUSTFLAGS"] = f"-Clink-arg=-Wl,--shuffle-sections=.text.*={seed}"
        with (out / f"{form}-build.log").open("w") as log:
            subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "--manifest-path",
                            work / "tools/throughput-analysis/Cargo.toml", "--bin", "binary_heap",
                            "--target-dir", out / "call-build"], cwd=ROOT, env=env,
                           stdout=log, stderr=log, check=True)
        binary = out / form
        shutil.copyfile(out / "call-build/release/binary_heap", binary)
        binary.chmod(0o755)
        binaries[form] = dict(path=str(binary), sha256=digest(binary), flags=env.get("RUSTFLAGS", ""))
        print("BUILT", form, flush=True)
        save(out / "preparation.json", dict(source_commit=source, binaries=binaries, loops=loops,
             rustc=command(["rustc", "-Vv"]), scope="Analysis only; product source untouched"))


def prepare_reserved(out, mixed=False):
    """Restore the pinned ordinary library; build a safe shared-buffer probe."""
    preparation = json.loads((out / "preparation.json").read_text())
    source, work = preparation["source_commit"], out / "work"
    for file in ("tools/throughput-analysis/src/bin/binary_heap.rs",
                 "src/collections/binary_heap/core.rs", "src/collections/binary_heap.rs", "src/vec.rs"):
        (work / file).write_text(command(["git", "show", source + ":" + file], cwd=ROOT))
    probe = (HERE / "experiments/heap_reserved.rs").read_text()
    name = "reserved_mixed" if mixed else "reserved_layout"
    output = "reserved-mixed" if mixed else "reserved"
    if mixed:
        # Change only preparation/metadata in this timing harness. The two
        # actual Heap fill functions and measurement body remain unchanged.
        start = probe.index("    let mut cases: Vec<Vec<u64>>")
        end = probe.index("    let expected =", start)
        probe = probe[:start] + '''    let original = ordered_corpus::corpus(seed, false);
    let mut digest = 0xcbf29ce484222325u64;
    for case in &original {
        for v in &case.keys { digest = (digest ^ *v).wrapping_mul(0x100000001b3); }
        digest = (digest ^ case.pivot).wrapping_mul(0x100000001b3);
    }
    let cases: Vec<Vec<u64>> = original.into_iter().map(|c| c.keys).collect();
''' + probe[end:]
        for before, after in [
            ("let size: usize = args[1].parse().unwrap();", "let size: i64 = args[1].parse().unwrap();\n    assert_eq!(size, -1);"),
            ("cases.iter().map(|_| Vec::with_capacity(size))", "cases.iter().map(|v| Vec::with_capacity(v.len()))"),
            ("epochs * 64 * size as u64", "epochs * cases.iter().map(|v| v.len() as u64).sum::<u64>()"),
            (r'\"checksum\":{}', r'\"corpus_digest\":{digest},\"checksum\":{}'),
        ]:
            assert probe.count(before) == 1, "Review the mixed-corpus preparation anchor"
            probe = probe.replace(before, after)
        probe = '#[path = "../ordered_corpus.rs"]\nmod ordered_corpus;\n' + probe.replace("//! Analysis only:", "// Analysis only:", 1)
    (work / f"tools/throughput-analysis/src/bin/{name}.rs").write_text(probe)
    env = os.environ.copy()
    env["CARGO_BUILD_JOBS"] = "2"
    env.pop("RUSTFLAGS", None)
    with (out / f"{output}-build.log").open("w") as log:
        subprocess.run(["cargo", "rustc", "--release", "--locked", "--offline",
                        "--manifest-path", work / "tools/throughput-analysis/Cargo.toml",
                        "--bin", name, "--target-dir", out / "reserved-build",
                        "--", "-Cdebuginfo=1", "-Cstrip=none"], env=env, cwd=ROOT,
                       stdout=log, stderr=log, check=True)
    shutil.copyfile(out / f"reserved-build/release/{name}", out / output)
    (out / output).chmod(0o755)
    print("BUILT shared-buffer reserved insertion probe", flush=True)


def run_kernel(out, sizes, seeds, kind="kernel"):
    os.sched_setaffinity(0, {min(os.sched_getaffinity(0))})
    binary = out / kind
    entry, count = ("kernel", 16) if kind == "kernel" else ("label", 3)
    sha = digest(binary)
    protocol = dict(binary_sha256=sha, sizes=sizes, seeds=seeds,
                    cpu=min(os.sched_getaffinity(0)), minimum_seconds=1,
                    driver_sha256=digest(Path(__file__)),
                    interpretation="Placement or shared-buffer diagnostic; not a full lifecycle benchmark or Stabilizer implementation")
    protocol_file = out / f"{kind}-protocol.json"
    if protocol_file.exists():
        assert json.loads(protocol_file.read_text()) == protocol, "resume protocol drift"
    else:
        save(protocol_file, protocol)
    table = out / f"{kind}-timing.csv"
    records = []
    if table.exists():
        with table.open() as stream:
            for r in csv.DictReader(stream):
                records.append({k: v if k == entry else float(v) if k == "seconds" else int(v)
                                for k, v in r.items()})
    completed = {(r["size"], r["seed"]) for r in records}
    for size, seed in completed:
        group = [r for r in records if (r["size"], r["seed"]) == (size, seed)]
        assert len(group) == count and len({r[entry] for r in group}) == count
        assert min(r["seconds"] for r in group) >= 1
    for size in sizes:
        for seed in seeds:
            if (size, seed) in completed:
                continue
            epochs = 1000
            while True:
                rows = [json.loads(s) for s in command([binary, size, seed, epochs]).splitlines()]
                shortest = min(r["seconds"] for r in rows)
                if shortest >= 0.05:
                    break
                epochs *= min(1024, max(2, math.ceil(0.06 / shortest)))
            epochs = math.ceil(epochs * 1.2 / shortest)
            while True:
                rows = [json.loads(s) for s in command([binary, size, seed, epochs]).splitlines()]
                assert len(rows) == count and len({r[entry] for r in rows}) == count
                assert len({(r["epochs"], r["operations"], r["elements"], r["checksum"]) for r in rows}) == 1
                shortest = min(r["seconds"] for r in rows)
                if shortest >= 1:
                    break
                epochs = math.ceil(epochs * max(1.3, 1.3 / shortest))
            records.extend(rows)
            assert digest(binary) == sha
            temporary = table.with_suffix(".tmp")
            with temporary.open("w") as stream:
                writer = csv.DictWriter(stream, fieldnames=list(records[0]))
                writer.writeheader()
                writer.writerows(records)
            temporary.replace(table)
            print("COMPLETE", kind, size, seed, "minimum", round(shortest, 3), flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("mode", choices=("prepare", "kernel", "inspect", "prepare-reserved", "reserved", "prepare-mixed", "mixed"))
    p.add_argument("--output", type=Path, default=ROOT / "target/verification/heap-layout")
    p.add_argument("--source-ref", default=PIN)
    p.add_argument("--forms", default=",".join(FORMS))
    p.add_argument("--link-seeds", default=",".join(map(str, LINK_SEEDS)))
    p.add_argument("--sizes", default="16,256,4096")
    p.add_argument("--seeds", default="617,6203,96011")
    p.add_argument("--reference", type=Path, help="Instruction-form reference for inspect; default: prepared base")
    args = p.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to(ROOT / "target"), "Use ignored target/ storage"
    assert platform.system() == "Linux" and platform.machine() == "x86_64", "This diagnostic requires Linux x86-64"
    if args.mode == "prepare":
        forms = args.forms.split(",") if args.forms else []
        assert len(set(forms)) == len(forms) and set(forms) <= set(FORMS)
        prepare(out, args.source_ref, forms, [int(s) for s in args.link_seeds.split(",") if s])
    elif args.mode in ("kernel", "reserved"):
        run_kernel(out, [int(s) for s in args.sizes.split(",")], [int(s) for s in args.seeds.split(",")], args.mode)
    elif args.mode == "prepare-reserved":
        prepare_reserved(out)
    elif args.mode == "prepare-mixed":
        prepare_reserved(out, mixed=True)
    elif args.mode == "mixed":
        run_kernel(out, [-1], [int(s) for s in args.seeds.split(",")], "reserved-mixed")
    else:
        inspect_links(out, args.reference or out / "base", [int(s) for s in args.link_seeds.split(",") if s])
