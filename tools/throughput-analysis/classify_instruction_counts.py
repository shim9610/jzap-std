#!/usr/bin/env python3
"""Classify instruction counts against std by copy mechanism.

callgrind counts each `rep movs/stos` iteration of libc memcpy/memset as one
instruction, while an inlined copy loop counts its real instructions. Totals
are therefore comparable only when both sides copy the same way:

- no-copy: copy functions are at most 5% of either side's total;
- same-copy: both sides call copy functions with counts within 5%;
- copy-differs: otherwise. The ratio is not meaningful; time these cases.

The ratio compares code plus allocator instructions (copy excluded).
"""
import argparse
import csv
import collections


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input')
    parser.add_argument('output')
    args = parser.parse_args()
    rows = list(csv.DictReader(open(args.input)))
    key = lambda row: (row['group'], row['profile'], row['task'])
    std = {key(row): row for row in rows if row['label'] == 'std' and row['status'] == 'ok'}
    out = []
    for row in rows:
        if row['label'] == 'std' or row['status'] != 'ok' or key(row) not in std:
            continue
        base = std[key(row)]
        number = lambda r, field: float(r[field])
        std_copy, copy = number(base, 'copy'), number(row, 'copy')
        std_total, total = number(base, 'per_call'), number(row, 'per_call')
        std_rest = number(base, 'code') + number(base, 'allocator')
        rest = number(row, 'code') + number(row, 'allocator')
        if std_copy <= 0.05 * std_total and copy <= 0.05 * total:
            kind = 'no-copy'
        elif std_copy > 0 and copy > 0 and abs(copy - std_copy) <= 0.05 * max(copy, std_copy):
            kind = 'same-copy'
        else:
            kind = 'copy-differs'
        out.append(dict(group=row['group'], profile=row['profile'], task=row['task'], label=row['label'],
                        class_=kind, std_instructions=round(std_rest, 1), instructions=round(rest, 1),
                        ratio=round(rest / std_rest, 4) if std_rest > 0 else '',
                        std_copy=round(std_copy, 1), copy=round(copy, 1)))
    with open(args.output, 'w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(out[0]))
        writer.writeheader()
        writer.writerows(out)
    print(collections.Counter(row['class_'] for row in out))


if __name__ == '__main__':
    main()
