#!/usr/bin/env python3
"""Per-instruction execution profile of one function from a Callgrind run.

Record with per-instruction positions, optionally with the cache simulator and
limited to the timed region:

    valgrind --tool=callgrind --dump-instr=yes [--cache-sim=yes] \\
        --toggle-collect='crate::measure' --callgrind-out-file=OUT BINARY ...

Then:

    python3 tools/perf-audit/hot_instructions.py OUT BINARY FUNCTION_REGEX \\
        --ops N [--top 40] [--compare OUT2 BINARY2]

`--ops` divides every count by the number of timed operations. The report
lists the function's own cost per operation for every recorded event (Ir, and
Dr/Dw/D1mr/D1mw/DLmr/DLmw with the cache simulator), the hottest
instructions, executed `call` instructions with their targets (an executed
`call` counts its calls exactly), instructions with a stack operand, and the
executed opcode mix. With `--compare`, it prints the same summary for the
second profile and the per-opcode difference.

Costs are self costs: instructions inside callees (for example `memcmp`) are
not included; the call counts show how often each callee was entered.
"""
import argparse
import collections
import re
import subprocess


def profile(path, pattern):
    """Self costs per instruction address of functions matching `pattern`."""
    events, costs = [], collections.defaultdict(lambda: None)
    names, current, columns, last, after_call = {}, None, 1, [], False
    regex = re.compile(pattern)
    with open(path) as source:
        for line in source:
            line = line.rstrip("\n")
            if line.startswith("positions:"):
                columns = len(line.split()[1:])
                last = [0] * columns
                continue
            if line.startswith("events:"):
                events = line.split()[1:]
                continue
            match = re.match(r"^(c?fn)=\((\d+)\)(?: (.*))?$", line)
            if match:
                if match.group(3):
                    names[match.group(2)] = match.group(3)
                if match.group(1) == "fn":
                    current = names.get(match.group(2))
                    after_call = False
                continue
            if line.startswith("calls="):
                after_call = True
                continue
            if not line or not (line[0].isdigit() or line[0] in "+-*"):
                continue
            fields = line.split()
            positions, values = fields[:columns], fields[columns:]
            for i, token in enumerate(positions):
                if token == "*":
                    continue
                if token[0] == "+":
                    last[i] += int(token[1:])
                elif token[0] == "-":
                    last[i] -= int(token[1:])
                else:
                    last[i] = int(token, 0)
            if after_call:
                # The line after `calls=` carries the callee's inclusive cost.
                after_call = False
                continue
            if current is None or not regex.search(current):
                continue
            address = last[0]
            row = costs[address] or [0] * len(events)
            for i, value in enumerate(values):
                row[i] += int(value)
            costs[address] = row
    return events, {address: row for address, row in costs.items() if row}


def disassembly(binary):
    out = subprocess.run(["objdump", "-d", "--no-show-raw-insn", "-C", binary],
                         capture_output=True, text=True, check=True).stdout
    found = {}
    for line in out.splitlines():
        match = re.match(r"^\s+([0-9a-f]+):\t(.*)$", line)
        if match:
            found[int(match.group(1), 16)] = match.group(2).strip()
    return found


def summary(path, binary, pattern, ops, top):
    events, costs = profile(path, pattern)
    code = disassembly(binary)
    ir = events.index("Ir")
    totals = [sum(row[i] for row in costs.values()) / ops for i in range(len(events))]
    print(f"== {path}: {len(costs)} executed instructions")
    print("self per op: " + ", ".join(f"{name} {value:.2f}" for name, value in zip(events, totals)))
    print(f"-- hottest {top}")
    for address, row in sorted(costs.items(), key=lambda item: -item[1][ir])[:top]:
        extra = " ".join(f"{name}={value / ops:.2f}" for name, value in zip(events, row) if name != "Ir" and value)
        print(f"{address:#x} {row[ir] / ops:9.2f} {code.get(address, '?')[:90]} {extra}")
    calls = collections.Counter()
    stack = 0.0
    mix = collections.Counter()
    for address, row in costs.items():
        text = code.get(address, "?")
        mix[text.split()[0] if text.split() else "?"] += row[ir] / ops
        if "(%rsp)" in text or "(%rbp)" in text:
            stack += row[ir] / ops
        if text.startswith("call"):
            target = re.search(r"<([^>]*)>", text)
            calls[target.group(1) if target else text] += row[ir] / ops
    print("-- executed calls per op")
    for target, count in calls.most_common():
        print(f"{count:9.2f} {target[:120]}")
    print(f"-- instructions with a stack operand per op: {stack:.2f}")
    return mix


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("out")
    parser.add_argument("binary")
    parser.add_argument("function", help="regex over demangled function names")
    parser.add_argument("--ops", type=int, required=True)
    parser.add_argument("--top", type=int, default=40)
    parser.add_argument("--compare", nargs=2, metavar=("OUT2", "BINARY2"))
    args = parser.parse_args()
    first = summary(args.out, args.binary, args.function, args.ops, args.top)
    if args.compare:
        second = summary(args.compare[0], args.compare[1], args.function, args.ops, args.top)
        print("-- opcode difference per op (second - first)")
        for opcode in sorted(set(first) | set(second), key=lambda o: -abs(second[o] - first[o])):
            delta = second[opcode] - first[opcode]
            if abs(delta) >= 0.05:
                print(f"{opcode:10s} {first[opcode]:9.2f} {second[opcode]:9.2f} {delta:+9.2f}")


if __name__ == "__main__":
    main()
