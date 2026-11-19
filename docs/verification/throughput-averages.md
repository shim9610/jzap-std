# Per-operation means from repeated long-interval measurements

The [Vec optimization checkpoint](vec-optimization.md) records the subsequent
source changes and renewed measurements. The averages below retain the preceding
baseline campaign.

The reporting standard is to set a fixed workload for each comparison group,
record at least one second per implementation, repeat, then calculate averages.
The existing [randomized campaign](throughput-long.md) supplies 1,584 valid
recordings with three seeds; its shortest recording is 1.022408 seconds.
This report recalculates those recordings without collecting new timings.

## Aggregation and units

- Mean recording time = sum of valid elapsed seconds / number of repeats.
- Mean time per operation = sum of valid elapsed seconds / sum of executed calls.
- Multiply the latter by 1,000,000,000 for ns/op, or by 1,000,000 for microseconds/op.
- Relative change = `(jzap mean / std mean - 1) * 100`; negative is faster.

Repeats can have different call counts, so the primary per-operation value is
weighted by executed calls. The unweighted mean of the seed-normalized values
is retained separately. Calculations use full-precision raw recordings rather
than rounded figures from the earlier report. Calibration, incomplete attempts
and unselected short intervals are excluded. Original outliers are retained.

**One operation is one complete workload call, not one element.** Vec edits
include source cloning, editing and result destruction; pop consumes the entire
vector. Box creation/clone rows include destruction of the temporary owner.
The inputs contain varied lengths/ranges; these values describe that declared
corpus. A derived ns/op is not the latency of a single timed call.

The following tables use the default compiler setting and three-seed pooled
means. Main Vec cases span 0–131,072 u64 elements; dynamic Box cases span
0–1,048,576 bytes. Length-limit and byte-budget policies are distinct, and allow
the same successful work. All settings and per-seed statistics are in the
locally generated `target/verification/throughput-long/per-operation.csv` and
`per-operation.json`. This normalization preserves every original std ratio;
it is not a new confidence-interval acceptance decision.

## Vec, mixed sizes

| Operation | Calls | std ns/op | Unmetered ns/op | Length limit ns/op | Budgeted ns/op | Limit + budget ns/op | Unmetered change | Budgeted change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| grow | 204,288 | 18,582.409 | 18,450.388 | 18,920.848 | 18,732.661 | 18,174.315 | -0.7% | +0.8% |
| reserved | 209,536 | 19,796.960 | 19,393.268 | 18,270.327 | 17,395.395 | 18,226.602 | -2.0% | -12.1% |
| copy | 710,912 | 5,667.607 | 5,573.708 | 5,550.367 | 5,280.704 | 5,385.654 | -1.7% | -6.8% |
| clone | 703,744 | 5,895.826 | 5,939.710 | 5,637.509 | 5,604.725 | 5,773.107 | +0.7% | -4.9% |
| retain | 68,672 | 61,730.878 | 62,623.759 | 60,726.028 | 58,506.254 | 59,691.981 | +1.4% | -5.2% |
| dedup | 132,672 | 31,049.699 | 34,743.576 | 34,830.024 | 35,822.526 | 35,191.998 | +11.9% | +15.4% |
| drain | 911,936 | 4,668.224 | 4,496.114 | 4,344.620 | 4,736.444 | 5,112.004 | -3.7% | +1.5% |
| extract | 164,288 | 23,688.331 | 25,106.775 | 24,597.030 | 23,886.743 | 23,820.873 | +6.0% | +0.8% |
| splice | 314,560 | 12,662.411 | 13,313.531 | 13,974.587 | 14,841.707 | 14,155.142 | +5.1% | +17.2% |
| within | 849,344 | 4,508.108 | 4,632.369 | 4,375.681 | 4,303.342 | 4,441.330 | +2.8% | -4.5% |
| splice_unknown | 124,096 | 60,226.431 | 30,986.510 | 31,302.282 | 32,345.603 | 28,972.359 | -48.5% | -46.3% |
| splice_no_hint | 103,616 | 65,688.367 | 39,571.958 | 38,967.952 | 39,742.538 | 39,220.425 | -39.8% | -39.5% |
| extract_none | 832,384 | 5,059.451 | 4,889.787 | 5,067.880 | 7,042.610 | 6,655.842 | -3.4% | +39.2% |
| extract_all | 273,536 | 14,687.716 | 13,627.185 | 15,360.631 | 14,885.478 | 14,650.890 | -7.2% | +1.3% |
| insert_remove | 433,728 | 9,539.853 | 8,830.098 | 9,099.249 | 9,535.514 | 9,362.381 | -7.4% | -0.0% |
| resize | 541,504 | 7,777.080 | 8,667.120 | 7,847.342 | 8,528.048 | 8,161.283 | +11.4% | +9.7% |
| split_off | 337,216 | 12,362.539 | 12,680.124 | 12,286.263 | 12,612.482 | 12,770.159 | +2.6% | +2.0% |
| append | 52,416 | 79,572.366 | 83,702.093 | 80,907.663 | 80,204.773 | 74,368.153 | +5.2% | +0.8% |
| shrink | 176,384 | 22,252.859 | 24,398.911 | 22,364.060 | 22,000.460 | 23,888.102 | +9.6% | -1.1% |
| pop | 425,728 | 9,285.140 | 9,698.483 | 9,587.266 | 9,928.774 | 10,464.429 | +4.5% | +6.9% |

## Box, mixed sizes

| Operation | Calls | std ns/op | Unmetered ns/op | Budgeted ns/op | Unmetered change | Budgeted change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| box_u64_create_drop | 396,816,448 | 9.770 | 8.728 | 12.655 | -10.7% | +29.5% |
| box_u64_clone_drop | 429,732,352 | 9.807 | 9.206 | 12.135 | -6.1% | +23.7% |
| box_u64_clone_from | 2,497,062,272 | 1.532 | 1.522 | 1.544 | -0.7% | +0.8% |
| box_slice_create_drop | 1,315,520 | 2,991.879 | 2,920.097 | 3,089.545 | -2.4% | +3.3% |
| box_slice_clone_drop | 952,448 | 4,459.488 | 4,244.840 | 4,065.395 | -4.8% | -8.8% |
| box_slice_clone_from | 1,279,680 | 3,308.955 | 3,223.467 | 3,081.863 | -2.6% | -6.9% |
| box_str_create_drop | 963,520 | 4,393.611 | 4,337.138 | 4,599.884 | -1.3% | +4.7% |
| box_str_clone_drop | 863,744 | 4,534.333 | 4,183.182 | 4,452.195 | -7.7% | -1.8% |
| box_str_clone_from | 875,968 | 9,011.616 | 4,344.574 | 4,458.066 | -51.8% | -50.5% |
| box_zeroed_create_drop | 2,104,896 | 1,868.057 | 1,853.295 | 1,829.485 | -0.8% | -2.1% |
| box_collect_create_drop | 1,098,112 | 3,798.785 | 3,847.556 | 3,697.832 | +1.3% | -2.7% |

## Vec, focused sizes 0–64

| Operation | Calls | std ns/op | Unmetered ns/op | Length limit ns/op | Budgeted ns/op | Limit + budget ns/op | Unmetered change | Budgeted change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| clone | 287,354,816 | 13.919 | 13.395 | 13.211 | 16.851 | 17.906 | -3.8% | +21.1% |
| extract_all | 152,864,640 | 24.113 | 31.032 | 32.851 | 29.093 | 32.083 | +28.7% | +20.7% |
| pop | 121,140,928 | 31.293 | 34.419 | 30.482 | 33.607 | 32.928 | +10.0% | +7.4% |

## Reproduction and verification

The standard reporting command, after the long-interval sampler finishes, is:

```sh
python3 tools/throughput-analysis/report.py --input target/verification/throughput-long
```

It verifies the completed groups, identical corpus/work counts and the
one-second minimum, and exports all 429 comparisons as JSON, CSV and Markdown.
The complete runner workflow is in [its README](../../tools/throughput-analysis/README.md).
Original totals and source/code-generation analysis remain in the
[campaign report](throughput-long.md); this arithmetic transformation does not
resolve the documented performance gaps or the unreproduced small-clone outlier.

Python tests cover call-weighted means versus seed-rate means, repeat-average
times, changed counts, outlier retention and selection of completed long attempts.
The original sampler and benchmark executables are unchanged. Runtime library
code and its completed source/guard certificate are unchanged as well.
