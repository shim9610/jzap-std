#!/usr/bin/env python3
"""Compare the machine code of two builds function by function.

Build both executables with the same toolchain and flags, and with
`RUSTFLAGS="-C symbol-mangling-version=v0"` so that every monomorphization has
its own demangled name (legacy mangling drops generic arguments and merges
them). Addresses, jump targets and RIP-relative displacements are normalized,
so a function that only moved compares equal.

    python3 tools/perf-audit/codegen_diff.py BASE NEW [--match REGEX] [--show NAME]

Prints every function present in only one build or whose normalized
instructions differ, then a summary. `--show` prints a unified diff of the
instructions of the first function whose name contains NAME.
"""
import argparse
import difflib
import re
import subprocess
import sys


def functions(binary):
    out = subprocess.run(["objdump", "-d", "--no-show-raw-insn", "-C", binary],
                         capture_output=True, text=True, check=True).stdout
    found, name = {}, None
    for line in out.splitlines():
        header = re.match(r"^[0-9a-f]+ <(.*)>:$", line)
        if header:
            name = header.group(1)
            found[name] = []
            continue
        if name and line.startswith("  "):
            ins = line.split("\t", 1)[-1].strip()
            ins = re.sub(r"<[^>]*>", "<>", ins)
            ins = re.sub(r"#.*$", "", ins).strip()
            ins = re.sub(r"\b[0-9a-f]{4,}\b", "ADDR", ins)
            ins = re.sub(r"0x[0-9a-f]+\(%rip\)", "ADDR(%rip)", ins)
            ins = re.sub(r"0x[0-9a-f]{4,}", "ADDR", ins)
            found[name].append(ins)
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base")
    parser.add_argument("new")
    parser.add_argument("--match", default=".", help="only functions whose name matches this regex")
    parser.add_argument("--show", help="print the instruction diff of the first function containing this text")
    args = parser.parse_args()
    base, new = functions(args.base), functions(args.new)
    pattern = re.compile(args.match)
    same = differ = 0
    for name in sorted(set(base) | set(new)):
        if not pattern.search(name):
            continue
        if name not in base or name not in new:
            side = "base" if name in base else "new"
            print(f"ONLY {side}\t{len((base if name in base else new)[name])}\t{name}")
        elif base[name] == new[name]:
            same += 1
        else:
            differ += 1
            print(f"DIFF\t{len(base[name])} -> {len(new[name])}\t{name}")
    print(f"same {same}, different {differ}")
    if args.show:
        names = [name for name in base if args.show in name and name in new]
        if not names:
            sys.exit(f"no function in both builds contains {args.show!r}")
        for line in difflib.unified_diff(base[names[0]], new[names[0]], lineterm="", n=1):
            print(line)


if __name__ == "__main__":
    main()
