# macOS benchmarks for jzap-std 0.1.0-beta.1

## Conditions

| Item | Value |
| --- | --- |
| Candidate | `665c95bba6cc3f94b2c84fbacc1192f1585bd553` |
| Measurement date | 2026-11-20, Korea Standard Time |
| Machine | Apple M4, Mac16,10, 4 performance cores, 6 efficiency cores, 16 GiB RAM |
| Operating system | macOS 27.2 (26B123), native arm64 Mach-O, Rosetta translation off |
| Toolchain | Rust 1.95.0 (`59807616e`), Cargo 1.95.0, LLVM 22.1.2, Xcode 27.2 (27B52) |
| Power and scheduling | AC power, Low Power Mode off, CPU affinity unset, serial benchmark processes, `caffeinate -i` |
| Build | Cargo `+1.95.0 build --locked --release`, default release profile for each benchmark workspace |
| Collection features | `hash-map,hash-set,owned-budgets,bplus-pool`. Concurrent Arc was built separately with `--bin arc_scenarios`. |
| B+ builds | `bplus-pool` for the full, fixed and preparation runs. Separate `bplus-bench` build for the std BTreeMap comparison. |
| Allocation measurements | Separate instrumented binaries. Requested bytes and allocator usable bytes from Darwin `malloc_size`. |
| Unmeasured | CPU thermal state and concurrent Arc process RSS |

[Benchmark protocol at the measured commit](https://github.com/shim9610/jzap-std/blob/665c95bba6cc3f94b2c84fbacc1192f1585bd553/docs/macos-benchmark-prompt.md)

### Timing and repetition

- Collection seeds were 617, 6203 and 96011. Concurrent Arc also used 17291 and 70183. Each seed used fresh processes, matched inputs and operation counts, rotated or reversed run order, and an independent std A/A control. Concurrent Arc held the worker count constant within each comparison.
- Collection rows use std as the comparator, including rows with budget policies. Arithmetic and Box rows name the comparator in the condition.
- Each accepted long timing interval lasted at least 1 second. The time per operation is the sum of accepted timed seconds divided by the sum of matched operation counts. Ranges are the minimum and maximum per-run times. Delta is measured time minus comparator time as a percentage of comparator time.
- A/A is the maximum observed relative difference between the comparator and its repeat. A row is marked **Held** when A/A exceeds 10% or paired differences cross both sides of the larger of 2% of comparator time and 0.1 ns. For B+ preparation, the unit is one complete build, validation or check lookup, and drop.
- B+ used 512, 32,768 and 65,536 ascending `u64` keys, 4,096 common queries per seed with matched hits and misses, and seeds 1, 2, 3 and 10. An independent repeat of seed 1 provided the brie-tree 0.1.2 A/A control. Each comparator ran in the same process as the corresponding measured implementation. The fixed run used `BPLUS_FIXED_ONLY=1`. Preparation used `BPLUS_PREPARE=1`.
- The arithmetic inputs were 8191 and 17, with opaque, bounded and call forms. The `Checked<T>` runs used the same success inputs, maximum word value and 2 for add/multiply overflow, 0 and 1 for subtract underflow, and 8191 and 0 for division and remainder errors. Each condition used three independent long runs with a repeated std control.
- Box used Varlit at `cd7b0446da3e731fba867802bef5f4b7b3fc328d`. The long lifecycle covers creation, cloning and dropping both sets of 65,536 boxes, reported per original box. The separate ABI tables use 30 short observations per phase per process, with three processes per condition. Their A/A column uses the std control.

### B+ fixtures

| Keys | jzap fanout / height | brie-tree key slots / height | Full batches | Fixed batches | std build batches |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 512 | 8 / 2 | 16 / 2 | 72,643 | 71,220 | 70,988 |
| 32,768 | 32 / 2 | 16 / 4 | 33,351 | 30,442 | 29,243 |
| 65,536 | 16 / 3 | 16 / 5 | 28,696 | 26,922 | 21,613 |

## Results

| Item | Result |
| --- | ---: |
| Collection conditions completed | 275 / 275 |
| Accepted collection timing intervals | 4,324 |
| Collection timing candidates under 1 second | 61 |
| B+ fixture/seed groups with verified counts and checksums | 60 |
| Comparison rows | 951 |
| Rows held | 119 |
| Rust tests passed | 431 |
| Benchmark tooling tests passed | 15 |

### Allocation observations

A single Arc held a 64-byte payload.

| Policy | Requested bytes | Usable bytes | Allocation calls |
| --- | ---: | ---: | ---: |
| std | 80 | 80 | 1 |
| Unlimited | 80 | 80 | 1 |
| Borrowed | 88 | 96 | 1 |
| Owned | 120 | 128 | 2 |

The allocator control recorded 32 requested and usable bytes before and after a failed reallocation, 80 after successful reallocation, and 0 after deallocation. The 33 B+ lookup-only allocation rows recorded zero allocation calls.

### Vec and Box

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/grow/limited` | 10545.097 [8300.854, 12817.842] | 9715.686 [7559.186, 11869.933] | -7.9% | 0.5% |  |
| `mixed/grow/limited_metered` | 10545.097 [8300.854, 12817.842] | 9718.112 [7562.848, 11855.856] | -7.8% | 0.5% |  |
| `mixed/grow/metered` | 10545.097 [8300.854, 12817.842] | 9737.911 [7612.132, 11876.414] | -7.7% | 0.5% |  |
| `mixed/grow/unlimited` | 10545.097 [8300.854, 12817.842] | 9743.672 [7595.145, 11895.454] | -7.6% | 0.5% |  |
| `mixed/reserved/limited` | 9140.052 [8704.875, 10108.623] | 9328.848 [8796.440, 10532.865] | +2.1% | 0.2% |  |
| `mixed/reserved/limited_metered` | 9140.052 [8704.875, 10108.623] | 9172.909 [8744.834, 10120.761] | +0.4% | 0.2% |  |
| `mixed/reserved/metered` | 9140.052 [8704.875, 10108.623] | 9173.598 [8717.704, 10128.055] | +0.4% | 0.2% |  |
| `mixed/reserved/unlimited` | 9140.052 [8704.875, 10108.623] | 9222.376 [8748.394, 10172.130] | +0.9% | 0.2% |  |
| `mixed/copy/limited` | 2697.898 [1905.088, 4078.417] | 2723.434 [1876.751, 4175.957] | +0.9% | 2.5% |  |
| `mixed/copy/limited_metered` | 2697.898 [1905.088, 4078.417] | 2752.668 [1882.439, 4300.088] | +2.0% | 2.5% |  |
| `mixed/copy/metered` | 2697.898 [1905.088, 4078.417] | 2726.618 [1884.170, 4030.604] | +1.1% | 2.5% |  |
| `mixed/copy/unlimited` | 2697.898 [1905.088, 4078.417] | 2717.389 [1875.762, 4141.237] | +0.7% | 2.5% |  |
| `mixed/clone/limited` | 2959.741 [2800.342, 3091.775] | 2980.577 [2814.500, 3163.527] | +0.7% | 3.3% | Held |
| `mixed/clone/limited_metered` | 2959.741 [2800.342, 3091.775] | 2915.982 [2818.843, 3000.311] | -1.5% | 3.3% |  |
| `mixed/clone/metered` | 2959.741 [2800.342, 3091.775] | 2926.313 [2844.805, 3071.053] | -1.1% | 3.3% | Held |
| `mixed/clone/unlimited` | 2959.741 [2800.342, 3091.775] | 2935.312 [2782.554, 3070.805] | -0.8% | 3.3% | Held |
| `mixed/retain/limited` | 33440.107 [28485.492, 38792.668] | 37028.312 [32069.329, 41876.757] | +10.7% | 0.2% |  |
| `mixed/retain/limited_metered` | 33440.107 [28485.492, 38792.668] | 36880.118 [31645.345, 41837.496] | +10.3% | 0.2% |  |
| `mixed/retain/metered` | 33440.107 [28485.492, 38792.668] | 36586.657 [31429.635, 42056.241] | +9.4% | 0.2% |  |
| `mixed/retain/unlimited` | 33440.107 [28485.492, 38792.668] | 36240.990 [30924.271, 41359.360] | +8.4% | 0.2% |  |
| `mixed/dedup/limited` | 20721.205 [15412.697, 34304.297] | 15096.854 [10468.695, 26913.756] | -27.1% | 1.6% |  |
| `mixed/dedup/limited_metered` | 20721.205 [15412.697, 34304.297] | 16841.877 [12361.356, 28557.776] | -18.7% | 1.6% |  |
| `mixed/dedup/metered` | 20721.205 [15412.697, 34304.297] | 16884.991 [12218.804, 28678.005] | -18.5% | 1.6% |  |
| `mixed/dedup/unlimited` | 20721.205 [15412.697, 34304.297] | 15447.404 [11105.905, 26471.997] | -25.5% | 1.6% |  |
| `mixed/drain/limited` | 2201.563 [1826.594, 2583.110] | 2187.059 [1821.818, 2550.700] | -0.7% | 0.6% |  |
| `mixed/drain/limited_metered` | 2201.563 [1826.594, 2583.110] | 2201.606 [1815.554, 2582.838] | +0.0% | 0.6% |  |
| `mixed/drain/metered` | 2201.563 [1826.594, 2583.110] | 2205.510 [1824.475, 2570.144] | +0.2% | 0.6% |  |
| `mixed/drain/unlimited` | 2201.563 [1826.594, 2583.110] | 2217.562 [1811.714, 2640.642] | +0.7% | 0.6% |  |
| `mixed/extract/limited` | 12954.257 [10401.269, 15222.413] | 12432.335 [10160.783, 14777.677] | -4.0% | 4.0% |  |
| `mixed/extract/limited_metered` | 12954.257 [10401.269, 15222.413] | 12782.752 [10221.209, 15398.244] | -1.3% | 4.0% |  |
| `mixed/extract/metered` | 12954.257 [10401.269, 15222.413] | 12482.111 [10011.862, 14995.250] | -3.6% | 4.0% |  |
| `mixed/extract/unlimited` | 12954.257 [10401.269, 15222.413] | 12364.662 [9980.868, 14863.990] | -4.6% | 4.0% |  |
| `mixed/splice/limited` | 5737.273 [4031.375, 9367.492] | 5772.041 [4023.698, 9344.448] | +0.6% | 0.7% |  |
| `mixed/splice/limited_metered` | 5737.273 [4031.375, 9367.492] | 5747.327 [4013.205, 9368.475] | +0.2% | 0.7% |  |
| `mixed/splice/metered` | 5737.273 [4031.375, 9367.492] | 5759.698 [4015.142, 9355.080] | +0.4% | 0.7% |  |
| `mixed/splice/unlimited` | 5737.273 [4031.375, 9367.492] | 5773.877 [4032.750, 9337.901] | +0.6% | 0.7% |  |
| `mixed/within/limited` | 3021.070 [2152.888, 4561.818] | 2554.986 [2014.425, 3764.189] | -15.4% | 3.1% |  |
| `mixed/within/limited_metered` | 3021.070 [2152.888, 4561.818] | 2585.105 [2041.227, 3802.601] | -14.4% | 3.1% |  |
| `mixed/within/metered` | 3021.070 [2152.888, 4561.818] | 2592.895 [2044.782, 3818.566] | -14.2% | 3.1% |  |
| `mixed/within/unlimited` | 3021.070 [2152.888, 4561.818] | 2589.818 [2028.102, 3863.004] | -14.3% | 3.1% |  |
| `mixed/splice_unknown/limited` | 10389.297 [8513.653, 12142.862] | 7195.621 [5188.236, 9728.371] | -30.7% | 1.7% |  |
| `mixed/splice_unknown/limited_metered` | 10389.297 [8513.653, 12142.862] | 7185.687 [5154.586, 9778.388] | -30.8% | 1.7% |  |
| `mixed/splice_unknown/metered` | 10389.297 [8513.653, 12142.862] | 7123.054 [5147.576, 9570.239] | -31.4% | 1.7% |  |
| `mixed/splice_unknown/unlimited` | 10389.297 [8513.653, 12142.862] | 7133.172 [5172.856, 9522.864] | -31.3% | 1.7% |  |
| `mixed/splice_no_hint/limited` | 10983.224 [9456.849, 12117.149] | 7612.181 [6112.756, 9080.662] | -30.7% | 0.4% |  |
| `mixed/splice_no_hint/limited_metered` | 10983.224 [9456.849, 12117.149] | 7602.808 [6096.152, 9103.222] | -30.8% | 0.4% |  |
| `mixed/splice_no_hint/metered` | 10983.224 [9456.849, 12117.149] | 7601.270 [6065.620, 9151.408] | -30.8% | 0.4% |  |
| `mixed/splice_no_hint/unlimited` | 10983.224 [9456.849, 12117.149] | 7595.427 [6079.767, 9094.538] | -30.8% | 0.4% |  |
| `mixed/extract_none/limited` | 2649.149 [2400.036, 2901.231] | 2628.440 [2382.858, 2861.845] | -0.8% | 0.2% |  |
| `mixed/extract_none/limited_metered` | 2649.149 [2400.036, 2901.231] | 2640.948 [2397.441, 2879.988] | -0.3% | 0.2% |  |
| `mixed/extract_none/metered` | 2649.149 [2400.036, 2901.231] | 2633.779 [2385.630, 2868.394] | -0.6% | 0.2% |  |
| `mixed/extract_none/unlimited` | 2649.149 [2400.036, 2901.231] | 2622.435 [2384.105, 2864.109] | -1.0% | 0.2% |  |
| `mixed/extract_all/limited` | 7454.321 [6979.277, 7754.612] | 7441.043 [6985.299, 7693.125] | -0.2% | 0.7% |  |
| `mixed/extract_all/limited_metered` | 7454.321 [6979.277, 7754.612] | 7462.345 [7018.843, 7726.093] | +0.1% | 0.7% |  |
| `mixed/extract_all/metered` | 7454.321 [6979.277, 7754.612] | 7418.588 [6999.978, 7673.393] | -0.5% | 0.7% |  |
| `mixed/extract_all/unlimited` | 7454.321 [6979.277, 7754.612] | 7453.732 [7004.139, 7706.217] | -0.0% | 0.7% |  |
| `mixed/insert_remove/limited` | 5955.741 [5042.086, 7056.918] | 5932.857 [5023.044, 7008.033] | -0.4% | 1.5% |  |
| `mixed/insert_remove/limited_metered` | 5955.741 [5042.086, 7056.918] | 5998.271 [5076.607, 7163.044] | +0.7% | 1.5% |  |
| `mixed/insert_remove/metered` | 5955.741 [5042.086, 7056.918] | 5967.477 [5041.559, 7144.089] | +0.2% | 1.5% |  |
| `mixed/insert_remove/unlimited` | 5955.741 [5042.086, 7056.918] | 5982.151 [5073.448, 7111.907] | +0.4% | 1.5% |  |
| `mixed/resize/limited` | 4094.581 [2704.769, 6770.788] | 4008.932 [2655.663, 6518.403] | -2.1% | 2.4% |  |
| `mixed/resize/limited_metered` | 4094.581 [2704.769, 6770.788] | 4022.087 [2662.808, 6566.562] | -1.8% | 2.4% |  |
| `mixed/resize/metered` | 4094.581 [2704.769, 6770.788] | 4067.170 [2790.068, 6630.892] | -0.7% | 2.4% | Held |
| `mixed/resize/unlimited` | 4094.581 [2704.769, 6770.788] | 3960.655 [2645.588, 6478.793] | -3.3% | 2.4% |  |
| `mixed/split_off/limited` | 4355.421 [3439.987, 5195.222] | 4316.226 [3460.483, 5066.920] | -0.9% | 1.7% |  |
| `mixed/split_off/limited_metered` | 4355.421 [3439.987, 5195.222] | 4313.772 [3475.982, 5003.050] | -1.0% | 1.7% |  |
| `mixed/split_off/metered` | 4355.421 [3439.987, 5195.222] | 4339.184 [3393.084, 5276.850] | -0.4% | 1.7% |  |
| `mixed/split_off/unlimited` | 4355.421 [3439.987, 5195.222] | 4371.565 [3407.150, 5255.051] | +0.4% | 1.7% |  |
| `mixed/append/limited` | 8507.069 [7564.481, 10818.606] | 8364.859 [7552.842, 10502.631] | -1.7% | 3.5% |  |
| `mixed/append/limited_metered` | 8507.069 [7564.481, 10818.606] | 8565.530 [7627.666, 10838.410] | +0.7% | 3.5% |  |
| `mixed/append/metered` | 8507.069 [7564.481, 10818.606] | 8523.536 [7401.392, 11018.204] | +0.2% | 3.5% | Held |
| `mixed/append/unlimited` | 8507.069 [7564.481, 10818.606] | 8391.105 [7446.233, 10614.958] | -1.4% | 3.5% |  |
| `mixed/shrink/limited` | 2109.947 [1577.667, 3607.967] | 2120.398 [1638.929, 3506.525] | +0.5% | 6.5% | Held |
| `mixed/shrink/limited_metered` | 2109.947 [1577.667, 3607.967] | 2178.001 [1617.350, 3532.314] | +3.2% | 6.5% | Held |
| `mixed/shrink/metered` | 2109.947 [1577.667, 3607.967] | 2107.016 [1580.959, 3566.343] | -0.1% | 6.5% |  |
| `mixed/shrink/unlimited` | 2109.947 [1577.667, 3607.967] | 2147.597 [1644.642, 3503.340] | +1.8% | 6.5% | Held |
| `mixed/pop/limited` | 5937.160 [4166.488, 9602.197] | 5947.138 [4180.560, 9619.668] | +0.2% | 0.2% |  |
| `mixed/pop/limited_metered` | 5937.160 [4166.488, 9602.197] | 5936.743 [4174.295, 9592.117] | -0.0% | 0.2% |  |
| `mixed/pop/metered` | 5937.160 [4166.488, 9602.197] | 5932.895 [4165.217, 9584.658] | -0.1% | 0.2% |  |
| `mixed/pop/unlimited` | 5937.160 [4166.488, 9602.197] | 5933.836 [4167.402, 9594.356] | -0.1% | 0.2% |  |
| `mixed/box_u64_create_drop/metered` | 14.712 [14.414, 15.026] | 14.370 [14.363, 14.383] | -2.3% | 3.9% |  |
| `mixed/box_u64_create_drop/unlimited` | 14.712 [14.414, 15.026] | 14.546 [14.282, 15.003] | -1.1% | 3.9% |  |
| `mixed/box_u64_clone_drop/metered` | 14.547 [13.997, 15.619] | 15.214 [14.317, 16.508] | +4.6% | 7.2% | Held |
| `mixed/box_u64_clone_drop/unlimited` | 14.547 [13.997, 15.619] | 15.115 [14.605, 15.517] | +3.9% | 7.2% | Held |
| `mixed/box_u64_clone_from/metered` | 0.865 [0.865, 0.866] | 0.877 [0.877, 0.878] | +1.4% | 0.4% |  |
| `mixed/box_u64_clone_from/unlimited` | 0.865 [0.865, 0.866] | 0.865 [0.864, 0.865] | -0.0% | 0.4% |  |
| `mixed/box_slice_create_drop/metered` | 1534.736 [1514.455, 1574.891] | 1537.163 [1510.498, 1570.285] | +0.2% | 1.0% |  |
| `mixed/box_slice_create_drop/unlimited` | 1534.736 [1514.455, 1574.891] | 1531.218 [1512.475, 1562.747] | -0.2% | 1.0% |  |
| `mixed/box_slice_clone_drop/metered` | 2015.005 [1651.963, 2430.543] | 2324.332 [1962.238, 2744.640] | +15.4% | 2.6% |  |
| `mixed/box_slice_clone_drop/unlimited` | 2015.005 [1651.963, 2430.543] | 2029.655 [1683.292, 2492.115] | +0.7% | 2.6% | Held |
| `mixed/box_slice_clone_from/metered` | 1379.217 [947.629, 1841.366] | 1386.709 [940.947, 1863.151] | +0.5% | 1.8% |  |
| `mixed/box_slice_clone_from/unlimited` | 1379.217 [947.629, 1841.366] | 1375.239 [945.908, 1850.788] | -0.3% | 1.8% |  |
| `mixed/box_str_create_drop/metered` | 1978.159 [1829.569, 2100.137] | 1963.455 [1807.911, 2101.487] | -0.7% | 1.4% |  |
| `mixed/box_str_create_drop/unlimited` | 1978.159 [1829.569, 2100.137] | 1960.596 [1816.500, 2083.938] | -0.9% | 1.4% |  |
| `mixed/box_str_clone_drop/metered` | 2094.078 [1401.852, 3376.384] | 2072.433 [1384.141, 3230.844] | -1.0% | 3.1% | Held |
| `mixed/box_str_clone_drop/unlimited` | 2094.078 [1401.852, 3376.384] | 2156.094 [1389.034, 3600.821] | +3.0% | 3.1% |  |
| `mixed/box_str_clone_from/metered` | 1779.250 [1246.836, 2752.549] | 1679.780 [1177.587, 2534.051] | -5.6% | 5.6% |  |
| `mixed/box_str_clone_from/unlimited` | 1779.250 [1246.836, 2752.549] | 1697.828 [1186.772, 2572.734] | -4.6% | 5.6% |  |
| `mixed/box_zeroed_create_drop/metered` | 439.497 [257.888, 1816.278] | 454.682 [215.618, 2098.784] | +3.5% | 16.0% | Held |
| `mixed/box_zeroed_create_drop/unlimited` | 439.497 [257.888, 1816.278] | 455.365 [269.792, 1896.675] | +3.6% | 16.0% | Held |
| `mixed/box_collect_create_drop/metered` | 1825.282 [1544.408, 2496.319] | 2030.611 [1805.640, 2631.693] | +11.2% | 8.0% |  |
| `mixed/box_collect_create_drop/unlimited` | 1825.282 [1544.408, 2496.319] | 2047.232 [1786.719, 2634.719] | +12.2% | 8.0% |  |
| `small/clone/limited` | 21.501 [20.974, 22.513] | 22.675 [20.736, 23.911] | +5.5% | 3.3% |  |
| `small/clone/limited_metered` | 21.501 [20.974, 22.513] | 21.600 [20.632, 22.280] | +0.5% | 3.3% | Held |
| `small/clone/metered` | 21.501 [20.974, 22.513] | 21.296 [20.513, 22.208] | -1.0% | 3.3% |  |
| `small/clone/unlimited` | 21.501 [20.974, 22.513] | 22.654 [21.806, 23.990] | +5.4% | 3.3% |  |
| `small/extract_all/limited` | 32.161 [31.543, 32.523] | 31.641 [31.276, 32.017] | -1.6% | 2.0% |  |
| `small/extract_all/limited_metered` | 32.161 [31.543, 32.523] | 32.995 [32.177, 33.509] | +2.6% | 2.0% |  |
| `small/extract_all/metered` | 32.161 [31.543, 32.523] | 32.900 [31.951, 33.551] | +2.3% | 2.0% |  |
| `small/extract_all/unlimited` | 32.161 [31.543, 32.523] | 32.282 [31.887, 32.488] | +0.4% | 2.0% |  |
| `small/pop/limited` | 31.921 [31.309, 32.536] | 32.884 [32.381, 33.868] | +3.0% | 2.4% |  |
| `small/pop/limited_metered` | 31.921 [31.309, 32.536] | 32.306 [31.681, 33.214] | +1.2% | 2.4% | Held |
| `small/pop/metered` | 31.921 [31.309, 32.536] | 32.840 [32.349, 33.640] | +2.9% | 2.4% |  |
| `small/pop/unlimited` | 31.921 [31.309, 32.536] | 33.159 [32.699, 33.747] | +3.9% | 2.4% |  |

### String

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/string_construct/limited` | 93.029 [83.233, 102.468] | 89.493 [79.938, 102.058] | -3.8% | 2.7% |  |
| `mixed/string_construct/limited_metered` | 93.029 [83.233, 102.468] | 89.870 [78.084, 100.608] | -3.4% | 2.7% |  |
| `mixed/string_construct/metered` | 93.029 [83.233, 102.468] | 91.155 [84.065, 98.673] | -2.0% | 2.7% |  |
| `mixed/string_construct/unlimited` | 93.029 [83.233, 102.468] | 89.565 [80.320, 102.287] | -3.7% | 2.7% |  |
| `mixed/string_reserved/limited` | 66.029 [57.638, 77.487] | 65.530 [56.901, 77.301] | -0.8% | 1.0% |  |
| `mixed/string_reserved/limited_metered` | 66.029 [57.638, 77.487] | 65.653 [56.530, 77.202] | -0.6% | 1.0% |  |
| `mixed/string_reserved/metered` | 66.029 [57.638, 77.487] | 65.414 [56.843, 76.369] | -0.9% | 1.0% |  |
| `mixed/string_reserved/unlimited` | 66.029 [57.638, 77.487] | 65.209 [56.451, 76.614] | -1.2% | 1.0% |  |
| `mixed/string_clone/limited` | 88.290 [76.312, 99.992] | 86.516 [75.175, 96.585] | -2.0% | 3.7% |  |
| `mixed/string_clone/limited_metered` | 88.290 [76.312, 99.992] | 87.272 [75.331, 97.953] | -1.2% | 3.7% |  |
| `mixed/string_clone/metered` | 88.290 [76.312, 99.992] | 85.563 [74.918, 97.829] | -3.1% | 3.7% |  |
| `mixed/string_clone/unlimited` | 88.290 [76.312, 99.992] | 86.789 [76.613, 99.504] | -1.7% | 3.7% |  |
| `mixed/string_replace/limited` | 125.678 [110.282, 148.960] | 121.992 [104.650, 145.882] | -2.9% | 1.5% |  |
| `mixed/string_replace/limited_metered` | 125.678 [110.282, 148.960] | 122.518 [106.476, 146.755] | -2.5% | 1.5% |  |
| `mixed/string_replace/metered` | 125.678 [110.282, 148.960] | 122.995 [107.065, 144.953] | -2.1% | 1.5% |  |
| `mixed/string_replace/unlimited` | 125.678 [110.282, 148.960] | 122.379 [105.820, 146.795] | -2.6% | 1.5% |  |
| `mixed/string_retain/limited` | 7219.943 [5813.785, 8770.728] | 6344.387 [5347.879, 7471.396] | -12.1% | 6.5% |  |
| `mixed/string_retain/limited_metered` | 7219.943 [5813.785, 8770.728] | 5983.329 [4755.341, 7882.265] | -17.1% | 6.5% |  |
| `mixed/string_retain/metered` | 7219.943 [5813.785, 8770.728] | 5973.721 [4382.169, 7919.411] | -17.3% | 6.5% |  |
| `mixed/string_retain/unlimited` | 7219.943 [5813.785, 8770.728] | 6126.417 [4975.456, 7565.740] | -15.1% | 6.5% |  |
| `mixed/string_pop/limited` | 5087.355 [3880.714, 6347.630] | 4546.408 [3492.243, 5925.072] | -10.6% | 3.1% |  |
| `mixed/string_pop/limited_metered` | 5087.355 [3880.714, 6347.630] | 4572.628 [3441.023, 6054.221] | -10.1% | 3.1% |  |
| `mixed/string_pop/metered` | 5087.355 [3880.714, 6347.630] | 5840.099 [4700.146, 7359.177] | +14.8% | 3.1% |  |
| `mixed/string_pop/unlimited` | 5087.355 [3880.714, 6347.630] | 4621.318 [3367.143, 6052.218] | -9.2% | 3.1% |  |
| `mixed/string_format/limited` | 186.503 [165.319, 244.990] | 194.242 [171.233, 252.126] | +4.1% | 1.9% |  |
| `mixed/string_format/limited_metered` | 186.503 [165.319, 244.990] | 194.589 [173.663, 250.330] | +4.3% | 1.9% |  |
| `mixed/string_format/metered` | 186.503 [165.319, 244.990] | 191.267 [170.814, 246.042] | +2.6% | 1.9% |  |
| `mixed/string_format/unlimited` | 186.503 [165.319, 244.990] | 192.881 [171.633, 247.319] | +3.4% | 1.9% |  |
| `mixed/string_within/limited` | 204.862 [179.280, 257.222] | 206.820 [185.664, 256.291] | +1.0% | 1.4% |  |
| `mixed/string_within/limited_metered` | 204.862 [179.280, 257.222] | 207.088 [180.185, 267.003] | +1.1% | 1.4% |  |
| `mixed/string_within/metered` | 204.862 [179.280, 257.222] | 208.892 [184.122, 262.554] | +2.0% | 1.4% |  |
| `mixed/string_within/unlimited` | 204.862 [179.280, 257.222] | 206.725 [178.591, 267.949] | +0.9% | 1.4% |  |
| `mixed/string_drain/limited` | 2143.258 [1775.780, 2390.229] | 986.297 [927.483, 1031.330] | -54.0% | 11.6% | Held |
| `mixed/string_drain/limited_metered` | 2143.258 [1775.780, 2390.229] | 960.097 [835.159, 1064.964] | -55.2% | 11.6% | Held |
| `mixed/string_drain/metered` | 2143.258 [1775.780, 2390.229] | 1160.902 [993.375, 1259.267] | -45.8% | 11.6% | Held |
| `mixed/string_drain/unlimited` | 2143.258 [1775.780, 2390.229] | 974.369 [875.930, 1062.685] | -54.5% | 11.6% | Held |
| `mixed/string_utf16/limited` | 4737.039 [3582.240, 6274.511] | 5891.007 [4866.643, 7540.101] | +24.4% | 7.6% |  |
| `mixed/string_utf16/limited_metered` | 4737.039 [3582.240, 6274.511] | 7336.403 [6016.706, 9177.702] | +54.9% | 7.6% |  |
| `mixed/string_utf16/metered` | 4737.039 [3582.240, 6274.511] | 7911.174 [6528.763, 9664.381] | +67.0% | 7.6% |  |
| `mixed/string_utf16/unlimited` | 4737.039 [3582.240, 6274.511] | 6218.523 [4882.856, 7829.960] | +31.3% | 7.6% |  |
| `mixed/string_lossy/limited` | 4587.116 [3725.113, 5791.820] | 4563.696 [3767.456, 5532.071] | -0.5% | 7.2% |  |
| `mixed/string_lossy/limited_metered` | 4587.116 [3725.113, 5791.820] | 4636.293 [3625.968, 5946.467] | +1.1% | 7.2% | Held |
| `mixed/string_lossy/metered` | 4587.116 [3725.113, 5791.820] | 4573.095 [3473.817, 5832.773] | -0.3% | 7.2% | Held |
| `mixed/string_lossy/unlimited` | 4587.116 [3725.113, 5791.820] | 4611.471 [3552.461, 5828.976] | +0.5% | 7.2% | Held |

### Rc, Arc and Weak

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/rc_construct/concurrent` | 15.497 [15.471, 15.521] | 15.773 [15.410, 15.995] | +1.8% | 16.5% | Held |
| `mixed/rc_construct/local` | 15.497 [15.471, 15.521] | 17.819 [17.365, 18.801] | +15.0% | 16.5% | Held |
| `mixed/rc_construct/unlimited` | 15.497 [15.471, 15.521] | 15.684 [15.286, 16.253] | +1.2% | 16.5% | Held |
| `mixed/rc_clone/concurrent` | 0.609 [0.597, 0.623] | 0.535 [0.534, 0.535] | -12.2% | 1.6% |  |
| `mixed/rc_clone/local` | 0.609 [0.597, 0.623] | 0.537 [0.536, 0.537] | -11.8% | 1.6% |  |
| `mixed/rc_clone/unlimited` | 0.609 [0.597, 0.623] | 0.534 [0.533, 0.535] | -12.3% | 1.6% |  |
| `mixed/rc_weak_clone/concurrent` | 0.765 [0.764, 0.765] | 0.785 [0.778, 0.792] | +2.7% | 0.3% |  |
| `mixed/rc_weak_clone/local` | 0.765 [0.764, 0.765] | 0.786 [0.783, 0.790] | +2.8% | 0.3% |  |
| `mixed/rc_weak_clone/unlimited` | 0.765 [0.764, 0.765] | 0.786 [0.786, 0.788] | +2.8% | 0.3% |  |
| `mixed/rc_downgrade/concurrent` | 0.640 [0.638, 0.641] | 0.633 [0.619, 0.644] | -1.0% | 0.2% |  |
| `mixed/rc_downgrade/local` | 0.640 [0.638, 0.641] | 0.608 [0.584, 0.633] | -5.0% | 0.2% |  |
| `mixed/rc_downgrade/unlimited` | 0.640 [0.638, 0.641] | 0.640 [0.617, 0.660] | +0.1% | 0.2% |  |
| `mixed/rc_upgrade/concurrent` | 0.778 [0.772, 0.786] | 0.863 [0.853, 0.869] | +10.9% | 1.2% |  |
| `mixed/rc_upgrade/local` | 0.778 [0.772, 0.786] | 0.862 [0.854, 0.868] | +10.8% | 1.2% |  |
| `mixed/rc_upgrade/unlimited` | 0.778 [0.772, 0.786] | 0.865 [0.865, 0.866] | +11.2% | 1.2% |  |
| `mixed/rc_get_mut/concurrent` | 0.374 [0.374, 0.374] | 0.374 [0.374, 0.374] | +0.0% | 0.1% |  |
| `mixed/rc_get_mut/local` | 0.374 [0.374, 0.374] | 0.374 [0.374, 0.374] | +0.0% | 0.1% |  |
| `mixed/rc_get_mut/unlimited` | 0.374 [0.374, 0.374] | 0.374 [0.374, 0.374] | +0.1% | 0.1% |  |
| `mixed/rc_unwrap/concurrent` | 15.624 [15.592, 15.667] | 18.907 [18.854, 19.006] | +21.0% | 0.2% |  |
| `mixed/rc_unwrap/local` | 15.624 [15.592, 15.667] | 16.000 [15.827, 16.103] | +2.4% | 0.2% |  |
| `mixed/rc_unwrap/unlimited` | 15.624 [15.592, 15.667] | 15.342 [15.309, 15.386] | -1.8% | 0.2% |  |
| `mixed/rc_cow_shared/concurrent` | 18.899 [18.182, 20.012] | 18.117 [17.812, 18.542] | -4.1% | 9.5% |  |
| `mixed/rc_cow_shared/local` | 18.899 [18.182, 20.012] | 17.667 [16.809, 18.577] | -6.5% | 9.5% |  |
| `mixed/rc_cow_shared/unlimited` | 18.899 [18.182, 20.012] | 16.210 [15.853, 16.813] | -14.2% | 9.5% |  |
| `mixed/rc_cow_weak/concurrent` | 30.122 [30.031, 30.249] | 31.476 [30.234, 33.927] | +4.5% | 7.4% |  |
| `mixed/rc_cow_weak/local` | 30.122 [30.031, 30.249] | 33.056 [32.348, 34.318] | +9.7% | 7.4% |  |
| `mixed/rc_cow_weak/unlimited` | 30.122 [30.031, 30.249] | 29.224 [29.142, 29.344] | -3.0% | 7.4% |  |
| `mixed/rc_cyclic/concurrent` | 15.772 [14.746, 16.407] | 18.399 [18.352, 18.484] | +16.7% | 7.3% |  |
| `mixed/rc_cyclic/local` | 15.772 [14.746, 16.407] | 17.245 [16.999, 17.390] | +9.3% | 7.3% |  |
| `mixed/rc_cyclic/unlimited` | 15.772 [14.746, 16.407] | 16.077 [15.486, 16.636] | +1.9% | 7.3% | Held |
| `mixed/rc_slice/concurrent` | 175.698 [117.828, 246.631] | 149.249 [111.609, 239.123] | -15.1% | 7.9% | Held |
| `mixed/rc_slice/local` | 175.698 [117.828, 246.631] | 160.966 [126.384, 194.083] | -8.4% | 7.9% | Held |
| `mixed/rc_slice/unlimited` | 175.698 [117.828, 246.631] | 166.014 [117.847, 215.197] | -5.5% | 7.9% |  |
| `mixed/rc_cow_unique/concurrent` | 1.051 [1.050, 1.052] | 0.421 [0.399, 0.433] | -59.9% | 0.3% |  |
| `mixed/rc_cow_unique/local` | 1.051 [1.050, 1.052] | 0.375 [0.375, 0.375] | -64.3% | 0.3% |  |
| `mixed/rc_cow_unique/unlimited` | 1.051 [1.050, 1.052] | 0.374 [0.374, 0.374] | -64.4% | 0.3% |  |
| `mixed/rc_cyclic_infallible/concurrent` | 15.559 [14.535, 16.103] | 18.409 [18.256, 18.686] | +18.3% | 10.2% | Held |
| `mixed/rc_cyclic_infallible/local` | 15.559 [14.535, 16.103] | 17.211 [16.204, 18.133] | +10.6% | 10.2% | Held |
| `mixed/rc_cyclic_infallible/unlimited` | 15.559 [14.535, 16.103] | 15.162 [14.311, 15.745] | -2.5% | 10.2% | Held |
| `mixed/rc_slice_clone/concurrent` | 174.071 [125.218, 222.651] | 183.889 [126.748, 233.984] | +5.6% | 5.0% |  |
| `mixed/rc_slice_clone/local` | 174.071 [125.218, 222.651] | 159.090 [124.001, 191.815] | -8.6% | 5.0% | Held |
| `mixed/rc_slice_clone/unlimited` | 174.071 [125.218, 222.651] | 147.141 [117.932, 172.294] | -15.5% | 5.0% |  |
| `mixed/rc_slice_cow/concurrent` | 293.250 [249.439, 376.903] | 258.022 [221.564, 319.292] | -12.0% | 1.0% | Held |
| `mixed/rc_slice_cow/local` | 293.250 [249.439, 376.903] | 307.331 [253.767, 393.937] | +4.8% | 1.0% |  |
| `mixed/rc_slice_cow/unlimited` | 293.250 [249.439, 376.903] | 285.924 [244.411, 361.348] | -2.5% | 1.0% |  |
| `mixed/arc_construct/concurrent` | 16.291 [16.069, 16.486] | 19.998 [19.637, 20.424] | +22.8% | 6.2% |  |
| `mixed/arc_construct/local` | 16.291 [16.069, 16.486] | 17.199 [16.026, 17.823] | +5.6% | 6.2% | Held |
| `mixed/arc_construct/unlimited` | 16.291 [16.069, 16.486] | 16.169 [15.346, 17.355] | -0.7% | 6.2% | Held |
| `mixed/arc_clone/concurrent` | 1.441 [1.433, 1.448] | 1.485 [1.482, 1.487] | +3.0% | 0.7% |  |
| `mixed/arc_clone/local` | 1.441 [1.433, 1.448] | 1.487 [1.483, 1.489] | +3.2% | 0.7% |  |
| `mixed/arc_clone/unlimited` | 1.441 [1.433, 1.448] | 1.487 [1.486, 1.488] | +3.2% | 0.7% |  |
| `mixed/arc_weak_clone/concurrent` | 1.689 [1.686, 1.691] | 1.750 [1.744, 1.762] | +3.6% | 1.2% |  |
| `mixed/arc_weak_clone/local` | 1.689 [1.686, 1.691] | 1.746 [1.745, 1.747] | +3.4% | 1.2% |  |
| `mixed/arc_weak_clone/unlimited` | 1.689 [1.686, 1.691] | 1.744 [1.743, 1.744] | +3.2% | 1.2% |  |
| `mixed/arc_downgrade/concurrent` | 1.496 [1.495, 1.496] | 1.496 [1.495, 1.497] | -0.0% | 0.2% |  |
| `mixed/arc_downgrade/local` | 1.496 [1.495, 1.496] | 1.496 [1.494, 1.499] | +0.0% | 0.2% |  |
| `mixed/arc_downgrade/unlimited` | 1.496 [1.495, 1.496] | 1.496 [1.495, 1.497] | +0.0% | 0.2% |  |
| `mixed/arc_upgrade/concurrent` | 1.744 [1.743, 1.744] | 1.743 [1.743, 1.744] | -0.0% | 0.1% |  |
| `mixed/arc_upgrade/local` | 1.744 [1.743, 1.744] | 1.744 [1.744, 1.745] | +0.0% | 0.1% |  |
| `mixed/arc_upgrade/unlimited` | 1.744 [1.743, 1.744] | 1.744 [1.744, 1.744] | +0.0% | 0.1% |  |
| `mixed/arc_get_mut/concurrent` | 1.003 [1.000, 1.009] | 1.000 [0.999, 1.001] | -0.3% | 0.9% |  |
| `mixed/arc_get_mut/local` | 1.003 [1.000, 1.009] | 1.000 [0.998, 1.002] | -0.3% | 0.9% |  |
| `mixed/arc_get_mut/unlimited` | 1.003 [1.000, 1.009] | 1.001 [1.000, 1.002] | -0.2% | 0.9% |  |
| `mixed/arc_unwrap/concurrent` | 15.856 [14.192, 17.648] | 20.419 [20.335, 20.484] | +28.8% | 16.4% | Held |
| `mixed/arc_unwrap/local` | 15.856 [14.192, 17.648] | 18.723 [18.394, 19.352] | +18.1% | 16.4% | Held |
| `mixed/arc_unwrap/unlimited` | 15.856 [14.192, 17.648] | 15.729 [13.942, 16.709] | -0.8% | 16.4% | Held |
| `mixed/arc_cow_shared/concurrent` | 37.752 [37.485, 37.889] | 35.372 [35.139, 35.814] | -6.3% | 1.5% |  |
| `mixed/arc_cow_shared/local` | 37.752 [37.485, 37.889] | 34.337 [33.846, 35.150] | -9.0% | 1.5% |  |
| `mixed/arc_cow_shared/unlimited` | 37.752 [37.485, 37.889] | 33.170 [32.654, 33.434] | -12.1% | 1.5% |  |
| `mixed/arc_cow_weak/concurrent` | 33.706 [32.296, 36.029] | 35.528 [35.054, 35.960] | +5.4% | 10.1% | Held |
| `mixed/arc_cow_weak/local` | 33.706 [32.296, 36.029] | 37.660 [36.307, 40.132] | +11.7% | 10.1% | Held |
| `mixed/arc_cow_weak/unlimited` | 33.706 [32.296, 36.029] | 30.001 [29.144, 30.389] | -11.0% | 10.1% | Held |
| `mixed/arc_cyclic/concurrent` | 15.300 [14.289, 16.040] | 19.404 [16.625, 20.802] | +26.8% | 4.4% |  |
| `mixed/arc_cyclic/local` | 15.300 [14.289, 16.040] | 16.386 [16.227, 16.669] | +7.1% | 4.4% |  |
| `mixed/arc_cyclic/unlimited` | 15.300 [14.289, 16.040] | 15.328 [14.739, 16.486] | +0.2% | 4.4% | Held |
| `mixed/arc_slice/concurrent` | 182.088 [123.798, 222.677] | 187.835 [129.535, 242.409] | +3.2% | 12.1% | Held |
| `mixed/arc_slice/local` | 182.088 [123.798, 222.677] | 161.619 [125.844, 195.227] | -11.2% | 12.1% | Held |
| `mixed/arc_slice/unlimited` | 182.088 [123.798, 222.677] | 145.532 [100.148, 219.800] | -20.1% | 12.1% | Held |
| `mixed/arc_cow_unique/concurrent` | 0.994 [0.992, 0.997] | 1.015 [1.014, 1.016] | +2.1% | 0.1% |  |
| `mixed/arc_cow_unique/local` | 0.994 [0.992, 0.997] | 1.004 [1.003, 1.005] | +1.0% | 0.1% |  |
| `mixed/arc_cow_unique/unlimited` | 0.994 [0.992, 0.997] | 1.001 [1.000, 1.002] | +0.6% | 0.1% |  |
| `mixed/arc_cyclic_infallible/concurrent` | 16.879 [16.644, 17.274] | 17.016 [16.347, 17.397] | +0.8% | 2.9% | Held |
| `mixed/arc_cyclic_infallible/local` | 16.879 [16.644, 17.274] | 18.467 [18.414, 18.510] | +9.4% | 2.9% |  |
| `mixed/arc_cyclic_infallible/unlimited` | 16.879 [16.644, 17.274] | 16.365 [16.176, 16.584] | -3.1% | 2.9% |  |
| `mixed/arc_slice_clone/concurrent` | 169.400 [122.117, 214.873] | 187.683 [131.470, 240.359] | +10.8% | 3.1% |  |
| `mixed/arc_slice_clone/local` | 169.400 [122.117, 214.873] | 187.896 [131.648, 241.132] | +10.9% | 3.1% |  |
| `mixed/arc_slice_clone/unlimited` | 169.400 [122.117, 214.873] | 175.511 [123.390, 217.551] | +3.6% | 3.1% |  |
| `mixed/arc_slice_cow/concurrent` | 295.329 [240.554, 398.888] | 329.488 [276.560, 409.012] | +11.6% | 17.9% | Held |
| `mixed/arc_slice_cow/local` | 295.329 [240.554, 398.888] | 312.091 [283.544, 335.216] | +5.7% | 17.9% | Held |
| `mixed/arc_slice_cow/unlimited` | 295.329 [240.554, 398.888] | 298.086 [268.369, 326.831] | +0.9% | 17.9% | Held |

### BTreeMap

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/btree_build/concurrent` | 23447.627 [18889.722, 26917.107] | 21814.970 [17564.980, 25109.952] | -7.0% | 2.2% |  |
| `mixed/btree_build/local` | 23447.627 [18889.722, 26917.107] | 21969.823 [18019.056, 24846.867] | -6.3% | 2.2% |  |
| `mixed/btree_build/unlimited` | 23447.627 [18889.722, 26917.107] | 21729.214 [17362.862, 24893.124] | -7.3% | 2.2% |  |
| `mixed/btree_get/concurrent` | 28.858 [28.024, 29.318] | 26.870 [25.488, 28.234] | -6.9% | 1.6% |  |
| `mixed/btree_get/local` | 28.858 [28.024, 29.318] | 27.160 [26.825, 27.350] | -5.9% | 1.6% |  |
| `mixed/btree_get/unlimited` | 28.858 [28.024, 29.318] | 26.565 [25.678, 27.300] | -7.9% | 1.6% |  |
| `mixed/btree_replace/concurrent` | 29.917 [28.969, 30.791] | 29.209 [28.490, 29.605] | -2.4% | 3.7% |  |
| `mixed/btree_replace/local` | 29.917 [28.969, 30.791] | 27.544 [26.280, 28.974] | -7.9% | 3.7% |  |
| `mixed/btree_replace/unlimited` | 29.917 [28.969, 30.791] | 28.619 [27.526, 29.990] | -4.3% | 3.7% |  |
| `mixed/btree_iter/concurrent` | 688.901 [559.204, 805.608] | 741.497 [609.616, 841.530] | +7.6% | 5.6% |  |
| `mixed/btree_iter/local` | 688.901 [559.204, 805.608] | 681.855 [566.977, 770.410] | -1.0% | 5.6% |  |
| `mixed/btree_iter/unlimited` | 688.901 [559.204, 805.608] | 644.217 [528.952, 747.532] | -6.5% | 5.6% |  |
| `mixed/btree_range/concurrent` | 348.334 [307.806, 415.897] | 356.837 [308.949, 444.329] | +2.4% | 5.9% |  |
| `mixed/btree_range/local` | 348.334 [307.806, 415.897] | 328.275 [287.104, 411.032] | -5.8% | 5.9% |  |
| `mixed/btree_range/unlimited` | 348.334 [307.806, 415.897] | 384.659 [330.170, 479.300] | +10.4% | 5.9% |  |
| `mixed/btree_entry/concurrent` | 29.759 [29.049, 30.424] | 29.779 [29.253, 30.340] | +0.1% | 2.3% |  |
| `mixed/btree_entry/local` | 29.759 [29.049, 30.424] | 27.541 [26.192, 28.259] | -7.5% | 2.3% |  |
| `mixed/btree_entry/unlimited` | 29.759 [29.049, 30.424] | 27.595 [26.166, 28.392] | -7.3% | 2.3% |  |
| `mixed/btree_clone/concurrent` | 2067.174 [1688.013, 2330.643] | 1821.692 [1498.965, 2069.732] | -11.9% | 1.8% |  |
| `mixed/btree_clone/local` | 2067.174 [1688.013, 2330.643] | 1731.535 [1410.187, 1977.337] | -16.2% | 1.8% |  |
| `mixed/btree_clone/unlimited` | 2067.174 [1688.013, 2330.643] | 1707.005 [1393.891, 1934.930] | -17.4% | 1.8% |  |
| `mixed/btree_remove/concurrent` | 23329.249 [18931.379, 26825.764] | 22773.046 [18577.223, 26040.791] | -2.4% | 0.5% |  |
| `mixed/btree_remove/local` | 23329.249 [18931.379, 26825.764] | 22920.456 [18999.387, 26042.967] | -1.8% | 0.5% |  |
| `mixed/btree_remove/unlimited` | 23329.249 [18931.379, 26825.764] | 22690.955 [18341.425, 25971.887] | -2.7% | 0.5% |  |
| `mixed/btree_retain/concurrent` | 4783.354 [3925.995, 5365.801] | 4951.121 [4092.458, 5579.049] | +3.5% | 1.1% |  |
| `mixed/btree_retain/local` | 4783.354 [3925.995, 5365.801] | 4983.577 [4050.331, 5629.106] | +4.2% | 1.1% |  |
| `mixed/btree_retain/unlimited` | 4783.354 [3925.995, 5365.801] | 4792.808 [3907.666, 5508.871] | +0.2% | 1.1% | Held |
| `mixed/btree_extract/concurrent` | 4788.767 [3955.856, 5511.781] | 4961.452 [4078.834, 5596.026] | +3.6% | 2.2% |  |
| `mixed/btree_extract/local` | 4788.767 [3955.856, 5511.781] | 4850.668 [3950.255, 5506.284] | +1.3% | 2.2% |  |
| `mixed/btree_extract/unlimited` | 4788.767 [3955.856, 5511.781] | 4883.071 [3982.271, 5609.375] | +2.0% | 2.2% |  |
| `mixed/btree_split/concurrent` | 2100.475 [1740.057, 2394.117] | 1947.726 [1611.204, 2183.306] | -7.3% | 3.4% |  |
| `mixed/btree_split/local` | 2100.475 [1740.057, 2394.117] | 1897.768 [1606.836, 2116.717] | -9.7% | 3.4% |  |
| `mixed/btree_split/unlimited` | 2100.475 [1740.057, 2394.117] | 1876.395 [1552.469, 2102.587] | -10.7% | 3.4% |  |
| `mixed/btree_append/concurrent` | 7073.527 [5904.760, 7877.488] | 6272.917 [5098.905, 7064.067] | -11.3% | 1.5% |  |
| `mixed/btree_append/local` | 7073.527 [5904.760, 7877.488] | 6188.248 [5177.611, 6866.986] | -12.5% | 1.5% |  |
| `mixed/btree_append/unlimited` | 7073.527 [5904.760, 7877.488] | 6175.562 [5135.612, 6959.113] | -12.7% | 1.5% |  |
| `mixed/btree_append_disjoint/concurrent` | 8696.351 [7243.049, 9646.353] | 6947.420 [5783.812, 7750.925] | -20.1% | 0.9% |  |
| `mixed/btree_append_disjoint/local` | 8696.351 [7243.049, 9646.353] | 6765.229 [5691.417, 7493.374] | -22.2% | 0.9% |  |
| `mixed/btree_append_disjoint/unlimited` | 8696.351 [7243.049, 9646.353] | 6750.669 [5580.932, 7501.349] | -22.4% | 0.9% |  |
| `mixed/btree_pop_all/concurrent` | 9460.113 [7847.973, 10662.828] | 9029.549 [7473.057, 10080.239] | -4.6% | 0.9% |  |
| `mixed/btree_pop_all/local` | 9460.113 [7847.973, 10662.828] | 8581.481 [7102.104, 9614.940] | -9.3% | 0.9% |  |
| `mixed/btree_pop_all/unlimited` | 9460.113 [7847.973, 10662.828] | 8569.269 [7091.826, 9566.679] | -9.4% | 0.9% |  |
| `mixed/btree_into_iter/concurrent` | 2084.176 [1697.043, 2334.907] | 2256.453 [1817.436, 2538.545] | +8.3% | 1.9% |  |
| `mixed/btree_into_iter/local` | 2084.176 [1697.043, 2334.907] | 2217.799 [1806.372, 2482.090] | +6.4% | 1.9% |  |
| `mixed/btree_into_iter/unlimited` | 2084.176 [1697.043, 2334.907] | 2236.099 [1810.643, 2545.947] | +7.3% | 1.9% |  |
| `mixed/btree_remove_cycle/concurrent` | 91.572 [91.370, 91.755] | 87.335 [87.047, 87.822] | -4.6% | 0.2% |  |
| `mixed/btree_remove_cycle/local` | 91.572 [91.370, 91.755] | 87.204 [87.065, 87.325] | -4.8% | 0.2% |  |
| `mixed/btree_remove_cycle/unlimited` | 91.572 [91.370, 91.755] | 87.436 [87.113, 87.869] | -4.5% | 0.2% |  |

### BTreeSet

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/btree_set_build/concurrent` | 21188.856 [17342.377, 24411.345] | 20860.615 [17231.670, 23703.186] | -1.5% | 3.3% |  |
| `mixed/btree_set_build/local` | 21188.856 [17342.377, 24411.345] | 20756.917 [17227.571, 23589.316] | -2.0% | 3.3% |  |
| `mixed/btree_set_build/unlimited` | 21188.856 [17342.377, 24411.345] | 19947.971 [16438.405, 22807.599] | -5.9% | 3.3% |  |
| `mixed/btree_set_get/concurrent` | 30.281 [29.524, 30.950] | 29.137 [28.923, 29.304] | -3.8% | 3.7% |  |
| `mixed/btree_set_get/local` | 30.281 [29.524, 30.950] | 29.371 [29.038, 29.699] | -3.0% | 3.7% |  |
| `mixed/btree_set_get/unlimited` | 30.281 [29.524, 30.950] | 28.931 [28.752, 29.183] | -4.5% | 3.7% |  |
| `mixed/btree_set_replace/concurrent` | 27.811 [26.627, 28.563] | 29.846 [29.004, 30.648] | +7.3% | 8.0% |  |
| `mixed/btree_set_replace/local` | 27.811 [26.627, 28.563] | 28.103 [27.567, 28.708] | +1.0% | 8.0% |  |
| `mixed/btree_set_replace/unlimited` | 27.811 [26.627, 28.563] | 27.846 [26.905, 28.448] | +0.1% | 8.0% |  |
| `mixed/btree_set_iter/concurrent` | 441.566 [362.984, 498.644] | 608.495 [491.427, 694.947] | +37.8% | 1.3% |  |
| `mixed/btree_set_iter/local` | 441.566 [362.984, 498.644] | 552.829 [435.951, 633.690] | +25.2% | 1.3% |  |
| `mixed/btree_set_iter/unlimited` | 441.566 [362.984, 498.644] | 577.933 [475.217, 660.369] | +30.9% | 1.3% |  |
| `mixed/btree_set_range/concurrent` | 266.709 [236.998, 321.789] | 313.233 [283.222, 383.416] | +17.4% | 3.4% |  |
| `mixed/btree_set_range/local` | 266.709 [236.998, 321.789] | 304.831 [265.302, 386.664] | +14.3% | 3.4% |  |
| `mixed/btree_set_range/unlimited` | 266.709 [236.998, 321.789] | 343.568 [302.627, 418.097] | +28.8% | 3.4% |  |
| `mixed/btree_set_clone/concurrent` | 2112.232 [1707.316, 2381.867] | 1927.187 [1591.672, 2149.146] | -8.8% | 2.1% |  |
| `mixed/btree_set_clone/local` | 2112.232 [1707.316, 2381.867] | 1864.035 [1542.273, 2083.880] | -11.8% | 2.1% |  |
| `mixed/btree_set_clone/unlimited` | 2112.232 [1707.316, 2381.867] | 1838.630 [1505.463, 2073.071] | -13.0% | 2.1% |  |
| `mixed/btree_set_remove/concurrent` | 22020.606 [18095.907, 25168.019] | 21201.639 [17432.247, 23863.972] | -3.7% | 2.0% |  |
| `mixed/btree_set_remove/local` | 22020.606 [18095.907, 25168.019] | 20854.036 [17066.316, 23523.497] | -5.3% | 2.0% |  |
| `mixed/btree_set_remove/unlimited` | 22020.606 [18095.907, 25168.019] | 20982.024 [17551.630, 23540.878] | -4.7% | 2.0% |  |
| `mixed/btree_set_retain/concurrent` | 4000.302 [3243.449, 4589.804] | 4270.123 [3508.763, 4859.797] | +6.7% | 2.8% |  |
| `mixed/btree_set_retain/local` | 4000.302 [3243.449, 4589.804] | 4304.276 [3492.742, 4856.900] | +7.6% | 2.8% |  |
| `mixed/btree_set_retain/unlimited` | 4000.302 [3243.449, 4589.804] | 4268.131 [3449.594, 4876.570] | +6.7% | 2.8% |  |
| `mixed/btree_set_extract/concurrent` | 3929.218 [3159.345, 4493.656] | 4359.046 [3547.583, 4999.776] | +10.9% | 4.3% |  |
| `mixed/btree_set_extract/local` | 3929.218 [3159.345, 4493.656] | 4331.140 [3529.446, 4941.754] | +10.2% | 4.3% |  |
| `mixed/btree_set_extract/unlimited` | 3929.218 [3159.345, 4493.656] | 4219.200 [3432.068, 4827.281] | +7.4% | 4.3% |  |
| `mixed/btree_set_split/concurrent` | 2129.067 [1753.173, 2383.439] | 2010.484 [1646.288, 2265.256] | -5.6% | 2.6% |  |
| `mixed/btree_set_split/local` | 2129.067 [1753.173, 2383.439] | 1991.365 [1620.745, 2278.578] | -6.5% | 2.6% |  |
| `mixed/btree_set_split/unlimited` | 2129.067 [1753.173, 2383.439] | 1941.512 [1599.763, 2176.905] | -8.8% | 2.6% |  |
| `mixed/btree_set_append/concurrent` | 6426.102 [5349.382, 7169.993] | 5907.368 [4906.942, 6588.116] | -8.1% | 1.9% |  |
| `mixed/btree_set_append/local` | 6426.102 [5349.382, 7169.993] | 5830.046 [4883.395, 6486.217] | -9.3% | 1.9% |  |
| `mixed/btree_set_append/unlimited` | 6426.102 [5349.382, 7169.993] | 5611.654 [4631.791, 6278.216] | -12.7% | 1.9% |  |
| `mixed/btree_set_pop_all/concurrent` | 6556.477 [5454.712, 7333.234] | 6654.631 [5472.935, 7450.098] | +1.5% | 0.9% |  |
| `mixed/btree_set_pop_all/local` | 6556.477 [5454.712, 7333.234] | 6597.348 [5453.546, 7456.341] | +0.6% | 0.9% |  |
| `mixed/btree_set_pop_all/unlimited` | 6556.477 [5454.712, 7333.234] | 6444.814 [5329.979, 7253.011] | -1.7% | 0.9% |  |
| `mixed/btree_set_into_iter/concurrent` | 2114.172 [1747.963, 2383.237] | 2352.692 [1916.330, 2648.700] | +11.3% | 1.7% |  |
| `mixed/btree_set_into_iter/local` | 2114.172 [1747.963, 2383.237] | 2294.430 [1877.347, 2576.918] | +8.5% | 1.7% |  |
| `mixed/btree_set_into_iter/unlimited` | 2114.172 [1747.963, 2383.237] | 2249.283 [1834.200, 2550.434] | +6.4% | 1.7% |  |
| `mixed/btree_set_remove_cycle/concurrent` | 82.622 [82.275, 83.139] | 80.267 [80.084, 80.476] | -2.9% | 0.4% |  |
| `mixed/btree_set_remove_cycle/local` | 82.622 [82.275, 83.139] | 80.044 [79.805, 80.393] | -3.1% | 0.4% |  |
| `mixed/btree_set_remove_cycle/unlimited` | 82.622 [82.275, 83.139] | 80.035 [79.814, 80.194] | -3.1% | 0.4% |  |
| `mixed/btree_set_union/concurrent` | 1528.440 [1262.504, 1713.467] | 1547.150 [1285.002, 1729.957] | +1.2% | 0.4% |  |
| `mixed/btree_set_union/local` | 1528.440 [1262.504, 1713.467] | 1543.362 [1279.751, 1731.878] | +1.0% | 0.4% |  |
| `mixed/btree_set_union/unlimited` | 1528.440 [1262.504, 1713.467] | 1545.592 [1275.919, 1736.938] | +1.1% | 0.4% |  |
| `mixed/btree_set_intersection/concurrent` | 1291.342 [1069.015, 1456.175] | 1443.609 [1172.815, 1632.050] | +11.8% | 0.9% |  |
| `mixed/btree_set_intersection/local` | 1291.342 [1069.015, 1456.175] | 1432.291 [1172.462, 1617.182] | +10.9% | 0.9% |  |
| `mixed/btree_set_intersection/unlimited` | 1291.342 [1069.015, 1456.175] | 1433.185 [1187.803, 1632.422] | +11.0% | 0.9% |  |
| `mixed/btree_set_difference/concurrent` | 1343.650 [1114.376, 1500.113] | 1679.593 [1391.062, 1881.504] | +25.0% | 1.1% |  |
| `mixed/btree_set_difference/local` | 1343.650 [1114.376, 1500.113] | 1652.893 [1376.724, 1879.851] | +23.0% | 1.1% |  |
| `mixed/btree_set_difference/unlimited` | 1343.650 [1114.376, 1500.113] | 1636.014 [1339.450, 1835.182] | +21.8% | 1.1% |  |
| `mixed/btree_set_symmetric_difference/concurrent` | 1482.246 [1230.410, 1653.460] | 1508.167 [1250.487, 1693.585] | +1.7% | 0.3% |  |
| `mixed/btree_set_symmetric_difference/local` | 1482.246 [1230.410, 1653.460] | 1507.660 [1254.842, 1691.165] | +1.7% | 0.3% |  |
| `mixed/btree_set_symmetric_difference/unlimited` | 1482.246 [1230.410, 1653.460] | 1505.956 [1247.817, 1689.162] | +1.6% | 0.3% |  |
| `mixed/btree_set_subset/concurrent` | 2584.173 [2133.047, 2908.742] | 2002.896 [1670.671, 2248.410] | -22.5% | 0.3% |  |
| `mixed/btree_set_subset/local` | 2584.173 [2133.047, 2908.742] | 2038.112 [1639.997, 2331.389] | -21.1% | 0.3% |  |
| `mixed/btree_set_subset/unlimited` | 2584.173 [2133.047, 2908.742] | 2072.985 [1668.911, 2362.380] | -19.8% | 0.3% |  |
| `mixed/btree_set_intersection_skew/concurrent` | 32.626 [30.976, 33.769] | 37.543 [34.819, 39.366] | +15.1% | 1.4% |  |
| `mixed/btree_set_intersection_skew/local` | 32.626 [30.976, 33.769] | 37.849 [35.785, 39.167] | +16.0% | 1.4% |  |
| `mixed/btree_set_intersection_skew/unlimited` | 32.626 [30.976, 33.769] | 36.879 [34.573, 38.452] | +13.0% | 1.4% |  |
| `mixed/btree_set_difference_skew/concurrent` | 34.329 [32.409, 35.613] | 40.779 [38.678, 42.115] | +18.8% | 1.2% |  |
| `mixed/btree_set_difference_skew/local` | 34.329 [32.409, 35.613] | 40.100 [37.282, 42.608] | +16.8% | 1.2% |  |
| `mixed/btree_set_difference_skew/unlimited` | 34.329 [32.409, 35.613] | 40.675 [38.272, 42.175] | +18.5% | 1.2% |  |
| `mixed/btree_set_intersection_disjoint/concurrent` | 2.295 [2.195, 2.356] | 3.688 [3.605, 3.796] | +60.7% | 3.7% |  |
| `mixed/btree_set_intersection_disjoint/local` | 2.295 [2.195, 2.356] | 3.657 [3.556, 3.779] | +59.3% | 3.7% |  |
| `mixed/btree_set_intersection_disjoint/unlimited` | 2.295 [2.195, 2.356] | 3.616 [3.521, 3.728] | +57.5% | 3.7% |  |
| `mixed/btree_set_union_disjoint/concurrent` | 2237.193 [1871.003, 2530.307] | 1890.257 [1567.984, 2114.774] | -15.5% | 2.8% |  |
| `mixed/btree_set_union_disjoint/local` | 2237.193 [1871.003, 2530.307] | 1898.908 [1555.881, 2154.520] | -15.1% | 2.8% |  |
| `mixed/btree_set_union_disjoint/unlimited` | 2237.193 [1871.003, 2530.307] | 1882.033 [1549.082, 2112.221] | -15.9% | 2.8% |  |
| `mixed/btree_set_owned_union/concurrent` | 3439.758 [2860.820, 3849.468] | 3633.280 [3000.191, 4079.214] | +5.6% | 2.2% |  |
| `mixed/btree_set_owned_union/local` | 3439.758 [2860.820, 3849.468] | 3645.250 [3087.689, 4045.832] | +6.0% | 2.2% |  |
| `mixed/btree_set_owned_union/unlimited` | 3439.758 [2860.820, 3849.468] | 3601.560 [2968.218, 4102.642] | +4.7% | 2.2% |  |
| `mixed/btree_set_owned_intersection/concurrent` | 2579.247 [2168.235, 2878.354] | 2596.548 [2164.467, 2896.664] | +0.7% | 1.0% |  |
| `mixed/btree_set_owned_intersection/local` | 2579.247 [2168.235, 2878.354] | 2580.492 [2164.038, 2875.435] | +0.0% | 1.0% |  |
| `mixed/btree_set_owned_intersection/unlimited` | 2579.247 [2168.235, 2878.354] | 2545.139 [2134.688, 2861.564] | -1.3% | 1.0% |  |
| `mixed/btree_set_owned_difference/concurrent` | 1983.743 [1672.501, 2221.794] | 2130.066 [1789.943, 2378.012] | +7.4% | 3.8% |  |
| `mixed/btree_set_owned_difference/local` | 1983.743 [1672.501, 2221.794] | 2078.086 [1712.589, 2333.364] | +4.8% | 3.8% |  |
| `mixed/btree_set_owned_difference/unlimited` | 1983.743 [1672.501, 2221.794] | 2046.027 [1691.346, 2329.466] | +3.1% | 3.8% |  |
| `mixed/btree_set_owned_symmetric_difference/concurrent` | 2641.898 [2200.375, 2960.758] | 2398.630 [1993.852, 2680.997] | -9.2% | 1.1% |  |
| `mixed/btree_set_owned_symmetric_difference/local` | 2641.898 [2200.375, 2960.758] | 2427.448 [2022.606, 2722.021] | -8.1% | 1.1% |  |
| `mixed/btree_set_owned_symmetric_difference/unlimited` | 2641.898 [2200.375, 2960.758] | 2392.134 [1976.006, 2674.771] | -9.5% | 1.1% |  |

### HashMap

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/hash_map_build/concurrent` | 12794.311 [10949.491, 14335.113] | 38077.424 [31892.733, 41916.142] | +197.6% | 3.6% |  |
| `mixed/hash_map_build/local` | 12794.311 [10949.491, 14335.113] | 37826.536 [31810.798, 41575.465] | +195.7% | 3.6% |  |
| `mixed/hash_map_build/unlimited` | 12794.311 [10949.491, 14335.113] | 37877.770 [31811.161, 41713.239] | +196.1% | 3.6% |  |
| `mixed/hash_map_get/concurrent` | 4.829 [4.804, 4.845] | 6.813 [6.788, 6.862] | +41.1% | 1.1% |  |
| `mixed/hash_map_get/local` | 4.829 [4.804, 4.845] | 7.141 [6.769, 7.895] | +47.9% | 1.1% |  |
| `mixed/hash_map_get/unlimited` | 4.829 [4.804, 4.845] | 6.863 [6.822, 6.897] | +42.1% | 1.1% |  |
| `mixed/hash_map_update/concurrent` | 5.223 [5.198, 5.249] | 7.540 [7.538, 7.542] | +44.4% | 0.4% |  |
| `mixed/hash_map_update/local` | 5.223 [5.198, 5.249] | 7.522 [7.497, 7.540] | +44.0% | 0.4% |  |
| `mixed/hash_map_update/unlimited` | 5.223 [5.198, 5.249] | 7.559 [7.537, 7.578] | +44.7% | 0.4% |  |
| `mixed/hash_map_entry/concurrent` | 4.836 [4.826, 4.843] | 7.511 [7.492, 7.541] | +55.3% | 0.2% |  |
| `mixed/hash_map_entry/local` | 4.836 [4.826, 4.843] | 7.541 [7.535, 7.549] | +55.9% | 0.2% |  |
| `mixed/hash_map_entry/unlimited` | 4.836 [4.826, 4.843] | 7.501 [7.497, 7.505] | +55.1% | 0.2% |  |
| `mixed/hash_map_iter/concurrent` | 697.753 [561.705, 792.336] | 1205.639 [1010.157, 1344.791] | +72.8% | 2.4% |  |
| `mixed/hash_map_iter/local` | 697.753 [561.705, 792.336] | 1120.043 [890.839, 1318.759] | +60.5% | 2.4% |  |
| `mixed/hash_map_iter/unlimited` | 697.753 [561.705, 792.336] | 1167.795 [955.726, 1399.601] | +67.4% | 2.4% |  |
| `mixed/hash_map_clone/concurrent` | 245.306 [213.365, 266.428] | 253.285 [219.765, 279.806] | +3.3% | 2.4% |  |
| `mixed/hash_map_clone/local` | 245.306 [213.365, 266.428] | 252.603 [222.790, 277.826] | +3.0% | 2.4% |  |
| `mixed/hash_map_clone/unlimited` | 245.306 [213.365, 266.428] | 251.055 [219.425, 278.056] | +2.3% | 2.4% |  |
| `mixed/hash_map_remove_cycle/concurrent` | 12.528 [12.401, 12.758] | 56.157 [55.908, 56.495] | +348.3% | 4.2% |  |
| `mixed/hash_map_remove_cycle/local` | 12.528 [12.401, 12.758] | 56.497 [56.280, 56.707] | +351.0% | 4.2% |  |
| `mixed/hash_map_remove_cycle/unlimited` | 12.528 [12.401, 12.758] | 55.759 [54.715, 56.261] | +345.1% | 4.2% |  |
| `mixed/hash_map_retain/concurrent` | 860.626 [699.016, 981.383] | 2226.226 [1851.627, 2609.882] | +158.7% | 3.8% |  |
| `mixed/hash_map_retain/local` | 860.626 [699.016, 981.383] | 2205.577 [1736.535, 2619.176] | +156.3% | 3.8% |  |
| `mixed/hash_map_retain/unlimited` | 860.626 [699.016, 981.383] | 2286.386 [1868.784, 2597.587] | +165.7% | 3.8% |  |
| `mixed/hash_map_extract/concurrent` | 1248.883 [901.840, 1612.854] | 2265.991 [1884.112, 2620.745] | +81.4% | 17.8% | Held |
| `mixed/hash_map_extract/local` | 1248.883 [901.840, 1612.854] | 2979.840 [2366.281, 3527.288] | +138.6% | 17.8% | Held |
| `mixed/hash_map_extract/unlimited` | 1248.883 [901.840, 1612.854] | 2195.066 [1780.909, 2704.984] | +75.8% | 17.8% | Held |
| `mixed/hash_map_drain/concurrent` | 831.052 [689.176, 942.676] | 1437.311 [1176.003, 1698.864] | +73.0% | 2.4% |  |
| `mixed/hash_map_drain/local` | 831.052 [689.176, 942.676] | 1710.890 [1486.143, 1922.507] | +105.9% | 2.4% |  |
| `mixed/hash_map_drain/unlimited` | 831.052 [689.176, 942.676] | 1411.826 [1216.972, 1554.812] | +69.9% | 2.4% |  |
| `mixed/hash_map_into_iter/concurrent` | 826.095 [703.308, 914.294] | 1439.927 [1271.024, 1643.377] | +74.3% | 1.5% |  |
| `mixed/hash_map_into_iter/local` | 826.095 [703.308, 914.294] | 1434.590 [1179.717, 1612.812] | +73.7% | 1.5% |  |
| `mixed/hash_map_into_iter/unlimited` | 826.095 [703.308, 914.294] | 1347.184 [1140.716, 1525.968] | +63.1% | 1.5% |  |
| `mixed/hash_map_reserve/concurrent` | 6231.270 [5138.837, 7067.725] | 7896.744 [6569.866, 8871.331] | +26.7% | 7.3% |  |
| `mixed/hash_map_reserve/local` | 6231.270 [5138.837, 7067.725] | 7906.882 [6590.684, 8894.894] | +26.9% | 7.3% |  |
| `mixed/hash_map_reserve/unlimited` | 6231.270 [5138.837, 7067.725] | 7911.306 [6621.032, 8871.389] | +27.0% | 7.3% |  |
| `mixed/hash_map_shrink/concurrent` | 8722.397 [7203.220, 9844.371] | 23594.822 [19097.860, 26849.008] | +170.5% | 3.0% |  |
| `mixed/hash_map_shrink/local` | 8722.397 [7203.220, 9844.371] | 23447.207 [19039.062, 26638.218] | +168.8% | 3.0% |  |
| `mixed/hash_map_shrink/unlimited` | 8722.397 [7203.220, 9844.371] | 23477.307 [19021.081, 26614.334] | +169.2% | 3.0% |  |
| `mixed/hash_map_default_new/concurrent` | 1.877 [1.876, 1.878] | 1.825 [1.824, 1.826] | -2.8% | 0.2% |  |
| `mixed/hash_map_default_new/local` | 1.877 [1.876, 1.878] | 1.823 [1.821, 1.826] | -2.9% | 0.2% |  |
| `mixed/hash_map_default_new/unlimited` | 1.877 [1.876, 1.878] | 1.832 [1.829, 1.836] | -2.4% | 0.2% |  |
| `mixed/hash_map_random_state_new/concurrent` | 0.622 [0.613, 0.627] | 1.732 [1.727, 1.736] | +178.4% | 2.3% |  |
| `mixed/hash_map_random_state_new/local` | 0.622 [0.613, 0.627] | 1.731 [1.729, 1.736] | +178.3% | 2.3% |  |
| `mixed/hash_map_random_state_new/unlimited` | 0.622 [0.613, 0.627] | 1.739 [1.737, 1.741] | +179.6% | 2.3% |  |
| `mixed/hash_map_random_state_hash/concurrent` | 3.422 [3.406, 3.432] | 3.413 [3.412, 3.414] | -0.3% | 0.6% |  |
| `mixed/hash_map_random_state_hash/local` | 3.422 [3.406, 3.432] | 3.412 [3.410, 3.413] | -0.3% | 0.6% |  |
| `mixed/hash_map_random_state_hash/unlimited` | 3.422 [3.406, 3.432] | 3.412 [3.410, 3.414] | -0.3% | 0.6% |  |
| `mixed/hash_map_fixed_build/concurrent` | 10342.518 [8660.933, 11421.698] | 36338.380 [30311.035, 40516.969] | +251.3% | 1.2% |  |
| `mixed/hash_map_fixed_build/local` | 10342.518 [8660.933, 11421.698] | 36847.395 [30995.067, 40789.867] | +256.3% | 1.2% |  |
| `mixed/hash_map_fixed_build/unlimited` | 10342.518 [8660.933, 11421.698] | 36500.657 [30583.412, 40444.439] | +252.9% | 1.2% |  |
| `mixed/hash_map_fixed_get/concurrent` | 8.688 [8.317, 9.013] | 6.846 [6.831, 6.869] | -21.2% | 8.5% |  |
| `mixed/hash_map_fixed_get/local` | 8.688 [8.317, 9.013] | 6.869 [6.842, 6.900] | -20.9% | 8.5% |  |
| `mixed/hash_map_fixed_get/unlimited` | 8.688 [8.317, 9.013] | 6.908 [6.876, 6.951] | -20.5% | 8.5% |  |
| `mixed/hash_map_fixed_clone/concurrent` | 244.958 [218.649, 264.856] | 252.041 [219.902, 280.813] | +2.9% | 5.1% |  |
| `mixed/hash_map_fixed_clone/local` | 244.958 [218.649, 264.856] | 250.070 [217.576, 278.436] | +2.1% | 5.1% |  |
| `mixed/hash_map_fixed_clone/unlimited` | 244.958 [218.649, 264.856] | 251.869 [223.365, 277.441] | +2.8% | 5.1% |  |
| `mixed/hash_map_fixed_iter/concurrent` | 492.436 [379.249, 583.618] | 807.657 [705.464, 897.980] | +64.0% | 0.5% |  |
| `mixed/hash_map_fixed_iter/local` | 492.436 [379.249, 583.618] | 811.662 [713.094, 898.453] | +64.8% | 0.5% |  |
| `mixed/hash_map_fixed_iter/unlimited` | 492.436 [379.249, 583.618] | 809.930 [712.260, 900.711] | +64.5% | 0.5% |  |

### HashSet

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/hash_set_build/concurrent` | 9143.071 [7746.082, 10112.944] | 37486.875 [31581.020, 41409.592] | +310.0% | 0.3% |  |
| `mixed/hash_set_build/local` | 9143.071 [7746.082, 10112.944] | 37629.835 [31764.794, 41557.920] | +311.6% | 0.3% |  |
| `mixed/hash_set_build/unlimited` | 9143.071 [7746.082, 10112.944] | 37638.510 [31767.184, 41539.564] | +311.7% | 0.3% |  |
| `mixed/hash_set_get/concurrent` | 5.101 [5.029, 5.176] | 6.548 [6.532, 6.575] | +28.4% | 0.8% |  |
| `mixed/hash_set_get/local` | 5.101 [5.029, 5.176] | 6.522 [6.500, 6.540] | +27.9% | 0.8% |  |
| `mixed/hash_set_get/unlimited` | 5.101 [5.029, 5.176] | 6.537 [6.477, 6.621] | +28.2% | 0.8% |  |
| `mixed/hash_set_replace/concurrent` | 4.652 [4.641, 4.662] | 5.861 [5.846, 5.878] | +26.0% | 0.8% |  |
| `mixed/hash_set_replace/local` | 4.652 [4.641, 4.662] | 5.858 [5.856, 5.860] | +25.9% | 0.8% |  |
| `mixed/hash_set_replace/unlimited` | 4.652 [4.641, 4.662] | 5.860 [5.848, 5.880] | +26.0% | 0.8% |  |
| `mixed/hash_set_remove_cycle/concurrent` | 15.509 [14.912, 16.150] | 53.903 [53.517, 54.146] | +247.6% | 4.2% |  |
| `mixed/hash_set_remove_cycle/local` | 15.509 [14.912, 16.150] | 53.297 [53.171, 53.448] | +243.6% | 4.2% |  |
| `mixed/hash_set_remove_cycle/unlimited` | 15.509 [14.912, 16.150] | 54.977 [54.662, 55.410] | +254.5% | 4.2% |  |
| `mixed/hash_set_iter/concurrent` | 684.012 [559.358, 775.603] | 1028.384 [852.736, 1161.554] | +50.3% | 1.7% |  |
| `mixed/hash_set_iter/local` | 684.012 [559.358, 775.603] | 974.318 [854.087, 1132.227] | +42.4% | 1.7% |  |
| `mixed/hash_set_iter/unlimited` | 684.012 [559.358, 775.603] | 1120.893 [994.006, 1351.337] | +63.9% | 1.7% |  |
| `mixed/hash_set_clone/concurrent` | 151.624 [135.256, 161.895] | 145.937 [128.666, 160.751] | -3.8% | 2.7% |  |
| `mixed/hash_set_clone/local` | 151.624 [135.256, 161.895] | 147.082 [129.870, 160.787] | -3.0% | 2.7% |  |
| `mixed/hash_set_clone/unlimited` | 151.624 [135.256, 161.895] | 148.158 [130.232, 163.871] | -2.3% | 2.7% |  |
| `mixed/hash_set_retain/concurrent` | 735.177 [603.187, 846.844] | 2090.180 [1573.999, 2589.971] | +184.3% | 2.6% |  |
| `mixed/hash_set_retain/local` | 735.177 [603.187, 846.844] | 2033.245 [1676.733, 2352.072] | +176.6% | 2.6% |  |
| `mixed/hash_set_retain/unlimited` | 735.177 [603.187, 846.844] | 2094.040 [1675.821, 2407.273] | +184.8% | 2.6% |  |
| `mixed/hash_set_extract/concurrent` | 1107.371 [826.861, 1389.807] | 2952.718 [2440.858, 3340.587] | +166.6% | 27.6% | Held |
| `mixed/hash_set_extract/local` | 1107.371 [826.861, 1389.807] | 2837.934 [2271.638, 3308.427] | +156.3% | 27.6% | Held |
| `mixed/hash_set_extract/unlimited` | 1107.371 [826.861, 1389.807] | 2847.267 [2212.522, 3496.332] | +157.1% | 27.6% | Held |
| `mixed/hash_set_drain/concurrent` | 709.049 [580.184, 817.982] | 1386.681 [1219.091, 1540.613] | +95.6% | 1.6% |  |
| `mixed/hash_set_drain/local` | 709.049 [580.184, 817.982] | 1325.774 [1164.310, 1535.657] | +87.0% | 1.6% |  |
| `mixed/hash_set_drain/unlimited` | 709.049 [580.184, 817.982] | 1321.984 [1169.801, 1443.815] | +86.4% | 1.6% |  |
| `mixed/hash_set_into_iter/concurrent` | 675.919 [559.466, 756.875] | 1360.686 [1183.459, 1566.583] | +101.3% | 1.5% |  |
| `mixed/hash_set_into_iter/local` | 675.919 [559.466, 756.875] | 1316.321 [1017.253, 1629.150] | +94.7% | 1.5% |  |
| `mixed/hash_set_into_iter/unlimited` | 675.919 [559.466, 756.875] | 1241.089 [1000.694, 1506.955] | +83.6% | 1.5% |  |
| `mixed/hash_set_reserve/concurrent` | 2938.888 [2439.757, 3286.484] | 7614.949 [6279.653, 8529.698] | +159.1% | 0.4% |  |
| `mixed/hash_set_reserve/local` | 2938.888 [2439.757, 3286.484] | 7549.651 [6208.571, 8439.278] | +156.9% | 0.4% |  |
| `mixed/hash_set_reserve/unlimited` | 2938.888 [2439.757, 3286.484] | 7542.027 [6232.914, 8433.090] | +156.6% | 0.4% |  |
| `mixed/hash_set_shrink/concurrent` | 5781.033 [4789.288, 6529.148] | 23471.699 [19151.161, 26735.776] | +306.0% | 1.2% |  |
| `mixed/hash_set_shrink/local` | 5781.033 [4789.288, 6529.148] | 22782.780 [18398.757, 26099.818] | +294.1% | 1.2% |  |
| `mixed/hash_set_shrink/unlimited` | 5781.033 [4789.288, 6529.148] | 22913.168 [18739.086, 25976.124] | +296.4% | 1.2% |  |
| `mixed/hash_set_union/concurrent` | 3038.683 [2439.650, 3581.364] | 9143.482 [7349.242, 10599.279] | +200.9% | 16.1% | Held |
| `mixed/hash_set_union/local` | 3038.683 [2439.650, 3581.364] | 8526.051 [6743.003, 9905.354] | +180.6% | 16.1% | Held |
| `mixed/hash_set_union/unlimited` | 3038.683 [2439.650, 3581.364] | 8889.484 [7088.026, 10307.948] | +192.5% | 16.1% | Held |
| `mixed/hash_set_intersection/concurrent` | 2605.012 [1996.021, 3227.176] | 6699.397 [5574.263, 7799.443] | +157.2% | 10.9% | Held |
| `mixed/hash_set_intersection/local` | 2605.012 [1996.021, 3227.176] | 4530.349 [3469.406, 5820.174] | +73.9% | 10.9% | Held |
| `mixed/hash_set_intersection/unlimited` | 2605.012 [1996.021, 3227.176] | 4791.310 [3840.377, 5862.389] | +83.9% | 10.9% | Held |
| `mixed/hash_set_difference/concurrent` | 3613.177 [2494.795, 4619.174] | 6628.587 [5142.232, 7651.454] | +83.5% | 19.4% | Held |
| `mixed/hash_set_difference/local` | 3613.177 [2494.795, 4619.174] | 6444.361 [4673.464, 7714.938] | +78.4% | 19.4% | Held |
| `mixed/hash_set_difference/unlimited` | 3613.177 [2494.795, 4619.174] | 6463.795 [5112.966, 7337.791] | +78.9% | 19.4% | Held |
| `mixed/hash_set_symmetric_difference/concurrent` | 7779.816 [4925.688, 11652.093] | 13262.291 [10785.617, 15985.251] | +70.5% | 18.7% | Held |
| `mixed/hash_set_symmetric_difference/local` | 7779.816 [4925.688, 11652.093] | 14663.886 [11861.371, 17637.011] | +88.5% | 18.7% | Held |
| `mixed/hash_set_symmetric_difference/unlimited` | 7779.816 [4925.688, 11652.093] | 15153.343 [12770.878, 17644.920] | +94.8% | 18.7% | Held |
| `mixed/hash_set_is_subset/concurrent` | 8.928 [8.213, 9.764] | 16.037 [15.039, 17.289] | +79.6% | 7.4% |  |
| `mixed/hash_set_is_subset/local` | 8.928 [8.213, 9.764] | 15.907 [15.377, 17.136] | +78.2% | 7.4% |  |
| `mixed/hash_set_is_subset/unlimited` | 8.928 [8.213, 9.764] | 16.214 [15.567, 16.588] | +81.6% | 7.4% |  |
| `mixed/hash_set_is_disjoint/concurrent` | 9.150 [8.358, 9.833] | 16.597 [15.541, 17.725] | +81.4% | 10.5% | Held |
| `mixed/hash_set_is_disjoint/local` | 9.150 [8.358, 9.833] | 16.111 [15.803, 16.582] | +76.1% | 10.5% | Held |
| `mixed/hash_set_is_disjoint/unlimited` | 9.150 [8.358, 9.833] | 16.052 [14.818, 16.832] | +75.4% | 10.5% | Held |
| `mixed/hash_set_default_new/concurrent` | 1.879 [1.879, 1.879] | 1.845 [1.845, 1.846] | -1.8% | 0.1% |  |
| `mixed/hash_set_default_new/local` | 1.879 [1.879, 1.879] | 1.831 [1.829, 1.832] | -2.6% | 0.1% |  |
| `mixed/hash_set_default_new/unlimited` | 1.879 [1.879, 1.879] | 1.829 [1.829, 1.830] | -2.6% | 0.1% |  |
| `mixed/hash_set_owned_union/concurrent` | 11616.058 [9320.091, 13471.486] | 44350.705 [35807.000, 50506.415] | +281.8% | 0.2% |  |
| `mixed/hash_set_owned_union/local` | 11616.058 [9320.091, 13471.486] | 44460.838 [36074.936, 50540.991] | +282.8% | 0.2% |  |
| `mixed/hash_set_owned_union/unlimited` | 11616.058 [9320.091, 13471.486] | 44409.210 [35970.327, 50574.095] | +282.3% | 0.2% |  |
| `mixed/hash_set_owned_intersection/concurrent` | 8971.243 [7391.262, 10097.375] | 26565.206 [22283.411, 29599.299] | +196.1% | 0.8% |  |
| `mixed/hash_set_owned_intersection/local` | 8971.243 [7391.262, 10097.375] | 26013.598 [21565.463, 29183.806] | +190.0% | 0.8% |  |
| `mixed/hash_set_owned_intersection/unlimited` | 8971.243 [7391.262, 10097.375] | 25844.056 [21369.504, 29043.476] | +188.1% | 0.8% |  |
| `mixed/hash_set_owned_difference/concurrent` | 9194.149 [7547.188, 10395.747] | 26503.580 [21721.314, 29926.216] | +188.3% | 1.0% |  |
| `mixed/hash_set_owned_difference/local` | 9194.149 [7547.188, 10395.747] | 26490.982 [21844.732, 29856.173] | +188.1% | 1.0% |  |
| `mixed/hash_set_owned_difference/unlimited` | 9194.149 [7547.188, 10395.747] | 24442.357 [20060.853, 27756.959] | +165.8% | 1.0% |  |
| `mixed/hash_set_owned_symmetric_difference/concurrent` | 20096.373 [16391.563, 22750.476] | 55644.243 [46768.029, 61757.625] | +176.9% | 0.2% |  |
| `mixed/hash_set_owned_symmetric_difference/local` | 20096.373 [16391.563, 22750.476] | 54298.467 [44884.020, 60831.291] | +170.2% | 0.2% |  |
| `mixed/hash_set_owned_symmetric_difference/unlimited` | 20096.373 [16391.563, 22750.476] | 56113.354 [47081.740, 62451.627] | +179.2% | 0.2% |  |
| `mixed/hash_set_intersection_disjoint/concurrent` | 2367.130 [1818.029, 2771.273] | 4316.690 [3480.666, 4988.297] | +82.4% | 6.7% |  |
| `mixed/hash_set_intersection_disjoint/local` | 2367.130 [1818.029, 2771.273] | 4317.069 [3469.682, 4898.244] | +82.4% | 6.7% |  |
| `mixed/hash_set_intersection_disjoint/unlimited` | 2367.130 [1818.029, 2771.273] | 4255.960 [3430.048, 4950.700] | +79.8% | 6.7% |  |
| `mixed/hash_set_intersection_skew/concurrent` | 14.187 [13.852, 14.633] | 22.099 [21.624, 22.694] | +55.8% | 1.1% |  |
| `mixed/hash_set_intersection_skew/local` | 14.187 [13.852, 14.633] | 22.260 [21.829, 22.910] | +56.9% | 1.1% |  |
| `mixed/hash_set_intersection_skew/unlimited` | 14.187 [13.852, 14.633] | 22.272 [21.798, 22.811] | +57.0% | 1.1% |  |
| `mixed/hash_set_is_disjoint_true/concurrent` | 2211.770 [1773.536, 2555.041] | 4451.791 [3662.142, 5043.979] | +101.3% | 2.4% |  |
| `mixed/hash_set_is_disjoint_true/local` | 2211.770 [1773.536, 2555.041] | 4363.359 [3491.881, 5060.488] | +97.3% | 2.4% |  |
| `mixed/hash_set_is_disjoint_true/unlimited` | 2211.770 [1773.536, 2555.041] | 4377.591 [3469.778, 5112.729] | +97.9% | 2.4% |  |
| `mixed/hash_set_is_subset_true/concurrent` | 2387.233 [1983.728, 2665.740] | 4238.784 [3383.442, 4826.988] | +77.6% | 0.4% |  |
| `mixed/hash_set_is_subset_true/local` | 2387.233 [1983.728, 2665.740] | 4126.907 [3334.561, 4673.538] | +72.9% | 0.4% |  |
| `mixed/hash_set_is_subset_true/unlimited` | 2387.233 [1983.728, 2665.740] | 4280.917 [3553.953, 4850.698] | +79.3% | 0.4% |  |

### BinaryHeap

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/binary_heap_push_8/concurrent` | 2215.081 [1793.911, 2593.002] | 2192.634 [1772.660, 2636.188] | -1.0% | 15.5% | Held |
| `mixed/binary_heap_push_8/local` | 2215.081 [1793.911, 2593.002] | 2272.533 [1705.983, 2739.420] | +2.6% | 15.5% | Held |
| `mixed/binary_heap_push_8/unlimited` | 2215.081 [1793.911, 2593.002] | 2197.824 [1537.310, 2751.080] | -0.8% | 15.5% | Held |
| `mixed/binary_heap_push_reserved_8/concurrent` | 2119.800 [1595.834, 2531.745] | 2098.540 [1407.633, 2663.668] | -1.0% | 10.6% | Held |
| `mixed/binary_heap_push_reserved_8/local` | 2119.800 [1595.834, 2531.745] | 1971.707 [1395.966, 2427.388] | -7.0% | 10.6% | Held |
| `mixed/binary_heap_push_reserved_8/unlimited` | 2119.800 [1595.834, 2531.745] | 2054.913 [1468.387, 2510.389] | -3.1% | 10.6% | Held |
| `mixed/binary_heap_push_ascending_8/concurrent` | 1707.752 [1426.913, 1895.489] | 1709.148 [1435.378, 1899.768] | +0.1% | 1.0% |  |
| `mixed/binary_heap_push_ascending_8/local` | 1707.752 [1426.913, 1895.489] | 1693.306 [1417.977, 1888.081] | -0.8% | 1.0% |  |
| `mixed/binary_heap_push_ascending_8/unlimited` | 1707.752 [1426.913, 1895.489] | 1723.691 [1458.814, 1930.253] | +0.9% | 1.0% |  |
| `mixed/binary_heap_pop_all_8/concurrent` | 7331.761 [6052.549, 8246.931] | 7008.428 [5534.990, 8010.067] | -4.4% | 2.9% |  |
| `mixed/binary_heap_pop_all_8/local` | 7331.761 [6052.549, 8246.931] | 6951.307 [5630.518, 7949.735] | -5.2% | 2.9% |  |
| `mixed/binary_heap_pop_all_8/unlimited` | 7331.761 [6052.549, 8246.931] | 7429.118 [5967.470, 8460.215] | +1.3% | 2.9% |  |
| `mixed/binary_heap_from_vec_8/concurrent` | 510.588 [427.584, 570.588] | 433.942 [362.383, 485.268] | -15.0% | 0.3% |  |
| `mixed/binary_heap_from_vec_8/local` | 510.588 [427.584, 570.588] | 435.173 [362.262, 489.829] | -14.8% | 0.3% |  |
| `mixed/binary_heap_from_vec_8/unlimited` | 510.588 [427.584, 570.588] | 433.510 [361.246, 485.852] | -15.1% | 0.3% |  |
| `mixed/binary_heap_sort_8/concurrent` | 6930.072 [5627.373, 7846.962] | 8227.861 [6753.513, 9334.229] | +18.7% | 0.8% |  |
| `mixed/binary_heap_sort_8/local` | 6930.072 [5627.373, 7846.962] | 7967.230 [6437.928, 9103.867] | +15.0% | 0.8% |  |
| `mixed/binary_heap_sort_8/unlimited` | 6930.072 [5627.373, 7846.962] | 8152.769 [6476.744, 9377.591] | +17.6% | 0.8% |  |
| `mixed/binary_heap_append_small_8/concurrent` | 269.605 [237.996, 291.552] | 277.341 [242.517, 299.537] | +2.9% | 0.4% |  |
| `mixed/binary_heap_append_small_8/local` | 269.605 [237.996, 291.552] | 277.633 [244.253, 301.057] | +3.0% | 0.4% |  |
| `mixed/binary_heap_append_small_8/unlimited` | 269.605 [237.996, 291.552] | 268.913 [236.998, 290.303] | -0.3% | 0.4% |  |
| `mixed/binary_heap_append_large_8/concurrent` | 273.891 [242.244, 292.901] | 244.703 [220.916, 260.419] | -10.7% | 0.8% |  |
| `mixed/binary_heap_append_large_8/local` | 273.891 [242.244, 292.901] | 242.562 [221.432, 255.690] | -11.4% | 0.8% |  |
| `mixed/binary_heap_append_large_8/unlimited` | 273.891 [242.244, 292.901] | 275.190 [242.650, 298.135] | +0.5% | 0.8% |  |
| `mixed/binary_heap_retain_8/concurrent` | 522.793 [361.059, 708.951] | 472.172 [341.373, 647.728] | -9.7% | 8.8% |  |
| `mixed/binary_heap_retain_8/local` | 522.793 [361.059, 708.951] | 466.387 [336.165, 590.591] | -10.8% | 8.8% |  |
| `mixed/binary_heap_retain_8/unlimited` | 522.793 [361.059, 708.951] | 464.129 [341.707, 658.626] | -11.2% | 8.8% |  |
| `mixed/binary_heap_peek_mut_8/concurrent` | 2822.519 [2495.395, 3489.306] | 3271.244 [2816.368, 4100.525] | +15.9% | 4.8% |  |
| `mixed/binary_heap_peek_mut_8/local` | 2822.519 [2495.395, 3489.306] | 3569.256 [3113.001, 4462.257] | +26.5% | 4.8% |  |
| `mixed/binary_heap_peek_mut_8/unlimited` | 2822.519 [2495.395, 3489.306] | 3541.252 [3153.123, 4482.750] | +25.5% | 4.8% |  |
| `mixed/binary_heap_clone_8/concurrent` | 124.427 [103.521, 143.184] | 124.589 [104.056, 141.664] | +0.1% | 1.5% |  |
| `mixed/binary_heap_clone_8/local` | 124.427 [103.521, 143.184] | 124.042 [101.970, 142.232] | -0.3% | 1.5% |  |
| `mixed/binary_heap_clone_8/unlimited` | 124.427 [103.521, 143.184] | 125.230 [104.367, 144.216] | +0.6% | 1.5% |  |
| `mixed/binary_heap_iter_8/concurrent` | 41.278 [34.962, 45.648] | 41.436 [33.793, 46.877] | +0.4% | 3.0% | Held |
| `mixed/binary_heap_iter_8/local` | 41.278 [34.962, 45.648] | 41.309 [34.457, 46.140] | +0.1% | 3.0% |  |
| `mixed/binary_heap_iter_8/unlimited` | 41.278 [34.962, 45.648] | 41.052 [34.325, 45.704] | -0.5% | 3.0% |  |
| `mixed/binary_heap_into_iter_8/concurrent` | 124.745 [104.655, 141.867] | 124.761 [103.825, 141.956] | +0.0% | 0.9% |  |
| `mixed/binary_heap_into_iter_8/local` | 124.745 [104.655, 141.867] | 124.065 [102.966, 140.793] | -0.5% | 0.9% |  |
| `mixed/binary_heap_into_iter_8/unlimited` | 124.745 [104.655, 141.867] | 126.424 [106.792, 142.425] | +1.3% | 0.9% |  |
| `mixed/binary_heap_drain_8/concurrent` | 124.086 [102.103, 142.350] | 125.438 [105.203, 143.662] | +1.1% | 1.2% |  |
| `mixed/binary_heap_drain_8/local` | 124.086 [102.103, 142.350] | 126.030 [104.444, 143.892] | +1.6% | 1.2% |  |
| `mixed/binary_heap_drain_8/unlimited` | 124.086 [102.103, 142.350] | 123.969 [103.060, 141.735] | -0.1% | 1.2% |  |
| `mixed/binary_heap_extend_8/concurrent` | 715.779 [602.189, 795.511] | 693.507 [587.227, 776.833] | -3.1% | 0.5% |  |
| `mixed/binary_heap_extend_8/local` | 715.779 [602.189, 795.511] | 686.621 [573.911, 778.528] | -4.1% | 0.5% |  |
| `mixed/binary_heap_extend_8/unlimited` | 715.779 [602.189, 795.511] | 678.340 [577.113, 759.814] | -5.2% | 0.5% |  |
| `mixed/binary_heap_push_256/concurrent` | 8750.213 [7458.343, 9719.947] | 8890.641 [7557.967, 9909.993] | +1.6% | 0.2% |  |
| `mixed/binary_heap_push_256/local` | 8750.213 [7458.343, 9719.947] | 8832.130 [7517.869, 9813.925] | +0.9% | 0.2% |  |
| `mixed/binary_heap_push_256/unlimited` | 8750.213 [7458.343, 9719.947] | 8826.188 [7507.272, 9812.246] | +0.9% | 0.2% |  |
| `mixed/binary_heap_push_reserved_256/concurrent` | 7834.113 [6430.175, 8801.227] | 10856.426 [8954.148, 12217.436] | +38.6% | 0.4% |  |
| `mixed/binary_heap_push_reserved_256/local` | 7834.113 [6430.175, 8801.227] | 8038.746 [6583.468, 9037.166] | +2.6% | 0.4% |  |
| `mixed/binary_heap_push_reserved_256/unlimited` | 7834.113 [6430.175, 8801.227] | 8015.341 [6561.591, 9013.401] | +2.3% | 0.4% |  |
| `mixed/binary_heap_push_ascending_256/concurrent` | 17869.826 [15015.501, 19925.757] | 17781.586 [14953.959, 19761.274] | -0.5% | 0.7% |  |
| `mixed/binary_heap_push_ascending_256/local` | 17869.826 [15015.501, 19925.757] | 17601.465 [14776.263, 19587.166] | -1.5% | 0.7% |  |
| `mixed/binary_heap_push_ascending_256/unlimited` | 17869.826 [15015.501, 19925.757] | 17594.186 [14809.912, 19557.960] | -1.5% | 0.7% |  |
| `mixed/binary_heap_pop_all_256/concurrent` | 28511.451 [23490.128, 31852.439] | 27480.094 [22614.632, 30657.396] | -3.6% | 1.3% |  |
| `mixed/binary_heap_pop_all_256/local` | 28511.451 [23490.128, 31852.439] | 27326.932 [22662.407, 30690.769] | -4.2% | 1.3% |  |
| `mixed/binary_heap_pop_all_256/unlimited` | 28511.451 [23490.128, 31852.439] | 27288.113 [22291.920, 30586.533] | -4.3% | 1.3% |  |
| `mixed/binary_heap_from_vec_256/concurrent` | 6741.216 [5546.017, 7592.798] | 6732.776 [5506.423, 7639.639] | -0.1% | 0.9% |  |
| `mixed/binary_heap_from_vec_256/local` | 6741.216 [5546.017, 7592.798] | 6770.215 [5569.280, 7609.621] | +0.4% | 0.9% |  |
| `mixed/binary_heap_from_vec_256/unlimited` | 6741.216 [5546.017, 7592.798] | 6727.115 [5506.087, 7614.859] | -0.2% | 0.9% |  |
| `mixed/binary_heap_sort_256/concurrent` | 30302.796 [25030.557, 33939.315] | 31372.224 [25825.974, 35217.312] | +3.5% | 0.4% |  |
| `mixed/binary_heap_sort_256/local` | 30302.796 [25030.557, 33939.315] | 31264.163 [25841.910, 35212.828] | +3.2% | 0.4% |  |
| `mixed/binary_heap_sort_256/unlimited` | 30302.796 [25030.557, 33939.315] | 31290.787 [26085.954, 34899.180] | +3.3% | 0.4% |  |
| `mixed/binary_heap_append_small_256/concurrent` | 2721.639 [1887.275, 3783.823] | 2711.058 [1908.670, 3777.960] | -0.4% | 0.7% |  |
| `mixed/binary_heap_append_small_256/local` | 2721.639 [1887.275, 3783.823] | 2753.754 [1910.105, 3860.164] | +1.2% | 0.7% |  |
| `mixed/binary_heap_append_small_256/unlimited` | 2721.639 [1887.275, 3783.823] | 2738.311 [1905.844, 3865.269] | +0.6% | 0.7% |  |
| `mixed/binary_heap_append_large_256/concurrent` | 2940.924 [1909.747, 4003.896] | 2080.718 [1744.919, 2310.846] | -29.2% | 5.3% |  |
| `mixed/binary_heap_append_large_256/local` | 2940.924 [1909.747, 4003.896] | 2083.760 [1750.782, 2301.620] | -29.1% | 5.3% |  |
| `mixed/binary_heap_append_large_256/unlimited` | 2940.924 [1909.747, 4003.896] | 2870.997 [1909.153, 3802.180] | -2.4% | 5.3% |  |
| `mixed/binary_heap_retain_256/concurrent` | 3962.562 [2993.337, 5068.072] | 3957.454 [2990.720, 5068.715] | -0.1% | 0.8% |  |
| `mixed/binary_heap_retain_256/local` | 3962.562 [2993.337, 5068.072] | 3947.357 [2986.693, 5031.250] | -0.4% | 0.8% |  |
| `mixed/binary_heap_retain_256/unlimited` | 3962.562 [2993.337, 5068.072] | 3957.064 [2980.567, 5047.091] | -0.1% | 0.8% |  |
| `mixed/binary_heap_peek_mut_256/concurrent` | 19249.850 [17253.031, 22055.745] | 19560.678 [17497.832, 22478.349] | +1.6% | 0.5% |  |
| `mixed/binary_heap_peek_mut_256/local` | 19249.850 [17253.031, 22055.745] | 19561.363 [17645.783, 22319.130] | +1.6% | 0.5% |  |
| `mixed/binary_heap_peek_mut_256/unlimited` | 19249.850 [17253.031, 22055.745] | 19510.469 [17418.559, 22479.380] | +1.4% | 0.5% |  |
| `mixed/binary_heap_clone_256/concurrent` | 1858.902 [1544.993, 2075.819] | 1860.863 [1548.606, 2076.618] | +0.1% | 0.7% |  |
| `mixed/binary_heap_clone_256/local` | 1858.902 [1544.993, 2075.819] | 1861.103 [1548.257, 2074.668] | +0.1% | 0.7% |  |
| `mixed/binary_heap_clone_256/unlimited` | 1858.902 [1544.993, 2075.819] | 1858.834 [1541.024, 2080.988] | -0.0% | 0.7% |  |
| `mixed/binary_heap_iter_256/concurrent` | 274.749 [227.662, 308.489] | 279.833 [226.593, 316.118] | +1.9% | 6.6% |  |
| `mixed/binary_heap_iter_256/local` | 274.749 [227.662, 308.489] | 273.925 [226.959, 308.515] | -0.3% | 6.6% |  |
| `mixed/binary_heap_iter_256/unlimited` | 274.749 [227.662, 308.489] | 273.454 [226.511, 308.605] | -0.5% | 6.6% |  |
| `mixed/binary_heap_into_iter_256/concurrent` | 1862.957 [1544.250, 2079.143] | 1854.643 [1535.577, 2071.560] | -0.4% | 0.9% |  |
| `mixed/binary_heap_into_iter_256/local` | 1862.957 [1544.250, 2079.143] | 1910.543 [1542.535, 2244.308] | +2.6% | 0.9% |  |
| `mixed/binary_heap_into_iter_256/unlimited` | 1862.957 [1544.250, 2079.143] | 1859.569 [1538.642, 2078.584] | -0.2% | 0.9% |  |
| `mixed/binary_heap_drain_256/concurrent` | 1852.991 [1537.044, 2071.128] | 1849.787 [1530.872, 2074.731] | -0.2% | 0.6% |  |
| `mixed/binary_heap_drain_256/local` | 1852.991 [1537.044, 2071.128] | 1857.414 [1541.327, 2076.941] | +0.2% | 0.6% |  |
| `mixed/binary_heap_drain_256/unlimited` | 1852.991 [1537.044, 2071.128] | 1859.530 [1546.185, 2077.566] | +0.4% | 0.6% |  |
| `mixed/binary_heap_extend_256/concurrent` | 8249.422 [7001.824, 9218.341] | 8240.297 [6975.582, 9215.765] | -0.1% | 1.0% |  |
| `mixed/binary_heap_extend_256/local` | 8249.422 [7001.824, 9218.341] | 8243.273 [7013.047, 9121.419] | -0.1% | 1.0% |  |
| `mixed/binary_heap_extend_256/unlimited` | 8249.422 [7001.824, 9218.341] | 8259.171 [7038.304, 9203.842] | +0.1% | 1.0% |  |

### VecDeque

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/deque_push_back_8/concurrent` | 473.510 [403.632, 522.225] | 486.669 [416.440, 537.077] | +2.8% | 1.5% |  |
| `mixed/deque_push_back_8/local` | 473.510 [403.632, 522.225] | 488.716 [417.436, 543.329] | +3.2% | 1.5% |  |
| `mixed/deque_push_back_8/unlimited` | 473.510 [403.632, 522.225] | 483.646 [412.167, 534.904] | +2.1% | 1.5% |  |
| `mixed/deque_push_front_8/concurrent` | 606.170 [516.354, 669.213] | 630.209 [533.962, 699.483] | +4.0% | 0.3% |  |
| `mixed/deque_push_front_8/local` | 606.170 [516.354, 669.213] | 633.369 [535.139, 705.748] | +4.5% | 0.3% |  |
| `mixed/deque_push_front_8/unlimited` | 606.170 [516.354, 669.213] | 636.314 [542.713, 704.963] | +5.0% | 0.3% |  |
| `mixed/deque_fifo_8/concurrent` | 675.837 [563.399, 755.000] | 665.386 [554.350, 744.671] | -1.5% | 0.4% |  |
| `mixed/deque_fifo_8/local` | 675.837 [563.399, 755.000] | 661.790 [551.707, 739.209] | -2.1% | 0.4% |  |
| `mixed/deque_fifo_8/unlimited` | 675.837 [563.399, 755.000] | 679.014 [566.069, 761.371] | +0.5% | 0.4% |  |
| `mixed/deque_pop_all_8/concurrent` | 282.711 [235.394, 319.767] | 278.103 [232.350, 312.401] | -1.6% | 0.8% |  |
| `mixed/deque_pop_all_8/local` | 282.711 [235.394, 319.767] | 280.929 [232.872, 317.777] | -0.6% | 0.8% |  |
| `mixed/deque_pop_all_8/unlimited` | 282.711 [235.394, 319.767] | 279.778 [232.688, 315.419] | -1.0% | 0.8% |  |
| `mixed/deque_iter_8/concurrent` | 44.952 [36.719, 50.979] | 43.965 [35.532, 50.309] | -2.2% | 0.6% |  |
| `mixed/deque_iter_8/local` | 44.952 [36.719, 50.979] | 44.026 [35.757, 50.315] | -2.1% | 0.6% |  |
| `mixed/deque_iter_8/unlimited` | 44.952 [36.719, 50.979] | 43.834 [35.514, 49.733] | -2.5% | 0.6% |  |
| `mixed/deque_index_8/concurrent` | 137.445 [114.399, 153.624] | 137.274 [114.245, 153.513] | -0.1% | 0.7% |  |
| `mixed/deque_index_8/local` | 137.445 [114.399, 153.624] | 137.667 [114.722, 153.714] | +0.2% | 0.7% |  |
| `mixed/deque_index_8/unlimited` | 137.445 [114.399, 153.624] | 137.719 [114.343, 154.797] | +0.2% | 0.7% |  |
| `mixed/deque_insert_remove_8/concurrent` | 340.453 [295.729, 368.111] | 336.694 [291.654, 366.442] | -1.1% | 3.0% |  |
| `mixed/deque_insert_remove_8/local` | 340.453 [295.729, 368.111] | 336.947 [292.646, 364.962] | -1.0% | 3.0% |  |
| `mixed/deque_insert_remove_8/unlimited` | 340.453 [295.729, 368.111] | 338.048 [294.318, 365.361] | -0.7% | 3.0% |  |
| `mixed/deque_rotate_8/concurrent` | 142.210 [120.235, 156.642] | 140.601 [119.859, 155.137] | -1.1% | 0.2% |  |
| `mixed/deque_rotate_8/local` | 142.210 [120.235, 156.642] | 142.469 [118.975, 159.815] | +0.2% | 0.2% |  |
| `mixed/deque_rotate_8/unlimited` | 142.210 [120.235, 156.642] | 139.214 [116.764, 155.065] | -2.1% | 0.2% |  |
| `mixed/deque_contiguous_8/concurrent` | 354.377 [325.100, 385.157] | 351.875 [323.414, 381.243] | -0.7% | 0.3% |  |
| `mixed/deque_contiguous_8/local` | 354.377 [325.100, 385.157] | 354.385 [326.097, 384.679] | +0.0% | 0.3% |  |
| `mixed/deque_contiguous_8/unlimited` | 354.377 [325.100, 385.157] | 353.128 [323.371, 384.379] | -0.4% | 0.3% |  |
| `mixed/deque_shrink_8/concurrent` | 409.689 [338.916, 457.642] | 420.199 [347.912, 470.303] | +2.6% | 0.3% |  |
| `mixed/deque_shrink_8/local` | 409.689 [338.916, 457.642] | 420.855 [349.049, 469.621] | +2.7% | 0.3% |  |
| `mixed/deque_shrink_8/unlimited` | 409.689 [338.916, 457.642] | 419.573 [346.734, 471.434] | +2.4% | 0.3% |  |
| `mixed/deque_append_8/concurrent` | 316.576 [280.952, 340.042] | 312.608 [275.247, 336.138] | -1.3% | 1.5% |  |
| `mixed/deque_append_8/local` | 316.576 [280.952, 340.042] | 309.786 [272.087, 333.080] | -2.1% | 1.5% |  |
| `mixed/deque_append_8/unlimited` | 316.576 [280.952, 340.042] | 308.196 [270.503, 335.509] | -2.6% | 1.5% |  |
| `mixed/deque_split_8/concurrent` | 202.726 [174.370, 223.225] | 203.032 [175.810, 225.610] | +0.2% | 0.9% |  |
| `mixed/deque_split_8/local` | 202.726 [174.370, 223.225] | 204.952 [175.179, 227.844] | +1.1% | 0.9% |  |
| `mixed/deque_split_8/unlimited` | 202.726 [174.370, 223.225] | 198.286 [166.895, 220.221] | -2.2% | 0.9% |  |
| `mixed/deque_clone_8/concurrent` | 135.255 [112.901, 153.030] | 132.923 [113.044, 147.933] | -1.7% | 1.2% |  |
| `mixed/deque_clone_8/local` | 135.255 [112.901, 153.030] | 131.041 [109.799, 145.534] | -3.1% | 1.2% |  |
| `mixed/deque_clone_8/unlimited` | 135.255 [112.901, 153.030] | 128.850 [107.008, 144.617] | -4.7% | 1.2% |  |
| `mixed/deque_retain_8/concurrent` | 479.513 [367.433, 603.418] | 471.220 [372.025, 562.350] | -1.7% | 8.5% |  |
| `mixed/deque_retain_8/local` | 479.513 [367.433, 603.418] | 471.259 [376.568, 559.129] | -1.7% | 8.5% |  |
| `mixed/deque_retain_8/unlimited` | 479.513 [367.433, 603.418] | 471.342 [374.622, 559.964] | -1.7% | 8.5% |  |
| `mixed/deque_drain_8/concurrent` | 206.707 [174.269, 230.127] | 206.607 [173.077, 232.213] | -0.0% | 1.2% |  |
| `mixed/deque_drain_8/local` | 206.707 [174.269, 230.127] | 208.733 [176.123, 232.545] | +1.0% | 1.2% |  |
| `mixed/deque_drain_8/unlimited` | 206.707 [174.269, 230.127] | 207.163 [173.220, 231.851] | +0.2% | 1.2% |  |
| `mixed/deque_resize_8/concurrent` | 391.810 [346.468, 423.442] | 389.020 [336.078, 428.376] | -0.7% | 2.4% |  |
| `mixed/deque_resize_8/local` | 391.810 [346.468, 423.442] | 385.744 [338.016, 416.915] | -1.5% | 2.4% |  |
| `mixed/deque_resize_8/unlimited` | 391.810 [346.468, 423.442] | 385.134 [335.998, 423.999] | -1.7% | 2.4% |  |
| `mixed/deque_extend_8/concurrent` | 131.738 [108.338, 149.396] | 134.281 [114.039, 151.383] | +1.9% | 1.4% |  |
| `mixed/deque_extend_8/local` | 131.738 [108.338, 149.396] | 135.402 [113.522, 152.921] | +2.8% | 1.4% |  |
| `mixed/deque_extend_8/unlimited` | 131.738 [108.338, 149.396] | 135.457 [112.164, 155.082] | +2.8% | 1.4% |  |
| `mixed/deque_clone_from_8/concurrent` | 117.452 [97.964, 132.115] | 111.346 [92.233, 125.467] | -5.2% | 0.5% |  |
| `mixed/deque_clone_from_8/local` | 117.452 [97.964, 132.115] | 111.376 [93.561, 124.715] | -5.2% | 0.5% |  |
| `mixed/deque_clone_from_8/unlimited` | 117.452 [97.964, 132.115] | 111.616 [92.582, 126.310] | -5.0% | 0.5% |  |
| `mixed/deque_push_back_256/concurrent` | 4871.321 [4085.452, 5428.782] | 4964.968 [4164.303, 5527.744] | +1.9% | 0.5% |  |
| `mixed/deque_push_back_256/local` | 4871.321 [4085.452, 5428.782] | 4937.731 [4138.252, 5496.224] | +1.4% | 0.5% |  |
| `mixed/deque_push_back_256/unlimited` | 4871.321 [4085.452, 5428.782] | 4940.333 [4122.873, 5522.611] | +1.4% | 0.5% |  |
| `mixed/deque_push_front_256/concurrent` | 5745.425 [4800.906, 6467.611] | 5685.639 [4758.778, 6399.220] | -1.0% | 0.4% |  |
| `mixed/deque_push_front_256/local` | 5745.425 [4800.906, 6467.611] | 5670.364 [4736.410, 6383.970] | -1.3% | 0.4% |  |
| `mixed/deque_push_front_256/unlimited` | 5745.425 [4800.906, 6467.611] | 5658.951 [4739.406, 6358.847] | -1.5% | 0.4% |  |
| `mixed/deque_fifo_256/concurrent` | 8425.041 [6970.594, 9451.736] | 7777.688 [6454.188, 8690.310] | -7.7% | 0.1% |  |
| `mixed/deque_fifo_256/local` | 8425.041 [6970.594, 9451.736] | 7779.962 [6459.540, 8690.822] | -7.7% | 0.1% |  |
| `mixed/deque_fifo_256/unlimited` | 8425.041 [6970.594, 9451.736] | 8155.724 [6760.829, 9119.738] | -3.2% | 0.1% |  |
| `mixed/deque_pop_all_256/concurrent` | 4101.287 [3398.607, 4586.555] | 3420.093 [2838.928, 3815.669] | -16.6% | 0.2% |  |
| `mixed/deque_pop_all_256/local` | 4101.287 [3398.607, 4586.555] | 3424.294 [2844.537, 3813.191] | -16.5% | 0.2% |  |
| `mixed/deque_pop_all_256/unlimited` | 4101.287 [3398.607, 4586.555] | 3416.541 [2840.650, 3805.136] | -16.7% | 0.2% |  |
| `mixed/deque_iter_256/concurrent` | 276.452 [228.394, 313.378] | 275.796 [228.733, 311.539] | -0.2% | 0.7% |  |
| `mixed/deque_iter_256/local` | 276.452 [228.394, 313.378] | 275.211 [228.468, 308.190] | -0.4% | 0.7% |  |
| `mixed/deque_iter_256/unlimited` | 276.452 [228.394, 313.378] | 280.110 [228.616, 317.274] | +1.3% | 0.7% |  |
| `mixed/deque_index_256/concurrent` | 277.596 [229.622, 312.906] | 277.318 [227.893, 315.775] | -0.1% | 0.7% |  |
| `mixed/deque_index_256/local` | 277.596 [229.622, 312.906] | 274.939 [229.144, 305.986] | -1.0% | 0.7% |  |
| `mixed/deque_index_256/unlimited` | 277.596 [229.622, 312.906] | 278.518 [229.168, 317.505] | +0.3% | 0.7% |  |
| `mixed/deque_insert_remove_256/concurrent` | 5039.963 [4195.820, 5614.953] | 4428.655 [3707.253, 4928.386] | -12.1% | 0.2% |  |
| `mixed/deque_insert_remove_256/local` | 5039.963 [4195.820, 5614.953] | 4425.662 [3708.775, 4915.177] | -12.2% | 0.2% |  |
| `mixed/deque_insert_remove_256/unlimited` | 5039.963 [4195.820, 5614.953] | 4420.237 [3706.632, 4915.477] | -12.3% | 0.2% |  |
| `mixed/deque_rotate_256/concurrent` | 2712.864 [2259.554, 3022.168] | 1882.273 [1565.678, 2101.768] | -30.6% | 0.2% |  |
| `mixed/deque_rotate_256/local` | 2712.864 [2259.554, 3022.168] | 1873.358 [1557.866, 2099.332] | -30.9% | 0.2% |  |
| `mixed/deque_rotate_256/unlimited` | 2712.864 [2259.554, 3022.168] | 1879.636 [1562.364, 2100.756] | -30.7% | 0.2% |  |
| `mixed/deque_contiguous_256/concurrent` | 6968.755 [5807.244, 7747.083] | 6305.742 [5258.302, 7019.289] | -9.5% | 0.4% |  |
| `mixed/deque_contiguous_256/local` | 6968.755 [5807.244, 7747.083] | 6294.924 [5226.528, 7013.245] | -9.7% | 0.4% |  |
| `mixed/deque_contiguous_256/unlimited` | 6968.755 [5807.244, 7747.083] | 6294.434 [5243.904, 7001.279] | -9.7% | 0.4% |  |
| `mixed/deque_shrink_256/concurrent` | 4823.195 [3999.310, 5386.207] | 5020.116 [4142.536, 5672.191] | +4.1% | 0.3% |  |
| `mixed/deque_shrink_256/local` | 4823.195 [3999.310, 5386.207] | 5000.452 [4134.404, 5637.700] | +3.7% | 0.3% |  |
| `mixed/deque_shrink_256/unlimited` | 4823.195 [3999.310, 5386.207] | 4986.836 [4112.935, 5621.871] | +3.4% | 0.3% |  |
| `mixed/deque_append_256/concurrent` | 3578.728 [3010.539, 3964.009] | 2838.263 [2376.977, 3172.115] | -20.7% | 0.3% |  |
| `mixed/deque_append_256/local` | 3578.728 [3010.539, 3964.009] | 2842.989 [2374.354, 3168.729] | -20.6% | 0.3% |  |
| `mixed/deque_append_256/unlimited` | 3578.728 [3010.539, 3964.009] | 2833.120 [2374.170, 3156.971] | -20.8% | 0.3% |  |
| `mixed/deque_split_256/concurrent` | 3681.373 [3063.874, 4092.376] | 2993.589 [2520.611, 3341.787] | -18.7% | 0.7% |  |
| `mixed/deque_split_256/local` | 3681.373 [3063.874, 4092.376] | 3004.954 [2527.787, 3353.845] | -18.4% | 0.7% |  |
| `mixed/deque_split_256/unlimited` | 3681.373 [3063.874, 4092.376] | 3010.040 [2547.196, 3360.610] | -18.2% | 0.7% |  |
| `mixed/deque_clone_256/concurrent` | 2714.301 [2252.815, 3043.016] | 1871.013 [1555.356, 2094.169] | -31.1% | 0.5% |  |
| `mixed/deque_clone_256/local` | 2714.301 [2252.815, 3043.016] | 1874.118 [1558.911, 2095.019] | -31.0% | 0.5% |  |
| `mixed/deque_clone_256/unlimited` | 2714.301 [2252.815, 3043.016] | 1875.888 [1558.449, 2098.842] | -30.9% | 0.5% |  |
| `mixed/deque_retain_256/concurrent` | 4556.014 [3551.305, 5538.211] | 2956.315 [2325.856, 3519.204] | -35.1% | 0.2% |  |
| `mixed/deque_retain_256/local` | 4556.014 [3551.305, 5538.211] | 2946.872 [2314.174, 3518.025] | -35.3% | 0.2% |  |
| `mixed/deque_retain_256/unlimited` | 4556.014 [3551.305, 5538.211] | 2954.384 [2321.187, 3526.798] | -35.2% | 0.2% |  |
| `mixed/deque_drain_256/concurrent` | 3205.115 [2657.843, 3603.693] | 2410.059 [2014.654, 2693.404] | -24.8% | 0.2% |  |
| `mixed/deque_drain_256/local` | 3205.115 [2657.843, 3603.693] | 2402.016 [2008.020, 2668.117] | -25.1% | 0.2% |  |
| `mixed/deque_drain_256/unlimited` | 3205.115 [2657.843, 3603.693] | 2403.698 [1998.472, 2687.614] | -25.0% | 0.2% |  |
| `mixed/deque_resize_256/concurrent` | 5884.438 [4908.330, 6558.630] | 5150.954 [4293.699, 5727.380] | -12.5% | 0.3% |  |
| `mixed/deque_resize_256/local` | 5884.438 [4908.330, 6558.630] | 5154.698 [4298.665, 5768.463] | -12.4% | 0.3% |  |
| `mixed/deque_resize_256/unlimited` | 5884.438 [4908.330, 6558.630] | 4849.706 [4057.618, 5410.130] | -17.6% | 0.3% |  |
| `mixed/deque_extend_256/concurrent` | 1887.579 [1555.143, 2116.477] | 1874.118 [1545.491, 2136.477] | -0.7% | 1.8% | Held |
| `mixed/deque_extend_256/local` | 1887.579 [1555.143, 2116.477] | 1872.565 [1554.707, 2088.562] | -0.8% | 1.8% |  |
| `mixed/deque_extend_256/unlimited` | 1887.579 [1555.143, 2116.477] | 1892.769 [1551.364, 2150.581] | +0.3% | 1.8% |  |
| `mixed/deque_clone_from_256/concurrent` | 2808.949 [2299.403, 3163.851] | 2008.577 [1664.449, 2271.543] | -28.5% | 0.6% |  |
| `mixed/deque_clone_from_256/local` | 2808.949 [2299.403, 3163.851] | 2007.593 [1634.952, 2313.092] | -28.5% | 0.6% |  |
| `mixed/deque_clone_from_256/unlimited` | 2808.949 [2299.403, 3163.851] | 2001.911 [1632.173, 2273.966] | -28.7% | 0.6% |  |

### Scratch and checked slices

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/scratch_push_pop_8/concurrent` | 559.945 [458.793, 629.020] | 530.193 [451.516, 597.754] | -5.3% | 1.3% |  |
| `mixed/scratch_push_pop_8/local` | 559.945 [458.793, 629.020] | 537.911 [455.714, 605.501] | -3.9% | 1.3% |  |
| `mixed/scratch_push_pop_8/unlimited` | 559.945 [458.793, 629.020] | 528.592 [445.254, 595.969] | -5.6% | 1.3% |  |
| `mixed/scratch_extend_8/concurrent` | 58.101 [47.849, 65.326] | 59.216 [49.108, 66.948] | +1.9% | 0.3% |  |
| `mixed/scratch_extend_8/local` | 58.101 [47.849, 65.326] | 61.395 [55.236, 65.932] | +5.7% | 0.3% |  |
| `mixed/scratch_extend_8/unlimited` | 58.101 [47.849, 65.326] | 58.861 [48.828, 65.978] | +1.3% | 0.3% |  |
| `mixed/scratch_resize_8/concurrent` | 43.088 [37.145, 48.231] | 44.400 [37.548, 50.456] | +3.0% | 3.4% |  |
| `mixed/scratch_resize_8/local` | 43.088 [37.145, 48.231] | 43.419 [36.161, 51.438] | +0.8% | 3.4% | Held |
| `mixed/scratch_resize_8/unlimited` | 43.088 [37.145, 48.231] | 44.172 [39.621, 49.311] | +2.5% | 3.4% |  |
| `mixed/scratch_init_8/concurrent` | 43.299 [37.570, 48.466] | 42.883 [36.697, 48.293] | -1.0% | 1.2% |  |
| `mixed/scratch_init_8/local` | 43.299 [37.570, 48.466] | 43.151 [35.779, 48.961] | -0.3% | 1.2% | Held |
| `mixed/scratch_init_8/unlimited` | 43.299 [37.570, 48.466] | 43.591 [36.667, 50.196] | +0.7% | 1.2% | Held |
| `mixed/scratch_clone_from_8/concurrent` | 60.189 [52.713, 65.635] | 58.520 [47.885, 66.948] | -2.8% | 6.7% | Held |
| `mixed/scratch_clone_from_8/local` | 60.189 [52.713, 65.635] | 58.401 [48.109, 66.497] | -3.0% | 6.7% |  |
| `mixed/scratch_clone_from_8/unlimited` | 60.189 [52.713, 65.635] | 58.869 [49.077, 66.185] | -2.2% | 6.7% |  |
| `mixed/scratch_borrowed_push_pop_8/unlimited` | 545.619 [437.009, 618.659] | 520.066 [434.944, 593.160] | -4.7% | 0.8% |  |
| `mixed/scratch_borrowed_extend_8/unlimited` | 58.985 [48.959, 67.129] | 59.467 [48.863, 68.079] | +0.8% | 0.9% |  |
| `mixed/scratch_get_8/unlimited` | 147.759 [123.871, 164.267] | 155.156 [130.354, 173.481] | +5.0% | 0.5% |  |
| `mixed/scratch_range_8/unlimited` | 1.049 [1.047, 1.050] | 1.046 [1.045, 1.047] | -0.2% | 0.2% |  |
| `mixed/scratch_copy_within_8/unlimited` | 32.395 [24.503, 39.166] | 31.862 [23.898, 38.319] | -1.6% | 1.9% |  |
| `mixed/scratch_push_pop_256/concurrent` | 4181.859 [3467.584, 4697.097] | 2902.454 [2363.360, 3276.771] | -30.6% | 0.8% |  |
| `mixed/scratch_push_pop_256/local` | 4181.859 [3467.584, 4697.097] | 2859.689 [2343.277, 3235.833] | -31.6% | 0.8% |  |
| `mixed/scratch_push_pop_256/unlimited` | 4181.859 [3467.584, 4697.097] | 2908.391 [2358.518, 3313.058] | -30.5% | 0.8% |  |
| `mixed/scratch_extend_256/concurrent` | 1896.867 [1469.384, 2261.760] | 1802.067 [1461.522, 2067.983] | -5.0% | 11.5% | Held |
| `mixed/scratch_extend_256/local` | 1896.867 [1469.384, 2261.760] | 1779.606 [1460.839, 2025.915] | -6.2% | 11.5% | Held |
| `mixed/scratch_extend_256/unlimited` | 1896.867 [1469.384, 2261.760] | 1815.151 [1481.043, 2086.032] | -4.3% | 11.5% | Held |
| `mixed/scratch_resize_256/concurrent` | 1819.421 [1503.941, 2038.536] | 1622.013 [1333.505, 1837.874] | -10.9% | 0.2% |  |
| `mixed/scratch_resize_256/local` | 1819.421 [1503.941, 2038.536] | 1624.826 [1357.127, 1808.905] | -10.7% | 0.2% |  |
| `mixed/scratch_resize_256/unlimited` | 1819.421 [1503.941, 2038.536] | 1616.683 [1334.467, 1825.035] | -11.1% | 0.2% |  |
| `mixed/scratch_init_256/concurrent` | 1822.892 [1505.036, 2040.998] | 1816.957 [1500.369, 2037.530] | -0.3% | 1.1% |  |
| `mixed/scratch_init_256/local` | 1822.892 [1505.036, 2040.998] | 1811.861 [1502.122, 2032.172] | -0.6% | 1.1% |  |
| `mixed/scratch_init_256/unlimited` | 1822.892 [1505.036, 2040.998] | 1821.314 [1510.936, 2047.546] | -0.1% | 1.1% |  |
| `mixed/scratch_clone_from_256/concurrent` | 1793.227 [1471.066, 2017.211] | 1783.025 [1462.180, 2005.827] | -0.6% | 1.7% |  |
| `mixed/scratch_clone_from_256/local` | 1793.227 [1471.066, 2017.211] | 1779.945 [1474.475, 1998.884] | -0.7% | 1.7% |  |
| `mixed/scratch_clone_from_256/unlimited` | 1793.227 [1471.066, 2017.211] | 1801.457 [1460.252, 2061.362] | +0.5% | 1.7% |  |
| `mixed/scratch_borrowed_push_pop_256/unlimited` | 4174.411 [3437.557, 4705.930] | 4187.604 [3457.137, 4716.771] | +0.3% | 0.7% |  |
| `mixed/scratch_borrowed_extend_256/unlimited` | 1862.952 [1490.693, 2252.782] | 1802.711 [1468.145, 2054.278] | -3.2% | 6.4% |  |
| `mixed/scratch_get_256/unlimited` | 280.388 [232.569, 315.096] | 282.444 [235.604, 316.597] | +0.7% | 7.1% |  |
| `mixed/scratch_range_256/unlimited` | 1.109 [1.105, 1.112] | 1.066 [1.062, 1.069] | -3.9% | 0.5% |  |
| `mixed/scratch_copy_within_256/unlimited` | 870.446 [717.207, 978.304] | 880.715 [719.554, 1004.757] | +1.2% | 4.1% |  |

### Collection scenarios

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/scenario_wordcount/concurrent` | 16015.373 [12186.984, 22362.976] | 24550.780 [18796.146, 34375.334] | +53.3% | 0.7% |  |
| `mixed/scenario_wordcount/local` | 16015.373 [12186.984, 22362.976] | 24434.268 [18625.042, 34262.050] | +52.6% | 0.7% |  |
| `mixed/scenario_wordcount/unlimited` | 16015.373 [12186.984, 22362.976] | 24405.229 [18648.156, 34010.662] | +52.4% | 0.7% |  |
| `mixed/scenario_scheduler/concurrent` | 79473.878 [60516.354, 111783.156] | 120760.524 [92591.746, 168302.637] | +51.9% | 0.3% |  |
| `mixed/scenario_scheduler/local` | 79473.878 [60516.354, 111783.156] | 119167.677 [91201.812, 166989.178] | +49.9% | 0.3% |  |
| `mixed/scenario_scheduler/unlimited` | 79473.878 [60516.354, 111783.156] | 115490.512 [89127.407, 161083.809] | +45.3% | 0.3% |  |
| `mixed/scenario_index/concurrent` | 66803.855 [50564.048, 93271.309] | 66535.941 [50502.211, 93901.806] | -0.4% | 2.1% |  |
| `mixed/scenario_index/local` | 66803.855 [50564.048, 93271.309] | 67256.280 [51342.658, 94182.217] | +0.7% | 2.1% |  |
| `mixed/scenario_index/unlimited` | 66803.855 [50564.048, 93271.309] | 66441.266 [50356.020, 93406.885] | -0.5% | 2.1% |  |
| `mixed/scenario_graph/concurrent` | 34989.529 [26951.193, 48685.347] | 63876.896 [49542.218, 87435.645] | +82.6% | 0.9% |  |
| `mixed/scenario_graph/local` | 34989.529 [26951.193, 48685.347] | 63299.866 [49126.130, 86733.066] | +80.9% | 0.9% |  |
| `mixed/scenario_graph/unlimited` | 34989.529 [26951.193, 48685.347] | 63326.906 [49061.326, 86744.088] | +81.0% | 0.9% |  |
| `mixed/scenario_request/concurrent` | 24107.433 [18100.366, 34195.710] | 32685.552 [24930.447, 45493.486] | +35.6% | 3.0% |  |
| `mixed/scenario_request/local` | 24107.433 [18100.366, 34195.710] | 32456.689 [24707.681, 44990.979] | +34.6% | 3.0% |  |
| `mixed/scenario_request/unlimited` | 24107.433 [18100.366, 34195.710] | 31950.099 [24461.527, 44269.221] | +32.5% | 3.0% |  |

### Concurrent Arc scenarios

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `mixed/arc_app_state8_t1/borrowed` | 3.510 [3.506, 3.520] | 3.642 [3.616, 3.702] | +3.7% | 0.3% |  |
| `mixed/arc_app_state8_t1/owned` | 3.510 [3.506, 3.520] | 3.626 [3.568, 3.696] | +3.3% | 0.3% |  |
| `mixed/arc_app_state8_t1/unlimited` | 3.510 [3.506, 3.520] | 3.739 [3.615, 3.929] | +6.5% | 0.3% |  |
| `mixed/arc_app_state8_t2/borrowed` | 10.379 [9.674, 10.810] | 12.166 [10.882, 14.189] | +17.2% | 12.9% | Held |
| `mixed/arc_app_state8_t2/owned` | 10.379 [9.674, 10.810] | 13.094 [11.551, 15.161] | +26.2% | 12.9% | Held |
| `mixed/arc_app_state8_t2/unlimited` | 10.379 [9.674, 10.810] | 11.624 [11.131, 12.296] | +12.0% | 12.9% | Held |
| `mixed/arc_app_state8_t4/borrowed` | 17.272 [16.331, 20.335] | 26.490 [21.900, 34.766] | +53.4% | 5.0% |  |
| `mixed/arc_app_state8_t4/owned` | 17.272 [16.331, 20.335] | 25.438 [20.813, 32.393] | +47.3% | 5.0% |  |
| `mixed/arc_app_state8_t4/unlimited` | 17.272 [16.331, 20.335] | 23.096 [21.829, 27.824] | +33.7% | 5.0% |  |
| `mixed/arc_app_state128_t1/borrowed` | 57.181 [57.039, 57.308] | 57.883 [57.799, 57.955] | +1.2% | 0.5% |  |
| `mixed/arc_app_state128_t1/owned` | 57.181 [57.039, 57.308] | 57.851 [57.710, 57.977] | +1.2% | 0.5% |  |
| `mixed/arc_app_state128_t1/unlimited` | 57.181 [57.039, 57.308] | 58.027 [57.802, 58.295] | +1.5% | 0.5% |  |
| `mixed/arc_app_state128_t4/borrowed` | 51.199 [49.360, 53.290] | 58.810 [57.205, 61.187] | +14.9% | 1.4% |  |
| `mixed/arc_app_state128_t4/owned` | 51.199 [49.360, 53.290] | 58.880 [57.168, 60.657] | +15.0% | 1.4% |  |
| `mixed/arc_app_state128_t4/unlimited` | 51.199 [49.360, 53.290] | 60.754 [57.079, 62.738] | +18.7% | 1.4% |  |
| `mixed/arc_app_request64_t1/borrowed` | 32.637 [32.527, 32.769] | 37.062 [36.415, 38.840] | +13.6% | 0.6% |  |
| `mixed/arc_app_request64_t1/owned` | 32.637 [32.527, 32.769] | 37.815 [37.576, 38.142] | +15.9% | 0.6% |  |
| `mixed/arc_app_request64_t1/unlimited` | 32.637 [32.527, 32.769] | 33.284 [33.132, 33.603] | +2.0% | 0.6% |  |
| `mixed/arc_app_request64_t4/borrowed` | 12.705 [12.590, 12.776] | 55.966 [50.696, 57.967] | +340.5% | 1.0% |  |
| `mixed/arc_app_request64_t4/owned` | 12.705 [12.590, 12.776] | 76.708 [75.945, 77.642] | +503.7% | 1.0% |  |
| `mixed/arc_app_request64_t4/unlimited` | 12.705 [12.590, 12.776] | 12.734 [12.648, 12.869] | +0.2% | 1.0% |  |
| `mixed/arc_app_request4096_t1/borrowed` | 72.829 [72.448, 73.116] | 114.169 [113.015, 115.332] | +56.8% | 1.1% |  |
| `mixed/arc_app_request4096_t1/owned` | 72.829 [72.448, 73.116] | 115.904 [115.121, 116.585] | +59.1% | 1.1% |  |
| `mixed/arc_app_request4096_t1/unlimited` | 72.829 [72.448, 73.116] | 72.714 [72.032, 73.431] | -0.2% | 1.1% |  |
| `mixed/arc_app_request4096_t4/borrowed` | 92.179 [88.467, 95.289] | 98.576 [97.410, 99.419] | +6.9% | 7.5% |  |
| `mixed/arc_app_request4096_t4/owned` | 92.179 [88.467, 95.289] | 192.635 [190.228, 194.753] | +109.0% | 7.5% |  |
| `mixed/arc_app_request4096_t4/unlimited` | 92.179 [88.467, 95.289] | 90.740 [89.314, 92.198] | -1.6% | 7.5% | Held |
| `mixed/arc_app_domain64_t1/borrowed` | 32.558 [32.526, 32.607] | 36.442 [36.020, 36.753] | +11.9% | 0.4% |  |
| `mixed/arc_app_domain64_t1/owned` | 32.558 [32.526, 32.607] | 52.982 [52.860, 53.047] | +62.7% | 0.4% |  |
| `mixed/arc_app_domain64_t1/unlimited` | 32.558 [32.526, 32.607] | 33.266 [33.108, 33.568] | +2.2% | 0.4% |  |
| `mixed/arc_app_domain64_t4/borrowed` | 12.673 [12.535, 12.767] | 77.958 [51.504, 92.249] | +515.2% | 1.4% |  |
| `mixed/arc_app_domain64_t4/owned` | 12.673 [12.535, 12.767] | 19.126 [18.237, 19.679] | +50.9% | 1.4% |  |
| `mixed/arc_app_domain64_t4/unlimited` | 12.673 [12.535, 12.767] | 12.802 [12.740, 12.833] | +1.0% | 1.4% |  |
| `mixed/arc_app_cow4096_r1/borrowed` | 67.522 [65.951, 69.937] | 65.505 [65.340, 65.771] | -3.0% | 5.6% |  |
| `mixed/arc_app_cow4096_r1/owned` | 67.522 [65.951, 69.937] | 66.323 [65.895, 66.693] | -1.8% | 5.6% |  |
| `mixed/arc_app_cow4096_r1/unlimited` | 67.522 [65.951, 69.937] | 65.149 [64.545, 65.937] | -3.5% | 5.6% |  |
| `mixed/arc_app_cow4096_r128/borrowed` | 517.279 [515.282, 518.616] | 527.876 [509.650, 538.853] | +2.0% | 0.3% |  |
| `mixed/arc_app_cow4096_r128/owned` | 517.279 [515.282, 518.616] | 520.294 [506.670, 536.781] | +0.6% | 0.3% | Held |
| `mixed/arc_app_cow4096_r128/unlimited` | 517.279 [515.282, 518.616] | 496.059 [479.854, 517.329] | -4.1% | 0.3% |  |
| `mixed/arc_app_cow65536_r128/borrowed` | 1413.623 [1386.552, 1443.548] | 1460.614 [1453.049, 1468.048] | +3.3% | 3.4% |  |
| `mixed/arc_app_cow65536_r128/owned` | 1413.623 [1386.552, 1443.548] | 1459.337 [1451.032, 1468.041] | +3.2% | 3.4% |  |
| `mixed/arc_app_cow65536_r128/unlimited` | 1413.623 [1386.552, 1443.548] | 1423.363 [1410.463, 1444.451] | +0.7% | 3.4% |  |
| `mixed/arc_app_mutex_t1/borrowed` | 5.097 [5.095, 5.099] | 4.766 [4.753, 4.781] | -6.5% | 0.2% |  |
| `mixed/arc_app_mutex_t1/owned` | 5.097 [5.095, 5.099] | 4.768 [4.755, 4.784] | -6.5% | 0.2% |  |
| `mixed/arc_app_mutex_t1/unlimited` | 5.097 [5.095, 5.099] | 4.766 [4.762, 4.772] | -6.5% | 0.2% |  |
| `mixed/arc_app_mutex_t4/borrowed` | 26.994 [24.692, 30.684] | 23.787 [22.657, 27.488] | -11.9% | 14.7% | Held |
| `mixed/arc_app_mutex_t4/owned` | 26.994 [24.692, 30.684] | 43.757 [39.034, 50.424] | +62.1% | 14.7% | Held |
| `mixed/arc_app_mutex_t4/unlimited` | 26.994 [24.692, 30.684] | 25.095 [22.133, 29.095] | -7.0% | 14.7% | Held |
| `mixed/arc_app_unique4096_r1/borrowed` | 4.662 [4.568, 4.746] | 5.592 [5.324, 6.042] | +20.0% | 14.0% | Held |
| `mixed/arc_app_unique4096_r1/owned` | 4.662 [4.568, 4.746] | 5.398 [5.366, 5.422] | +15.8% | 14.0% | Held |
| `mixed/arc_app_unique4096_r1/unlimited` | 4.662 [4.568, 4.746] | 5.555 [5.530, 5.566] | +19.2% | 14.0% | Held |
| `mixed/arc_app_init4096_t1/borrowed` | 73.586 [72.387, 74.286] | 74.621 [73.961, 75.344] | +1.4% | 2.2% |  |
| `mixed/arc_app_init4096_t1/owned` | 73.586 [72.387, 74.286] | 75.108 [73.819, 76.603] | +2.1% | 2.2% |  |
| `mixed/arc_app_init4096_t1/unlimited` | 73.586 [72.387, 74.286] | 73.962 [72.830, 74.599] | +0.5% | 2.2% |  |
| `mixed/arc_app_init4096_t4/borrowed` | 92.444 [89.089, 95.161] | 42.201 [37.281, 52.979] | -54.4% | 4.5% |  |
| `mixed/arc_app_init4096_t4/owned` | 92.444 [89.089, 95.161] | 89.389 [85.303, 92.345] | -3.3% | 4.5% |  |
| `mixed/arc_app_init4096_t4/unlimited` | 92.444 [89.089, 95.161] | 93.645 [90.820, 96.247] | +1.3% | 4.5% | Held |

### B+ full lookup

| Condition | brie-tree, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `65,536 keys / 16:3 / fixed` | 19.612 [19.228, 20.002] | 17.921 [17.061, 18.841] | -8.6% | 3.1% |  |
| `65,536 keys / 16:3 / jzap` | 19.612 [19.228, 20.002] | 50.601 [50.393, 50.932] | +158.0% | 3.1% |  |
| `65,536 keys / 16:3 / packed` | 19.612 [19.228, 20.002] | 16.828 [16.137, 17.664] | -14.2% | 3.1% |  |
| `65,536 keys / 16:3 / resident` | 19.612 [19.228, 20.002] | 48.082 [47.917, 48.267] | +145.2% | 3.1% |  |
| `65,536 keys / 16:3 / split` | 19.612 [19.228, 20.002] | 18.009 [17.476, 18.498] | -8.2% | 3.1% |  |
| `32,768 keys / 32:2 / fixed` | 13.354 [13.096, 13.636] | 13.062 [12.977, 13.148] | -2.2% | 0.5% |  |
| `32,768 keys / 32:2 / jzap` | 13.354 [13.096, 13.636] | 40.684 [40.423, 40.958] | +204.7% | 0.5% |  |
| `32,768 keys / 32:2 / packed` | 13.354 [13.096, 13.636] | 12.414 [12.222, 12.506] | -7.0% | 0.5% |  |
| `32,768 keys / 32:2 / resident` | 13.354 [13.096, 13.636] | 35.884 [35.747, 36.051] | +168.7% | 0.5% |  |
| `32,768 keys / 32:2 / split` | 13.354 [13.096, 13.636] | 13.742 [13.600, 14.060] | +2.9% | 0.5% |  |
| `512 keys / 8:2 / fixed` | 5.639 [5.600, 5.664] | 6.660 [6.585, 6.739] | +18.1% | 0.5% |  |
| `512 keys / 8:2 / jzap` | 5.639 [5.600, 5.664] | 22.290 [21.896, 22.860] | +295.3% | 0.5% |  |
| `512 keys / 8:2 / packed` | 5.639 [5.600, 5.664] | 5.804 [5.671, 5.926] | +2.9% | 0.5% |  |
| `512 keys / 8:2 / resident` | 5.639 [5.600, 5.664] | 21.568 [21.212, 21.779] | +282.5% | 0.5% |  |
| `512 keys / 8:2 / split` | 5.639 [5.600, 5.664] | 6.614 [6.403, 6.724] | +17.3% | 0.5% |  |

### B+ fixed lookup

| Condition | brie-tree, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `65,536 keys / 16:3 / fixed` | 19.934 [19.195, 20.315] | 17.506 [16.648, 19.609] | -12.2% | 7.6% |  |
| `65,536 keys / 16:3 / jzap` | 19.934 [19.195, 20.315] | 51.218 [50.925, 51.530] | +156.9% | 7.6% |  |
| `65,536 keys / 16:3 / packed` | 19.934 [19.195, 20.315] | 16.274 [15.154, 16.936] | -18.4% | 7.6% |  |
| `32,768 keys / 32:2 / fixed` | 13.663 [13.378, 13.879] | 13.486 [13.307, 13.643] | -1.3% | 0.8% |  |
| `32,768 keys / 32:2 / jzap` | 13.663 [13.378, 13.879] | 41.117 [40.877, 41.329] | +200.9% | 0.8% |  |
| `32,768 keys / 32:2 / packed` | 13.663 [13.378, 13.879] | 12.760 [12.565, 12.972] | -6.6% | 0.8% |  |
| `512 keys / 8:2 / fixed` | 5.777 [5.683, 5.886] | 6.635 [6.567, 6.746] | +14.9% | 0.7% |  |
| `512 keys / 8:2 / jzap` | 5.777 [5.683, 5.886] | 22.242 [21.931, 22.807] | +285.0% | 0.7% |  |
| `512 keys / 8:2 / packed` | 5.777 [5.683, 5.886] | 5.713 [5.603, 5.878] | -1.1% | 0.7% | Held |

### B+ preparation

| Condition | brie-tree, ns/build [range] | jzap, ns/build [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `65,536 keys / 16:3 / fixed` | 361615.298 [360594.868, 362512.953] | 46113.064 [45972.070, 46345.920] | -87.2% | 0.1% |  |
| `32,768 keys / 32:2 / fixed` | 179325.016 [178993.946, 179554.690] | 23539.954 [23515.977, 23560.120] | -86.9% | 0.1% |  |
| `512 keys / 8:2 / fixed` | 2536.749 [2527.269, 2543.759] | 308.503 [307.089, 310.043] | -87.8% | 0.2% |  |

### B+ separate std build

| Condition | brie-tree, ns/op [range] | Measured implementation, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `65,536 keys / 16:3 / checked_ref` | 20.352 [19.941, 20.925] | 43.259 [42.960, 43.646] | +112.6% | 1.8% |  |
| `65,536 keys / 16:3 / checked_ref_validate` | 20.352 [19.941, 20.925] | 63.284 [62.654, 64.148] | +210.9% | 1.8% |  |
| `65,536 keys / 16:3 / jzap` | 20.352 [19.941, 20.925] | 49.541 [49.300, 49.931] | +143.4% | 1.8% |  |
| `65,536 keys / 16:3 / jzap_u32` | 20.352 [19.941, 20.925] | 52.020 [51.775, 52.330] | +155.6% | 1.8% |  |
| `65,536 keys / 16:3 / jzap_validate` | 20.352 [19.941, 20.925] | 66.631 [66.152, 67.717] | +227.4% | 1.8% |  |
| `65,536 keys / 16:3 / std` | 20.352 [19.941, 20.925] | 25.322 [22.960, 29.203] | +24.4% | 1.8% |  |
| `32,768 keys / 32:2 / checked_ref` | 13.689 [13.441, 13.832] | 35.832 [35.547, 36.116] | +161.8% | 0.3% |  |
| `32,768 keys / 32:2 / checked_ref_validate` | 13.689 [13.441, 13.832] | 60.578 [60.244, 60.731] | +342.5% | 0.3% |  |
| `32,768 keys / 32:2 / jzap` | 13.689 [13.441, 13.832] | 39.322 [39.117, 39.463] | +187.3% | 0.3% |  |
| `32,768 keys / 32:2 / jzap_u32` | 13.689 [13.441, 13.832] | 41.377 [41.266, 41.584] | +202.3% | 0.3% |  |
| `32,768 keys / 32:2 / jzap_validate` | 13.689 [13.441, 13.832] | 64.271 [64.052, 64.720] | +369.5% | 0.3% |  |
| `32,768 keys / 32:2 / std` | 13.689 [13.441, 13.832] | 21.117 [19.429, 22.524] | +54.3% | 0.3% |  |
| `512 keys / 8:2 / checked_ref` | 5.701 [5.593, 5.889] | 19.440 [19.067, 19.751] | +241.0% | 1.6% |  |
| `512 keys / 8:2 / checked_ref_validate` | 5.701 [5.593, 5.889] | 30.301 [30.193, 30.404] | +431.5% | 1.6% |  |
| `512 keys / 8:2 / jzap` | 5.701 [5.593, 5.889] | 22.973 [22.240, 23.507] | +303.0% | 1.6% |  |
| `512 keys / 8:2 / jzap_u32` | 5.701 [5.593, 5.889] | 23.741 [23.253, 24.219] | +316.5% | 1.6% |  |
| `512 keys / 8:2 / jzap_validate` | 5.701 [5.593, 5.889] | 35.633 [35.237, 35.852] | +525.1% | 1.6% |  |
| `512 keys / 8:2 / std` | 5.701 [5.593, 5.889] | 8.040 [7.908, 8.243] | +41.0% | 1.6% |  |

### Arithmetic

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `opaque/add/jzap vs native` | 0.281 [0.281, 0.281] | 0.280 [0.280, 0.281] | -0.2% | 1.4% |  |
| `opaque/add/jzap vs plain` | 0.284 [0.282, 0.286] | 0.280 [0.280, 0.281] | -1.2% | 2.4% |  |
| `opaque/add/wrapping vs plain` | 0.284 [0.282, 0.286] | 0.285 [0.285, 0.286] | +0.6% | 2.4% |  |
| `opaque/sub/jzap vs native` | 0.282 [0.281, 0.282] | 0.281 [0.281, 0.281] | -0.2% | 0.3% |  |
| `opaque/sub/jzap vs plain` | 0.284 [0.283, 0.285] | 0.281 [0.281, 0.281] | -1.0% | 1.2% |  |
| `opaque/sub/wrapping vs plain` | 0.284 [0.283, 0.285] | 0.284 [0.284, 0.285] | +0.1% | 1.2% |  |
| `opaque/mul/jzap vs native` | 0.272 [0.267, 0.278] | 0.277 [0.274, 0.280] | +1.9% | 4.8% |  |
| `opaque/mul/jzap vs plain` | 0.286 [0.284, 0.289] | 0.277 [0.274, 0.280] | -3.3% | 2.8% |  |
| `opaque/mul/wrapping vs plain` | 0.286 [0.284, 0.289] | 0.288 [0.283, 0.292] | +0.5% | 2.8% |  |
| `opaque/div/jzap vs native` | 0.501 [0.500, 0.503] | 0.504 [0.501, 0.506] | +0.6% | 2.0% |  |
| `opaque/div/jzap vs plain` | 0.575 [0.570, 0.579] | 0.504 [0.501, 0.506] | -12.3% | 1.0% |  |
| `opaque/div/wrapping vs plain` | 0.575 [0.570, 0.579] | 0.502 [0.496, 0.505] | -12.7% | 1.0% |  |
| `bounded/add/jzap vs native` | 0.518 [0.518, 0.519] | 0.520 [0.518, 0.523] | +0.3% | 0.4% |  |
| `bounded/add/jzap vs plain` | 0.511 [0.510, 0.511] | 0.520 [0.518, 0.523] | +1.8% | 2.4% |  |
| `bounded/add/wrapping vs plain` | 0.511 [0.510, 0.511] | 0.527 [0.517, 0.546] | +3.1% | 2.4% |  |
| `bounded/sub/jzap vs native` | 0.481 [0.478, 0.486] | 0.480 [0.478, 0.484] | -0.1% | 1.6% |  |
| `bounded/sub/jzap vs plain` | 0.487 [0.478, 0.502] | 0.480 [0.478, 0.484] | -1.3% | 4.6% |  |
| `bounded/sub/wrapping vs plain` | 0.487 [0.478, 0.502] | 0.486 [0.478, 0.500] | -0.2% | 4.6% |  |
| `bounded/mul/jzap vs native` | 0.515 [0.511, 0.518] | 0.521 [0.517, 0.527] | +1.0% | 0.9% |  |
| `bounded/mul/jzap vs plain` | 0.511 [0.511, 0.511] | 0.521 [0.517, 0.527] | +1.9% | 1.8% |  |
| `bounded/mul/wrapping vs plain` | 0.511 [0.511, 0.511] | 0.526 [0.518, 0.532] | +2.8% | 1.8% |  |
| `bounded/div/jzap vs native` | 0.565 [0.565, 0.565] | 0.563 [0.563, 0.563] | -0.3% | 0.4% |  |
| `bounded/div/jzap vs plain` | 0.563 [0.562, 0.563] | 0.563 [0.563, 0.563] | +0.1% | 0.6% |  |
| `bounded/div/wrapping vs plain` | 0.563 [0.562, 0.563] | 0.565 [0.564, 0.566] | +0.3% | 0.6% |  |
| `call/add/jzap vs native` | 0.721 [0.719, 0.725] | 0.723 [0.720, 0.726] | +0.2% | 0.6% |  |
| `call/add/jzap vs plain` | 0.935 [0.923, 0.946] | 0.723 [0.720, 0.726] | -22.7% | 2.5% |  |
| `call/add/wrapping vs plain` | 0.935 [0.923, 0.946] | 0.711 [0.710, 0.713] | -23.9% | 2.5% |  |
| `call/sub/jzap vs native` | 0.728 [0.725, 0.730] | 0.728 [0.727, 0.729] | +0.0% | 0.8% |  |
| `call/sub/jzap vs plain` | 0.717 [0.716, 0.718] | 0.728 [0.727, 0.729] | +1.6% | 0.7% |  |
| `call/sub/wrapping vs plain` | 0.717 [0.716, 0.718] | 0.716 [0.715, 0.716] | -0.1% | 0.7% |  |
| `call/mul/jzap vs native` | 0.733 [0.731, 0.734] | 0.734 [0.733, 0.737] | +0.2% | 0.6% |  |
| `call/mul/jzap vs plain` | 0.716 [0.715, 0.718] | 0.734 [0.733, 0.737] | +2.5% | 1.1% |  |
| `call/mul/wrapping vs plain` | 0.716 [0.715, 0.718] | 0.719 [0.717, 0.720] | +0.4% | 1.1% |  |
| `call/div/jzap vs native` | 0.731 [0.730, 0.733] | 0.729 [0.727, 0.733] | -0.2% | 0.4% |  |
| `call/div/jzap vs plain` | 0.722 [0.720, 0.723] | 0.729 [0.727, 0.733] | +1.1% | 0.4% |  |
| `call/div/wrapping vs plain` | 0.722 [0.720, 0.723] | 0.733 [0.732, 0.733] | +1.5% | 0.4% |  |

### `Checked<T>`

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `add/ok` | 0.735 [0.734, 0.735] | 0.737 [0.736, 0.738] | +0.3% | 0.1% |  |
| `add/fail` | 0.738 [0.737, 0.739] | 0.738 [0.737, 0.739] | +0.0% | 0.4% |  |
| `sub/ok` | 0.739 [0.736, 0.747] | 0.736 [0.736, 0.737] | -0.4% | 1.1% |  |
| `sub/fail` | 0.738 [0.737, 0.739] | 0.737 [0.736, 0.738] | -0.2% | 0.4% |  |
| `mul/ok` | 0.740 [0.739, 0.740] | 0.740 [0.739, 0.740] | +0.1% | 0.2% |  |
| `mul/fail` | 0.741 [0.741, 0.742] | 0.742 [0.741, 0.742] | +0.1% | 0.1% |  |
| `div/ok` | 0.737 [0.737, 0.737] | 0.738 [0.737, 0.738] | +0.1% | 0.3% |  |
| `div/fail` | 0.945 [0.914, 0.960] | 0.922 [0.875, 0.961] | -2.3% | 12.7% | Held |
| `rem/ok` | 0.741 [0.741, 0.742] | 0.742 [0.742, 0.743] | +0.1% | 0.1% |  |
| `rem/fail` | 0.923 [0.848, 0.961] | 0.897 [0.835, 0.931] | -2.9% | 19.4% | Held |

### Box long lifecycle

| Condition | Comparator, ns/op [range] | jzap, ns/op [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `phase/unmetered vs std` | 26.244 [26.172, 26.364] | 26.401 [26.113, 26.952] | +0.6% | 4.0% |  |
| `phase/unmetered vs varlit` | 26.487 [26.120, 27.021] | 26.401 [26.113, 26.952] | -0.3% | 5.8% | Held |
| `phase/metered vs std` | 26.244 [26.172, 26.364] | 27.072 [26.468, 27.890] | +3.2% | 4.0% |  |
| `phase/metered vs varlit` | 26.487 [26.120, 27.021] | 27.072 [26.468, 27.890] | +2.2% | 5.8% |  |
| `item/unmetered vs std` | 26.618 [26.223, 27.172] | 26.794 [26.365, 27.391] | +0.7% | 2.7% |  |
| `item/unmetered vs varlit` | 26.518 [26.181, 27.137] | 26.794 [26.365, 27.391] | +1.0% | 3.0% |  |
| `item/metered vs std` | 26.618 [26.223, 27.172] | 27.019 [26.977, 27.055] | +1.5% | 2.7% |  |
| `item/metered vs varlit` | 26.518 [26.181, 27.137] | 27.019 [26.977, 27.055] | +1.9% | 3.0% |  |

### Box ABI diagnostics

| Condition | Comparator, ns/box [range] | jzap, ns/box [range] | Delta | A/A | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| `phase/create/unmetered vs std` | 8.701 [7.454, 10.472] | 7.800 [7.462, 8.168] | -10.4% | 29.4% | Held |
| `phase/create/unmetered vs varlit` | 7.526 [7.417, 7.711] | 7.800 [7.462, 8.168] | +3.6% | 29.4% | Held |
| `phase/create/metered vs std` | 8.701 [7.454, 10.472] | 7.803 [7.451, 8.504] | -10.3% | 29.4% | Held |
| `phase/create/metered vs varlit` | 7.526 [7.417, 7.711] | 7.803 [7.451, 8.504] | +3.7% | 29.4% | Held |
| `phase/clone/unmetered vs std` | 8.605 [7.473, 10.391] | 7.577 [7.460, 7.805] | -11.9% | 22.5% | Held |
| `phase/clone/unmetered vs varlit` | 7.554 [7.446, 7.729] | 7.577 [7.460, 7.805] | +0.3% | 22.5% | Held |
| `phase/clone/metered vs std` | 8.605 [7.473, 10.391] | 7.822 [7.710, 8.034] | -9.1% | 22.5% | Held |
| `phase/clone/metered vs varlit` | 7.554 [7.446, 7.729] | 7.822 [7.710, 8.034] | +3.5% | 22.5% | Held |
| `phase/drop/unmetered vs std` | 12.917 [11.435, 15.861] | 11.716 [11.449, 12.139] | -9.3% | 28.5% | Held |
| `phase/drop/unmetered vs varlit` | 11.088 [10.885, 11.419] | 11.716 [11.449, 12.139] | +5.7% | 28.5% | Held |
| `phase/drop/metered vs std` | 12.917 [11.435, 15.861] | 11.541 [11.491, 11.639] | -10.7% | 28.5% | Held |
| `phase/drop/metered vs varlit` | 11.088 [10.885, 11.419] | 11.541 [11.491, 11.639] | +4.1% | 28.5% | Held |
| `item/create/unmetered vs std` | 7.964 [7.436, 8.253] | 7.904 [7.456, 8.151] | -0.8% | 9.6% |  |
| `item/create/unmetered vs varlit` | 7.433 [7.403, 7.460] | 7.904 [7.456, 8.151] | +6.3% | 9.6% |  |
| `item/create/metered vs std` | 7.964 [7.436, 8.253] | 7.487 [7.474, 7.495] | -6.0% | 9.6% |  |
| `item/create/metered vs varlit` | 7.433 [7.403, 7.460] | 7.487 [7.474, 7.495] | +0.7% | 9.6% |  |
| `item/clone/unmetered vs std` | 7.494 [7.467, 7.547] | 7.465 [7.445, 7.488] | -0.4% | 7.1% |  |
| `item/clone/unmetered vs varlit` | 7.462 [7.452, 7.468] | 7.465 [7.445, 7.488] | +0.0% | 7.1% |  |
| `item/clone/metered vs std` | 7.494 [7.467, 7.547] | 8.001 [7.981, 8.028] | +6.8% | 7.1% |  |
| `item/clone/metered vs varlit` | 7.462 [7.452, 7.468] | 8.001 [7.981, 8.028] | +7.2% | 7.1% |  |
| `item/drop/unmetered vs std` | 11.497 [11.383, 11.644] | 11.589 [11.365, 11.766] | +0.8% | 36.2% | Held |
| `item/drop/unmetered vs varlit` | 11.527 [10.972, 12.148] | 11.589 [11.365, 11.766] | +0.5% | 36.2% | Held |
| `item/drop/metered vs std` | 11.497 [11.383, 11.644] | 12.286 [10.969, 14.658] | +6.9% | 36.2% | Held |
| `item/drop/metered vs varlit` | 11.527 [10.972, 12.148] | 12.286 [10.969, 14.658] | +6.6% | 36.2% | Held |

## B+ range and point lookup follow-up

### Follow-up conditions

| Item | Value |
| --- | --- |
| Candidate | `25f773789b0e64d2548d1776f1483dae7fc98ade` ([PR #7](https://github.com/shim9610/jzap-std/pull/7)) |
| Point baseline | `665c95bba6cc3f94b2c84fbacc1192f1585bd553`, rebuilt and run in the same campaign |
| Machine | Apple M4 Mac16,10, 4 performance and 6 efficiency cores, 16 GiB RAM |
| Environment | macOS 27.2 (26B123), native arm64, Rosetta off, Rust 1.95.0, AC power, Low Power Mode off |
| Scheduling | Serial processes under `caffeinate -i`, CPU affinity unset |
| Builds | Cargo `+1.95.0 build --locked --release`; `bplus-bench --bin bplus_range` for range and `bplus-pool --bin bplus` for point |
| Range executable SHA-256 | `ef8765c7373ec8ff9c6399a7bbed5fd45b88fcedd28a4b35a2cece099740a5ae` |
| Point executable SHA-256 (both commits) | `4586dc2a7babbfc9c6b68c1eb703e8164444416bbbcb542e2923d86b3eea0cf2` |

Both point binaries were built independently at the same checkout path with the same options. The two files had identical contents. The focused BTree test suite passed 24 tests at the baseline and 26 at the candidate. The candidate's all-features suite passed 433 tests.

Both benchmarks used matching `u64` inputs, operations and checksums within each pair. Keys were ascending even integers starting at 2, with values equal to three times the key. Each accepted interval was at least one second. Each representative time is accepted total time divided by the total number of operations. Difference is jzap minus brie-tree, calculated before rounding.

### Public `FixedTree::range`

The range benchmark used fanout/height 8/2, 16/3 and 32/2, with 4,096 common queries per case and seed. Bounds were lower-inclusive and upper-exclusive. The timed interval included public range creation, consumption and the final exhaustion check. Tree construction preceded timing. Seeds 1, 2, 3 and 10 ran twice in fresh processes. Each group included brie-tree 0.1.2, jzap and `brie_control` in the same process.

A total of 24 seed/fixture runs and 96 seed/fixture/return-count groups were accepted. The first width-8, seed-1 run stopped on a 0.494-second interval. The whole group was rerun successfully with the original executable. Calls, returned item counts and checksums matched in all 96 accepted groups.

Times and differences are in **ns/range**.

| Width/height and returned items | brie-tree time | jzap time | Difference |
| --- | ---: | ---: | ---: |
| 8/2 · 0 items | 4.410 | 9.686 | +5.276 |
| 8/2 · 1 items | 9.828 | 11.145 | +1.317 |
| 8/2 · 32 items | 46.470 | 41.872 | -4.598 |
| 8/2 · 256 items | 168.416 | 171.143 | +2.727 |
| 16/3 · 0 items | 14.342 | 18.228 | +3.885 |
| 16/3 · 1 items | 25.951 | 20.454 | -5.498 |
| 16/3 · 32 items | 97.828 | 66.155 | -31.673 |
| 16/3 · 256 items | 283.049 | 204.435 | -78.615 |
| 32/2 · 0 items | 10.879 | 14.173 | +3.295 |
| 32/2 · 1 items | 20.265 | 16.207 | -4.058 |
| 32/2 · 32 items | 85.388 | 57.761 | -27.627 |
| 32/2 · 256 items | 262.412 | 177.590 | -84.822 |

Round differences pool the four seeds in each round. In parentheses, the counts show positive and negative paired differences. Min–max covers the eight seed/round observations. A/A is the maximum absolute change between brie-tree and its same-process control. The hold rules are A/A above 10% or paired differences exceeding the greater of 2% of comparator time and 0.1 ns in both directions. No range row met either hold condition.

| Width/height and returned items | Round 1 difference | Round 2 difference | brie-tree min–max | jzap min–max | Paired difference min–max | Max brie-tree A/A | Held reason |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 8/2 · 0 items | +5.213 (4+/0−) | +5.342 (4+/0−) | 4.331–4.437 | 9.345–9.878 | 5.014–5.454 | 0.27% | — |
| 8/2 · 1 items | +1.301 (4+/0−) | +1.333 (4+/0−) | 9.510–10.080 | 10.718–11.295 | 1.208–1.454 | 0.57% | — |
| 8/2 · 32 items | -4.591 (0+/4−) | -4.605 (0+/4−) | 45.127–46.897 | 41.168–43.156 | -5.456–-3.644 | 0.42% | — |
| 8/2 · 256 items | +2.716 (4+/0−) | +2.739 (4+/0−) | 165.797–169.110 | 168.575–172.069 | 2.476–2.959 | 0.13% | — |
| 16/3 · 0 items | +3.901 (4+/0−) | +3.870 (4+/0−) | 13.925–14.579 | 17.911–18.455 | 3.745–4.039 | 1.92% | — |
| 16/3 · 1 items | -5.361 (0+/4−) | -5.635 (0+/4−) | 25.044–26.472 | 19.961–20.885 | -5.845–-5.083 | 1.15% | — |
| 16/3 · 32 items | -31.483 (0+/4−) | -31.873 (0+/4−) | 95.931–98.876 | 65.065–66.545 | -32.546–-30.660 | 1.93% | — |
| 16/3 · 256 items | -78.754 (0+/4−) | -78.474 (0+/4−) | 281.800–286.061 | 202.240–206.293 | -82.960–-75.668 | 2.63% | — |
| 32/2 · 0 items | +3.225 (4+/0−) | +3.368 (4+/0−) | 10.666–10.983 | 13.916–14.397 | 3.141–3.496 | 0.70% | — |
| 32/2 · 1 items | -3.968 (0+/4−) | -4.148 (0+/4−) | 19.548–20.658 | 15.989–16.383 | -4.379–-3.559 | 1.24% | — |
| 32/2 · 32 items | -27.883 (0+/4−) | -27.366 (0+/4−) | 83.380–86.615 | 56.685–58.820 | -29.335–-26.060 | 2.36% | — |
| 32/2 · 256 items | -85.040 (0+/4−) | -84.597 (0+/4−) | 260.139–264.274 | 176.169–179.133 | -87.862–-82.945 | 0.89% | — |

At width 8, the paired differences for 0, 1 and 256 returned items were positive in all four seeds in each round.

### Public point lookup

The point runs used `BPLUS_FIXED_ONLY=1`, with 71,220, 30,442 and 26,922 batches at 512, 32,768 and 65,536 keys respectively. Each batch used 4,096 queries. The baseline and candidate alternated first position for seeds 1, 2, 3 and 10. An independent seed-1 pair supplied A/A and was excluded from the central estimates. Each result is paired with brie-tree 0.1.2 in the same process. All 30 fixture/build/process groups matched the expected query counts and checksums.

Times and differences are in **ns/lookup**.

| Keys, API and commit | brie-tree time | jzap time | Difference |
| --- | ---: | ---: | ---: |
| 512 keys · `FixedTree::get` · 665c95b | 5.691 | 7.018 | +1.327 |
| 512 keys · `FixedTree::get` · 25f7737 | 5.650 | 6.981 | +1.331 |
| 512 keys · streaming `Lookup` · 665c95b | 5.691 | 22.640 | +16.949 |
| 512 keys · streaming `Lookup` · 25f7737 | 5.650 | 23.375 | +17.725 |
| 65,536 keys · `FixedTree::get` · 665c95b | 22.003 | 16.680 | -5.323 |
| 65,536 keys · `FixedTree::get` · 25f7737 | 21.117 | 16.609 | -4.508 |
| 65,536 keys · streaming `Lookup` · 665c95b | 22.003 | 50.105 | +28.101 |
| 65,536 keys · streaming `Lookup` · 25f7737 | 21.117 | 49.947 | +28.829 |
| 32,768 keys · `FixedTree::get` · 665c95b | 14.115 | 13.020 | -1.095 |
| 32,768 keys · `FixedTree::get` · 25f7737 | 13.903 | 12.894 | -1.009 |
| 32,768 keys · streaming `Lookup` · 665c95b | 14.115 | 40.035 | +25.921 |
| 32,768 keys · streaming `Lookup` · 25f7737 | 13.903 | 39.946 | +26.043 |

Normalized change divides the candidate jzap/brie-tree ratio by the baseline jzap/brie-tree ratio, then subtracts one. A/A is the absolute change from the first seed-1 process to its repeat.

| Keys and API | Candidate − baseline (ns/lookup) | Normalized change | Per-seed normalized min–max | brie-tree A/A (baseline / candidate) | jzap A/A (baseline / candidate) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 512 keys · `FixedTree::get` | -0.037 | +0.20% | -5.72%–+6.27% | 0.88% / 2.59% | 0.43% / 1.55% |
| 512 keys · streaming `Lookup` | +0.735 | +4.00% | -0.11%–+8.63% | 0.88% / 2.59% | 0.33% / 0.62% |
| 65,536 keys · `FixedTree::get` | -0.071 | +3.75% | -9.52%–+12.13% | 2.99% / 1.25% | 1.12% / 3.94% |
| 65,536 keys · streaming `Lookup` | -0.158 | +3.87% | -0.04%–+7.56% | 2.99% / 1.25% | 0.05% / 1.92% |
| 32,768 keys · `FixedTree::get` | -0.126 | +0.54% | -2.29%–+2.84% | 2.65% / 0.02% | 1.03% / 2.53% |
| 32,768 keys · streaming `Lookup` | -0.090 | +1.29% | -2.54%–+3.74% | 2.65% / 0.02% | 0.09% / 0.52% |

| Keys and API | brie-tree baseline min–max | brie-tree candidate min–max | jzap baseline min–max | jzap candidate min–max |
| --- | ---: | ---: | ---: | ---: |
| 512 keys · `FixedTree::get` | 5.641–5.750 | 5.542–5.842 | 6.715–7.231 | 6.736–7.253 |
| 512 keys · streaming `Lookup` | 5.641–5.750 | 5.542–5.842 | 22.263–22.912 | 23.225–23.629 |
| 65,536 keys · `FixedTree::get` | 21.031–22.885 | 20.519–21.981 | 15.069–17.738 | 15.396–17.750 |
| 65,536 keys · streaming `Lookup` | 21.031–22.885 | 20.519–21.981 | 50.013–50.209 | 49.792–50.122 |
| 32,768 keys · `FixedTree::get` | 13.761–14.699 | 13.586–14.243 | 12.811–13.279 | 12.745–13.180 |
| 32,768 keys · streaming `Lookup` | 13.761–14.699 | 13.586–14.243 | 39.829–40.174 | 39.759–40.157 |

Per-seed normalized changes spanned both signs in all six point conditions.

## ARM64 hash-table follow-up

### Conditions

- Scalar baseline `665c95bba6cc3f94b2c84fbacc1192f1585bd553`. NEON candidate `10c0598f025ffc69e424a21947cca7fa85f3f144` from [PR #9](https://github.com/shim9610/jzap-std/pull/9). [Requested workloads](https://github.com/shim9610/jzap-std/pull/8#issuecomment-7029849362)
- Measured 2026-11-20 on Apple M4 Mac16,10 with macOS 27.2 (26B123), native `aarch64-apple-darwin`, Rust 1.95.0, AC power, and Low Power Mode off. Processes ran serially under `caffeinate -i` with CPU affinity unset.
- Both revisions were built independently at the same absolute checkout and Cargo target paths. HashMap and HashSet tests passed 43 tests per revision. The timing build command was:

```sh
cargo +1.95.0 build --locked --release \
  --manifest-path tools/throughput-analysis/Cargo.toml \
  --features hash-map,hash-set,owned-budgets,bplus-pool \
  --bin hash_map --bin hash_set --bin scenarios \
  --target-dir target/verification/macos-hash/build
```

- Each mixed-size workload used seeds 617, 6203, and 96011. Each seed ran both revisions and three fresh processes per revision: std, unmetered jzap, and a repeat of std. Run order rotated by seed and revision. All six runs per accepted group matched epochs, input corpus digest, operation counts, and checksum.
- The target interval was 1.2 seconds. The shortest accepted interval was 1.177765 seconds. There were 21 accepted seed groups, 126 accepted timing records, and 0 rejected timing attempts due to a short interval.
- `hash_map_remove_cycle` counts one remove/insert pair per operation. `hash_map_fixed_get` counts each lookup and uses equal zero SipHash13 keys. Build, owned union, and scenario rows count one complete workload per operation. The mixed-size workloads use 64 generated cases per seed.

| Binary | Scalar SHA-256 | NEON SHA-256 |
| --- | --- | --- |
| `hash_map` | `a0be0866fdc800f38b9a99866f50aca19327edb1d8de0f1f24643b40076ab36e` | `85a2b81fadebd22e9ccc4f775c60cdcf78fe321af21ea7febd6381c55fd0b021` |
| `hash_set` | `fc60b703b8b75e1b9a1ff4934f3bf69db80ea27b561fbb02d3225c46d3fdfb3b` | `1ed0000b9e91ebc1ebf55d6d40a86563891924d156f7986b83e0ff200af587b2` |
| `scenarios` | `d347fc5315fe4b9a8c6325dd9ceb8b5915ae6e05ffdd800adcb10ba0209c0e43` | `44e4fead5406e1fa85aa2440c3b83edd2738a4b17e78701ae60e0445efd7873f` |

### NEON candidate

Times are in ns/operation. Each representative time is total timed seconds divided by total matched operations. Differences use unrounded values.

| Condition | std time | jzap time | jzap − std |
| --- | ---: | ---: | ---: |
| `hash_map_remove_cycle` | 12.939 | 15.202 | +2.263 (+17.5%) |
| `hash_map_fixed_build` | 10164.181 | 9168.744 | -995.437 (-9.8%) |
| `hash_map_fixed_get` | 8.444 | 5.575 | -2.869 (-34.0%) |
| `hash_set_build` | 8820.557 | 9742.409 | +921.852 (+10.5%) |
| `hash_set_owned_union` | 11241.174 | 10453.439 | -787.736 (-7.0%) |
| `scenario_graph` | 35264.489 | 34521.564 | -742.926 (-2.1%) |
| `scenario_scheduler` | 77192.352 | 76111.168 | -1081.184 (-1.4%) |

### Scalar baseline rerun

Times are in ns/operation. Each representative time is total timed seconds divided by total matched operations. Differences use unrounded values.

| Condition | std time | jzap time | jzap − std |
| --- | ---: | ---: | ---: |
| `hash_map_remove_cycle` | 11.869 | 51.989 | +40.120 (+338.0%) |
| `hash_map_fixed_build` | 10092.060 | 33997.942 | +23905.882 (+236.9%) |
| `hash_map_fixed_get` | 8.363 | 6.707 | -1.656 (-19.8%) |
| `hash_set_build` | 8874.830 | 36214.391 | +27339.561 (+308.1%) |
| `hash_set_owned_union` | 11256.908 | 42860.286 | +31603.378 (+280.7%) |
| `scenario_graph` | 34939.832 | 61045.572 | +26105.740 (+74.7%) |
| `scenario_scheduler` | 76872.191 | 111025.909 | +34153.718 (+44.4%) |

### Seed spread and A/A

The span is the minimum and maximum of the three seed results in ns/operation. Max A/A is the largest absolute std repeat difference as a fraction of the corresponding std time. A row is held if A/A exceeds 10% or paired differences exceed max(2% of std, 0.1 ns) in both directions.

| Condition | Revision | std span | jzap span | Paired difference span | Max A/A | Held reason |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `hash_map_remove_cycle` | scalar | 11.600 to 12.163 | 51.004 to 52.700 | 39.404 to 40.537 | 2.67% | — |
| `hash_map_remove_cycle` | neon | 12.187 to 13.521 | 14.679 to 15.748 | 2.055 to 2.492 | 3.97% | — |
| `hash_map_fixed_build` | scalar | 8485.482 to 11258.958 | 28276.582 to 37792.877 | 19791.101 to 26646.131 | 2.38% | — |
| `hash_map_fixed_build` | neon | 8572.139 to 11323.599 | 7744.987 to 10154.295 | -1169.303 to -827.152 | 3.24% | — |
| `hash_map_fixed_get` | scalar | 7.815 to 8.929 | 6.689 to 6.730 | -2.227 to -1.084 | 2.20% | — |
| `hash_map_fixed_get` | neon | 7.886 to 8.897 | 5.555 to 5.588 | -3.309 to -2.305 | 6.08% | — |
| `hash_set_build` | scalar | 7510.137 to 9797.409 | 30387.288 to 40350.524 | 22877.151 to 30553.115 | 0.30% | — |
| `hash_set_build` | neon | 7485.068 to 9751.524 | 8256.827 to 10772.655 | 771.758 to 1021.131 | 0.65% | — |
| `hash_set_owned_union` | scalar | 9046.436 to 13078.376 | 34765.468 to 48903.096 | 25719.032 to 36063.262 | 0.67% | — |
| `hash_set_owned_union` | neon | 9057.863 to 13102.858 | 8196.711 to 12151.574 | -951.284 to -520.308 | 1.02% | — |
| `scenario_graph` | scalar | 26902.714 to 47846.033 | 47259.130 to 83678.755 | 20356.416 to 35832.722 | 0.84% | — |
| `scenario_graph` | neon | 26760.688 to 48642.367 | 26293.830 to 48090.742 | -1256.367 to -466.858 | 2.28% | — |
| `scenario_scheduler` | scalar | 58180.769 to 108737.061 | 85175.903 to 154795.721 | 26995.134 to 46058.660 | 0.40% | — |
| `scenario_scheduler` | neon | 58523.207 to 109096.469 | 57857.390 to 106955.855 | -2140.614 to -665.817 | 0.47% | — |

| Condition | jzap change (NEON vs scalar) | std change |
| --- | ---: | ---: |
| `hash_map_remove_cycle` | -70.8% | +9.0% |
| `hash_map_fixed_build` | -73.0% | +0.7% |
| `hash_map_fixed_get` | -16.9% | +1.0% |
| `hash_set_build` | -73.1% | -0.6% |
| `hash_set_owned_union` | -75.6% | -0.1% |
| `scenario_graph` | -43.4% | +0.9% |
| `scenario_scheduler` | -31.4% | +0.4% |

<details>
<summary>Raw timing records</summary>

Rows are in execution order. `unlimited` is unmetered jzap and `std_control` is the independent std repeat. Each role used the same seed, corpus and operation count within an attempt.

```csv
task,seed,attempt,accepted,variant,label,elapsed_seconds,operations,checksum,corpus_digest
hash_map_remove_cycle,617,1,True,scalar,std,1.235427208,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,617,1,True,scalar,std_control,1.24033325,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,617,1,True,scalar,unlimited,5.4321175,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,617,1,True,neon,std_control,1.329060291,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,617,1,True,neon,unlimited,1.563315583,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,617,1,True,neon,std,1.297959667,106503693,962396678070,5493369367503854109
hash_map_remove_cycle,6203,1,True,neon,std_control,1.339508167,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,6203,1,True,neon,std,1.324473125,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,6203,1,True,neon,unlimited,1.53120875,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,6203,1,True,scalar,std,1.1937929170000001,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,6203,1,True,scalar,unlimited,5.265582542,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,6203,1,True,scalar,std_control,1.22566225,100616545,916093484010,16571444928825650321
hash_map_remove_cycle,96011,1,True,scalar,unlimited,5.214368792,98945070,848850712488,2647082129846269003
hash_map_remove_cycle,96011,1,True,scalar,std,1.203423875,98945070,848850712488,2647082129846269003
hash_map_remove_cycle,96011,1,True,scalar,std_control,1.183657042,98945070,848850712488,2647082129846269003
hash_map_remove_cycle,96011,1,True,neon,std,1.3377905,98945070,848850712488,2647082129846269003
hash_map_remove_cycle,96011,1,True,neon,std_control,1.390846542,98945070,848850712488,2647082129846269003
hash_map_remove_cycle,96011,1,True,neon,unlimited,1.5581876669999999,98945070,848850712488,2647082129846269003
hash_map_fixed_build,617,1,True,scalar,std,1.307714542,154112,64421224,5493369367503854109
hash_map_fixed_build,617,1,True,scalar,std_control,1.287664167,154112,64421224,5493369367503854109
hash_map_fixed_build,617,1,True,scalar,unlimited,4.357760667,154112,64421224,5493369367503854109
hash_map_fixed_build,617,1,True,neon,std_control,1.30089275,154112,64421224,5493369367503854109
hash_map_fixed_build,617,1,True,neon,unlimited,1.193595375,154112,64421224,5493369367503854109
hash_map_fixed_build,617,1,True,neon,std,1.321069458,154112,64421224,5493369367503854109
hash_map_fixed_build,6203,1,True,neon,std_control,1.390906417,118976,66446237,16571444928825650321
hash_map_fixed_build,6203,1,True,neon,std,1.347236458,118976,66446237,16571444928825650321
hash_map_fixed_build,6203,1,True,neon,unlimited,1.208117417,118976,66446237,16571444928825650321
hash_map_fixed_build,6203,1,True,scalar,std,1.339545833,118976,66446237,16571444928825650321
hash_map_fixed_build,6203,1,True,scalar,unlimited,4.496445291,118976,66446237,16571444928825650321
hash_map_fixed_build,6203,1,True,scalar,std_control,1.371426083,118976,66446237,16571444928825650321
hash_map_fixed_build,96011,1,True,scalar,unlimited,4.418590708,117312,66143805,2647082129846269003
hash_map_fixed_build,96011,1,True,scalar,std,1.292679833,117312,66143805,2647082129846269003
hash_map_fixed_build,96011,1,True,scalar,std_control,1.306754167,117312,66143805,2647082129846269003
hash_map_fixed_build,96011,1,True,neon,std,1.299790167,117312,66143805,2647082129846269003
hash_map_fixed_build,96011,1,True,neon,std_control,1.306383166,117312,66143805,2647082129846269003
hash_map_fixed_build,96011,1,True,neon,unlimited,1.177764708,117312,66143805,2647082129846269003
hash_map_fixed_get,617,1,True,scalar,std,1.8301014169999998,219428106,991406374470,5493369367503854109
hash_map_fixed_get,617,1,True,scalar,std_control,1.789922667,219428106,991406374470,5493369367503854109
hash_map_fixed_get,617,1,True,scalar,unlimited,1.4677537090000001,219428106,991406374470,5493369367503854109
hash_map_fixed_get,617,1,True,neon,std_control,1.760636417,219428106,991406374470,5493369367503854109
hash_map_fixed_get,617,1,True,neon,unlimited,1.218826833,219428106,991406374470,5493369367503854109
hash_map_fixed_get,617,1,True,neon,std,1.874533333,219428106,991406374470,5493369367503854109
hash_map_fixed_get,6203,1,True,neon,std_control,1.9325655830000001,215816234,982481786226,16571444928825650321
hash_map_fixed_get,6203,1,True,neon,std,1.920224459,215816234,982481786226,16571444928825650321
hash_map_fixed_get,6203,1,True,neon,unlimited,1.206048625,215816234,982481786226,16571444928825650321
hash_map_fixed_get,6203,1,True,scalar,std,1.926955209,215816234,982481786226,16571444928825650321
hash_map_fixed_get,6203,1,True,scalar,unlimited,1.446374917,215816234,982481786226,16571444928825650321
hash_map_fixed_get,6203,1,True,scalar,std_control,1.933974209,215816234,982481786226,16571444928825650321
hash_map_fixed_get,96011,1,True,scalar,unlimited,1.441652,214200560,918814337952,2647082129846269003
hash_map_fixed_get,96011,1,True,scalar,std,1.673932333,214200560,918814337952,2647082129846269003
hash_map_fixed_get,96011,1,True,scalar,std_control,1.689265667,214200560,918814337952,2647082129846269003
hash_map_fixed_get,96011,1,True,neon,std,1.689206667,214200560,918814337952,2647082129846269003
hash_map_fixed_get,96011,1,True,neon,std_control,1.787710125,214200560,918814337952,2647082129846269003
hash_map_fixed_get,96011,1,True,neon,unlimited,1.195530291,214200560,918814337952,2647082129846269003
hash_set_build,617,1,True,scalar,std,1.223731792,162944,68113138,5493369367503854109
hash_set_build,617,1,True,scalar,std_control,1.226183375,162944,68113138,5493369367503854109
hash_set_build,617,1,True,scalar,unlimited,4.95142625,162944,68113138,5493369367503854109
hash_set_build,617,1,True,neon,std_control,1.21535825,162944,68113138,5493369367503854109
hash_set_build,617,1,True,neon,unlimited,1.345400375,162944,68113138,5493369367503854109
hash_set_build,617,1,True,neon,std,1.219646958,162944,68113138,5493369367503854109
hash_set_build,6203,1,True,neon,std_control,1.220028625,125120,69877565,16571444928825650321
hash_set_build,6203,1,True,neon,std,1.220110625,125120,69877565,16571444928825650321
hash_set_build,6203,1,True,neon,unlimited,1.347874541,125120,69877565,16571444928825650321
hash_set_build,6203,1,True,scalar,std,1.225851791,125120,69877565,16571444928825650321
hash_set_build,6203,1,True,scalar,unlimited,5.048657542,125120,69877565,16571444928825650321
hash_set_build,6203,1,True,scalar,std_control,1.2285888329999999,125120,69877565,16571444928825650321
hash_set_build,96011,1,True,scalar,unlimited,4.963123041,125120,70546175,2647082129846269003
hash_set_build,96011,1,True,scalar,std,1.217354,125120,70546175,2647082129846269003
hash_set_build,96011,1,True,scalar,std_control,1.213719333,125120,70546175,2647082129846269003
hash_set_build,96011,1,True,neon,std,1.2047554169999999,125120,70546175,2647082129846269003
hash_set_build,96011,1,True,neon,std_control,1.212540875,125120,70546175,2647082129846269003
hash_set_build,96011,1,True,neon,unlimited,1.332132584,125120,70546175,2647082129846269003
hash_set_owned_union,617,1,True,scalar,std,1.320055917,145920,91532880,5493369367503854109
hash_set_owned_union,617,1,True,scalar,std_control,1.328918209,145920,91532880,5493369367503854109
hash_set_owned_union,617,1,True,scalar,unlimited,5.072977042,145920,91532880,5493369367503854109
hash_set_owned_union,617,1,True,neon,std_control,1.324452,145920,91532880,5493369367503854109
hash_set_owned_union,617,1,True,neon,unlimited,1.196064083,145920,91532880,5493369367503854109
hash_set_owned_union,617,1,True,neon,std,1.321723375,145920,91532880,5493369367503854109
hash_set_owned_union,6203,1,True,neon,std_control,1.27223875,99904,83716430,16571444928825650321
hash_set_owned_union,6203,1,True,neon,std,1.259450084,99904,83716430,16571444928825650321
hash_set_owned_union,6203,1,True,neon,unlimited,1.20746925,99904,83716430,16571444928825650321
hash_set_owned_union,6203,1,True,scalar,std,1.268920541,99904,83716430,16571444928825650321
hash_set_owned_union,6203,1,True,scalar,unlimited,4.871784625,99904,83716430,16571444928825650321
hash_set_owned_union,6203,1,True,scalar,std_control,1.267947541,99904,83716430,16571444928825650321
hash_set_owned_union,96011,1,True,scalar,unlimited,4.785461375,97856,82787705,2647082129846269003
hash_set_owned_union,96011,1,True,scalar,std,1.279797541,97856,82787705,2647082129846269003
hash_set_owned_union,96011,1,True,scalar,std_control,1.276280208,97856,82787705,2647082129846269003
hash_set_owned_union,96011,1,True,neon,std,1.28219325,97856,82787705,2647082129846269003
hash_set_owned_union,96011,1,True,neon,std_control,1.280289292,97856,82787705,2647082129846269003
hash_set_owned_union,96011,1,True,neon,unlimited,1.189104416,97856,82787705,2647082129846269003
scenario_graph,617,1,True,scalar,std,1.2251257500000001,33920,16430963293084922598,11842222284389109881
scenario_graph,617,1,True,scalar,std_control,1.228461208,33920,16430963293084922598,11842222284389109881
scenario_graph,617,1,True,scalar,unlimited,2.126719541,33920,16430963293084922598,11842222284389109881
scenario_graph,617,1,True,neon,std_control,1.2170566250000001,33920,16430963293084922598,11842222284389109881
scenario_graph,617,1,True,neon,unlimited,1.202863958,33920,16430963293084922598,11842222284389109881
scenario_graph,617,1,True,neon,std,1.245479917,33920,16430963293084922598,11842222284389109881
scenario_graph,6203,1,True,neon,std_control,1.221102,45568,16657302743251022112,811796717652796592
scenario_graph,6203,1,True,neon,std,1.219431041,45568,16657302743251022112,811796717652796592
scenario_graph,6203,1,True,neon,unlimited,1.19815725,45568,16657302743251022112,811796717652796592
scenario_graph,6203,1,True,scalar,std,1.225902875,45568,16657302743251022112,811796717652796592
scenario_graph,6203,1,True,scalar,unlimited,2.153504042,45568,16657302743251022112,811796717652796592
scenario_graph,6203,1,True,scalar,std_control,1.215597834,45568,16657302743251022112,811796717652796592
scenario_graph,96011,1,True,scalar,unlimited,2.115398917,25280,732450430288553247,14512637301797670405
scenario_graph,96011,1,True,scalar,std,1.209547708,25280,732450430288553247,14512637301797670405
scenario_graph,96011,1,True,scalar,std_control,1.210877417,25280,732450430288553247,14512637301797670405
scenario_graph,96011,1,True,neon,std,1.229679041,25280,732450430288553247,14512637301797670405
scenario_graph,96011,1,True,neon,std_control,1.209123625,25280,732450430288553247,14512637301797670405
scenario_graph,96011,1,True,neon,unlimited,1.215733958,25280,732450430288553247,14512637301797670405
scenario_scheduler,617,1,True,scalar,std,1.205692709,15296,2967014449476403490,11842222284389109881
scenario_scheduler,617,1,True,scalar,std_control,1.208069042,15296,2967014449476403490,11842222284389109881
scenario_scheduler,617,1,True,scalar,unlimited,1.7429092499999999,15296,2967014449476403490,11842222284389109881
scenario_scheduler,617,1,True,neon,std_control,1.214969958,15296,2967014449476403490,11842222284389109881
scenario_scheduler,617,1,True,neon,unlimited,1.196440458,15296,2967014449476403490,11842222284389109881
scenario_scheduler,617,1,True,neon,std,1.209684459,15296,2967014449476403490,11842222284389109881
scenario_scheduler,6203,1,True,neon,std_control,1.2214015,20800,5062920859296507522,811796717652796592
scenario_scheduler,6203,1,True,neon,std,1.217282708,20800,5062920859296507522,811796717652796592
scenario_scheduler,6203,1,True,neon,unlimited,1.203433709,20800,5062920859296507522,811796717652796592
scenario_scheduler,6203,1,True,scalar,std,1.2101600000000001,20800,5062920859296507522,811796717652796592
scenario_scheduler,6203,1,True,scalar,unlimited,1.771658792,20800,5062920859296507522,811796717652796592
scenario_scheduler,6203,1,True,scalar,std_control,1.212475375,20800,5062920859296507522,811796717652796592
scenario_scheduler,96011,1,True,scalar,unlimited,1.743619,11264,18032262979870618832,14512637301797670405
scenario_scheduler,96011,1,True,scalar,std,1.22481425,11264,18032262979870618832,14512637301797670405
scenario_scheduler,96011,1,True,scalar,std_control,1.219911417,11264,18032262979870618832,14512637301797670405
scenario_scheduler,96011,1,True,neon,std,1.228862625,11264,18032262979870618832,14512637301797670405
scenario_scheduler,96011,1,True,neon,std_control,1.2346045,11264,18032262979870618832,14512637301797670405
scenario_scheduler,96011,1,True,neon,unlimited,1.20475075,11264,18032262979870618832,14512637301797670405
```

</details>

## Apple M4 optimization follow-up

### Conditions

| Suite | Previous revision | Candidate |
| --- | --- | --- |
| HashMap and HashSet | `10c0598f025ffc69e424a21947cca7fa85f3f144` | `f79bf47ffd2a32b7042572406434175f9ea7ca31` ([PR #9](https://github.com/shim9610/jzap-std/pull/9)) |
| B+ point lookup | `665c95bba6cc3f94b2c84fbacc1192f1585bd553` | `c4ccb0d220a5b0832b0073dc8ca2f39c4ad48984` ([PR #11](https://github.com/shim9610/jzap-std/pull/11)) |
| B+ range | `25f773789b0e64d2548d1776f1483dae7fc98ade` | `c4ccb0d220a5b0832b0073dc8ca2f39c4ad48984` |

[Requested test matrix](https://github.com/shim9610/jzap-std/pull/8#issuecomment-7038316438)

All runs used Apple M4 Mac16,10, macOS 27.2 (26B123), native `aarch64-apple-darwin`, Rust 1.95.0 (`59807616e`), AC power, and Low Power Mode off. The benchmark processes ran serially under `caffeinate -i`, with CPU affinity unset and compilation idle. Main-suite revisions were built sequentially at the same checkout and Cargo target paths with `--locked --release`. Hash builds used `hash-map,hash-set,owned-budgets,bplus-pool`. B+ point and range used `bplus-pool` and `bplus-bench`, respectively. All seeds were 617, 6203, and 96011.

The native matrix used fresh processes and rotated revision order. Work, corpus digests where available, and checksums matched within each accepted seed group. Each accepted native interval was at least one second. Central times divide total accepted seconds by total operations. Differences use the unrounded times. A/A is the maximum absolute comparator repeat change across the available repeats. The status is **Held** when A/A exceeds 10% or the paired differences cross both directions by more than max(2% of comparator time, 0.1 ns).

The focused HashMap and HashSet suites passed 43 tests per revision. The focused BTree suite passed 24 tests at the point baseline, 26 at the range baseline, and 26 at the B+ candidate.

<details>
<summary>Binary SHA-256</summary>

| Binary snapshot | SHA-256 |
| --- | --- |
| `bin/10c0598f025ffc69e424a21947cca7fa85f3f144-phase/hash_set` | `1ed1702415a74ca4878dd8450058bbf455ca56ed568c9a905caa43f4e093d8b5` |
| `bin/10c0598f025ffc69e424a21947cca7fa85f3f144/hash_map` | `2c875d3f7436c55388c299e0066a98e50a47988b5c36dd322640e420ec692c37` |
| `bin/10c0598f025ffc69e424a21947cca7fa85f3f144/hash_set` | `faf7cbefc57d3d87972040ce0c1311bed6f50469aa037645b2ebd7e09ffa4e81` |
| `bin/10c0598f025ffc69e424a21947cca7fa85f3f144/scenarios` | `38ec7907925fa637fd7abaa4d2f541902e49aa5d6b5cfc94d780bd94f811681e` |
| `bin/25f773789b0e64d2548d1776f1483dae7fc98ade-paired/bplus_range` | `d8b0ef82b20f5f6bee860d0a80515ad972f69d1fb4cb6107e21e69037cccf305` |
| `bin/25f773789b0e64d2548d1776f1483dae7fc98ade-range/bplus_range` | `cacb7d63ad711251f55c5cc455034d4c2b31ab0b6d6a99ceb04e00cc5a8b1324` |
| `bin/665c95bba6cc3f94b2c84fbacc1192f1585bd553-point/bplus` | `fdec4b49768a413a21084dba24182fc1f186b23e9a91fe40be5b2b1bea70e973` |
| `bin/c4ccb0d220a5b0832b0073dc8ca2f39c4ad48984-paired/bplus_range` | `0c0d629ab8eaa9215dc89fad4b796b829a9ce2d49586f77a6c7b2b0514463bfb` |
| `bin/c4ccb0d220a5b0832b0073dc8ca2f39c4ad48984-point/bplus` | `26898e2c2e2989e898e5fd24e0eb6e03e771a1d1e9cbc4cc7bcaf2326f8beb89` |
| `bin/c4ccb0d220a5b0832b0073dc8ca2f39c4ad48984-range/bplus_range` | `26306e78b8e7d81a19ad251eee2b47b5d12479d61f793dddd802ef732f1a9fc4` |
| `bin/f79bf47ffd2a32b7042572406434175f9ea7ca31/hash_map` | `b48814cc9b1a303f6bae916a129acfedcc0a16849d7006e510f67d2464f2bb64` |
| `bin/f79bf47ffd2a32b7042572406434175f9ea7ca31/hash_set` | `5d9e52abb0b5c7f4b39a7ad4fab57db370b239424f40d6c472c8578da673c662` |
| `bin/f79bf47ffd2a32b7042572406434175f9ea7ca31/scenarios` | `30156ce6ef89fab3235f954195d56956d88268709abae3a3a2626436871a47f2` |
| `hosted/target/release/hash-probe-pair` | `a5fedab7e76bd33af17908d47b3c3a8fea276e0ecef77e7119c612bdf0a2dc94` |

</details>

### Hash tables on Apple M4

Mixed workloads use the default hash builders except the two explicitly named `fixed_*` controls. Those two controls preserve the previous report's equal SipHash13 keys. `hash_map_build` and `hash_map_get` are their default-builder counterparts. jzap is unmetered throughout. Remove/reinsert counts one remove plus one insert. Get counts individual lookups. Other rows count a complete workload or one named isolated phase.

The combined `hash_set_shrink` workload times clone, reserve, shrink and drop. Its isolated phases time clone, reserve, and shrink individually, with setup and teardown outside the timer. The previous revision's separate phase binary uses the candidate's benchmark-only phase harness so that both revisions execute the same phase definitions.

Comparator is Rust std. Times and differences are in **ns/op**.

#### Candidate

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `hash_map_remove_cycle` | 13.425 | 14.734 | +1.309 (+9.8%) |
| `hash_map_build` | 12354.825 | 9201.220 | -3153.605 (-25.5%) |
| `hash_map_get` | 4.786 | 5.610 | +0.824 (+17.2%) |
| `hash_map_fixed_build` | 10094.431 | 8417.462 | -1676.969 (-16.6%) |
| `hash_map_fixed_get` | 8.405 | 5.676 | -2.729 (-32.5%) |
| `hash_set_build` | 8978.148 | 8968.806 | -9.342 (-0.1%) |
| `hash_set_owned_union` | 11128.630 | 10714.682 | -413.948 (-3.7%) |
| `hash_set_shrink` | 5688.252 | 5952.767 | +264.515 (+4.7%) |
| `hash_set_phase_clone` | 131.358 | 132.163 | +0.805 (+0.6%) |
| `hash_set_phase_grow` | 2721.677 | 2917.041 | +195.363 (+7.2%) |
| `hash_set_phase_shrink` | 2739.925 | 2945.388 | +205.463 (+7.5%) |
| `scenario_graph` | 34830.100 | 33961.775 | -868.325 (-2.5%) |
| `scenario_scheduler` | 78270.550 | 76395.665 | -1874.885 (-2.4%) |

#### Previous revision rerun

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `hash_map_remove_cycle` | 12.305 | 15.573 | +3.268 (+26.6%) |
| `hash_map_build` | 12362.408 | 10059.399 | -2303.009 (-18.6%) |
| `hash_map_get` | 4.764 | 5.619 | +0.855 (+18.0%) |
| `hash_map_fixed_build` | 10324.219 | 9319.594 | -1004.625 (-9.7%) |
| `hash_map_fixed_get` | 8.488 | 5.668 | -2.820 (-33.2%) |
| `hash_set_build` | 8952.157 | 9792.107 | +839.950 (+9.4%) |
| `hash_set_owned_union` | 11093.485 | 10650.517 | -442.968 (-4.0%) |
| `hash_set_shrink` | 5555.331 | 6474.846 | +919.515 (+16.6%) |
| `hash_set_phase_clone` | 130.985 | 132.586 | +1.600 (+1.2%) |
| `hash_set_phase_grow` | 2732.254 | 3163.996 | +431.742 (+15.8%) |
| `hash_set_phase_shrink` | 2727.740 | 3217.532 | +489.792 (+18.0%) |
| `scenario_graph` | 35400.149 | 35133.406 | -266.743 (-0.8%) |
| `scenario_scheduler` | 78801.499 | 76530.590 | -2270.909 (-2.9%) |

#### Seed spread and comparator repeats

| Condition | Revision | Comparator min–max | jzap min–max | Paired difference min–max | Max A/A | Held reason |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `hash_map_remove_cycle` | candidate | 12.778 to 14.020 | 14.570 to 14.826 | 0.806 to 1.792 | 6.23% | — |
| `hash_map_build` | candidate | 10307.121 to 13866.926 | 7805.763 to 10143.798 | -3723.127 to -2501.358 | 1.23% | — |
| `hash_map_get` | candidate | 4.764 to 4.798 | 5.587 to 5.650 | 0.789 to 0.854 | 0.58% | — |
| `hash_map_fixed_build` | candidate | 8417.451 to 11266.237 | 7125.140 to 9321.633 | -1944.604 to -1292.310 | 2.21% | — |
| `hash_map_fixed_get` | candidate | 7.535 to 9.081 | 5.656 to 5.697 | -3.384 to -1.860 | 5.30% | — |
| `hash_set_build` | candidate | 7598.251 to 9929.965 | 7613.942 to 9894.147 | -35.818 to 15.691 | 0.17% | — |
| `hash_set_owned_union` | candidate | 9014.193 to 12930.717 | 8432.533 to 12572.136 | -581.660 to -217.888 | 0.31% | — |
| `hash_set_shrink` | candidate | 4627.046 to 6434.370 | 4919.497 to 6675.377 | 241.007 to 292.451 | 1.14% | — |
| `hash_set_phase_clone` | candidate | 116.678 to 141.628 | 117.348 to 142.726 | 0.669 to 1.098 | 1.09% | — |
| `hash_set_phase_grow` | candidate | 2257.187 to 3047.542 | 2415.939 to 3252.741 | 158.752 to 234.101 | 0.71% | — |
| `hash_set_phase_shrink` | candidate | 2255.568 to 3081.825 | 2433.338 to 3322.727 | 177.771 to 240.902 | 0.19% | — |
| `scenario_graph` | candidate | 26615.358 to 48360.873 | 25758.751 to 47912.453 | -1212.267 to -448.419 | 0.38% | — |
| `scenario_scheduler` | candidate | 59235.378 to 109963.354 | 57657.972 to 107650.364 | -2312.989 to -1577.406 | 0.77% | — |
| `hash_map_remove_cycle` | previous | 12.037 to 12.679 | 15.111 to 16.223 | 2.741 to 4.016 | 2.12% | — |
| `hash_map_build` | previous | 10669.792 to 13524.552 | 8531.825 to 11096.093 | -2428.459 to -2137.967 | 0.20% | — |
| `hash_map_get` | previous | 4.737 to 4.781 | 5.577 to 5.671 | 0.829 to 0.897 | 0.72% | — |
| `hash_map_fixed_build` | previous | 8694.830 to 11735.846 | 7908.212 to 10307.624 | -1428.222 to -786.617 | 3.51% | — |
| `hash_map_fixed_get` | previous | 8.149 to 9.160 | 5.615 to 5.720 | -3.490 to -2.438 | 3.65% | — |
| `hash_set_build` | previous | 7590.668 to 9911.704 | 8304.878 to 10791.757 | 714.210 to 962.773 | 0.37% | — |
| `hash_set_owned_union` | previous | 8997.023 to 12869.895 | 8564.701 to 12384.260 | -804.835 to -107.582 | 0.30% | — |
| `hash_set_shrink` | previous | 4614.261 to 6199.670 | 5339.541 to 7272.109 | 725.280 to 1072.439 | 1.18% | — |
| `hash_set_phase_clone` | previous | 115.099 to 141.290 | 116.424 to 142.633 | 1.325 to 2.178 | 1.52% | — |
| `hash_set_phase_grow` | previous | 2255.254 to 3051.792 | 2611.042 to 3563.756 | 355.788 to 511.964 | 0.44% | — |
| `hash_set_phase_shrink` | previous | 2256.136 to 3062.976 | 2649.660 to 3635.102 | 393.523 to 572.125 | 0.33% | — |
| `scenario_graph` | previous | 27000.174 to 49301.510 | 26828.916 to 48750.626 | -550.884 to -171.258 | 1.89% | — |
| `scenario_scheduler` | previous | 59926.645 to 110634.988 | 57886.263 to 108362.378 | -2575.753 to -2040.381 | 0.52% | — |

### B+ point lookup on Apple M4

Each batch used 4,096 queries with the report's existing xorshift generator, hit/miss policy and 512, 32,768 and 65,536-key fixtures. `BPLUS_FIXED_ONLY=1` was used for both revisions with common per-fixture batch counts. Independent seed-617 brie-tree repeats, one per revision, provide A/A and are excluded from the central estimates.

Comparator is brie-tree 0.1.2. Times and differences are in **ns/lookup**.

#### Candidate

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `8/2, FixedTree::get` | 5.810 | 7.010 | +1.200 (+20.7%) |
| `8/2, streaming Lookup` | 5.810 | 19.853 | +14.043 (+241.7%) |
| `16/3, FixedTree::get` | 20.258 | 17.636 | -2.621 (-12.9%) |
| `16/3, streaming Lookup` | 20.258 | 51.332 | +31.074 (+153.4%) |
| `32/2, FixedTree::get` | 13.950 | 13.292 | -0.658 (-4.7%) |
| `32/2, streaming Lookup` | 13.950 | 40.981 | +27.031 (+193.8%) |

#### Previous revision rerun

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `8/2, FixedTree::get` | 5.666 | 6.968 | +1.302 (+23.0%) |
| `8/2, streaming Lookup` | 5.666 | 22.715 | +17.048 (+300.9%) |
| `16/3, FixedTree::get` | 19.552 | 18.289 | -1.262 (-6.5%) |
| `16/3, streaming Lookup` | 19.552 | 50.611 | +31.059 (+158.9%) |
| `32/2, FixedTree::get` | 13.451 | 12.937 | -0.513 (-3.8%) |
| `32/2, streaming Lookup` | 13.451 | 40.596 | +27.145 (+201.8%) |

#### Seed spread and comparator repeats

| Condition | Revision | Comparator min–max | jzap min–max | Paired difference min–max | Max A/A | Held reason |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `8/2, FixedTree::get` | candidate | 5.776 to 5.875 | 6.912 to 7.097 | 1.037 to 1.322 | 3.87% | — |
| `8/2, streaming Lookup` | candidate | 5.776 to 5.875 | 19.320 to 20.409 | 13.541 to 14.534 | 3.87% | — |
| `16/3, FixedTree::get` | candidate | 20.113 to 20.462 | 17.283 to 18.029 | -2.914 to -2.433 | 2.45% | — |
| `16/3, streaming Lookup` | candidate | 20.113 to 20.462 | 50.966 to 51.570 | 30.504 to 31.457 | 2.45% | — |
| `32/2, FixedTree::get` | candidate | 13.621 to 14.116 | 13.142 to 13.461 | -0.975 to -0.348 | 4.11% | — |
| `32/2, streaming Lookup` | candidate | 13.621 to 14.116 | 40.709 to 41.299 | 26.818 to 27.187 | 4.11% | — |
| `8/2, FixedTree::get` | previous | 5.602 to 5.700 | 6.795 to 7.190 | 1.097 to 1.490 | 1.75% | — |
| `8/2, streaming Lookup` | previous | 5.602 to 5.700 | 22.549 to 22.975 | 16.920 to 17.278 | 1.75% | — |
| `16/3, FixedTree::get` | previous | 18.875 to 20.024 | 17.858 to 19.009 | -1.897 to -0.875 | 0.79% | — |
| `16/3, streaming Lookup` | previous | 18.875 to 20.024 | 49.945 to 50.967 | 30.190 to 32.092 | 0.79% | — |
| `32/2, FixedTree::get` | previous | 13.188 to 13.641 | 12.734 to 13.091 | -0.654 to -0.431 | 1.25% | — |
| `32/2, streaming Lookup` | previous | 13.188 to 13.641 | 40.375 to 40.805 | 26.966 to 27.282 | 1.25% | — |

### B+ fixed range on Apple M4

The existing bounds, query generator and 4,096-query batches were retained. Timing includes range creation, all returned items, and the exhaustion check. Fixtures were built before timing. A benchmark-only environment override set the same batch counts across both revisions for each paired seed and condition. Each process includes the brie-tree A/A control.

Comparator is brie-tree 0.1.2. Times and differences are in **ns/range**.

#### Candidate

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `8/2, 0 returned` | 4.435 | 7.641 | +3.206 (+72.3%) |
| `8/2, 1 returned` | 10.046 | 9.422 | -0.624 (-6.2%) |
| `8/2, 32 returned` | 47.557 | 38.994 | -8.563 (-18.0%) |
| `8/2, 256 returned` | 169.243 | 167.975 | -1.267 (-0.7%) |
| `16/3, 0 returned` | 14.743 | 18.617 | +3.874 (+26.3%) |
| `16/3, 1 returned` | 26.574 | 20.743 | -5.831 (-21.9%) |
| `16/3, 32 returned` | 100.772 | 67.906 | -32.866 (-32.6%) |
| `16/3, 256 returned` | 289.509 | 204.489 | -85.020 (-29.4%) |
| `32/2, 0 returned` | 11.083 | 14.506 | +3.423 (+30.9%) |
| `32/2, 1 returned` | 20.844 | 16.878 | -3.965 (-19.0%) |
| `32/2, 32 returned` | 88.049 | 59.286 | -28.763 (-32.7%) |
| `32/2, 256 returned` | 267.475 | 181.940 | -85.535 (-32.0%) |

#### Previous revision rerun

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `8/2, 0 returned` | 4.440 | 9.896 | +5.456 (+122.9%) |
| `8/2, 1 returned` | 10.042 | 11.395 | +1.353 (+13.5%) |
| `8/2, 32 returned` | 47.592 | 41.502 | -6.090 (-12.8%) |
| `8/2, 256 returned` | 169.260 | 170.781 | +1.521 (+0.9%) |
| `16/3, 0 returned` | 14.770 | 18.581 | +3.811 (+25.8%) |
| `16/3, 1 returned` | 26.597 | 21.204 | -5.393 (-20.3%) |
| `16/3, 32 returned` | 101.081 | 67.900 | -33.181 (-32.8%) |
| `16/3, 256 returned` | 289.732 | 207.277 | -82.455 (-28.5%) |
| `32/2, 0 returned` | 11.113 | 14.653 | +3.540 (+31.9%) |
| `32/2, 1 returned` | 20.836 | 16.836 | -4.000 (-19.2%) |
| `32/2, 32 returned` | 88.292 | 59.564 | -28.728 (-32.5%) |
| `32/2, 256 returned` | 265.496 | 182.153 | -83.343 (-31.4%) |

#### Seed spread and comparator repeats

| Condition | Revision | Comparator min–max | jzap min–max | Paired difference min–max | Max A/A | Held reason |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `8/2, 0 returned` | candidate | 4.432 to 4.438 | 7.629 to 7.661 | 3.191 to 3.230 | 1.99% | — |
| `8/2, 1 returned` | candidate | 9.968 to 10.112 | 9.369 to 9.487 | -0.744 to -0.481 | 0.29% | — |
| `8/2, 32 returned` | candidate | 47.359 to 47.682 | 38.833 to 39.114 | -8.797 to -8.323 | 0.13% | — |
| `8/2, 256 returned` | candidate | 169.152 to 169.302 | 167.876 to 168.031 | -1.283 to -1.243 | 0.10% | — |
| `16/3, 0 returned` | candidate | 14.660 to 14.798 | 18.577 to 18.648 | 3.805 to 3.966 | 1.20% | — |
| `16/3, 1 returned` | candidate | 26.400 to 26.699 | 20.646 to 20.832 | -5.874 to -5.754 | 0.95% | — |
| `16/3, 32 returned` | candidate | 100.394 to 101.147 | 67.419 to 68.185 | -33.729 to -32.278 | 1.21% | — |
| `16/3, 256 returned` | candidate | 287.766 to 290.940 | 204.183 to 205.075 | -85.865 to -83.558 | 0.91% | — |
| `32/2, 0 returned` | candidate | 11.047 to 11.108 | 14.485 to 14.523 | 3.415 to 3.438 | 0.52% | — |
| `32/2, 1 returned` | candidate | 20.787 to 20.910 | 16.848 to 16.896 | -4.019 to -3.891 | 0.36% | — |
| `32/2, 32 returned` | candidate | 87.454 to 88.449 | 58.446 to 60.406 | -30.003 to -27.837 | 1.46% | — |
| `32/2, 256 returned` | candidate | 266.124 to 269.973 | 181.465 to 182.843 | -87.130 to -84.659 | 1.79% | — |
| `8/2, 0 returned` | previous | 4.435 to 4.446 | 9.866 to 9.954 | 5.428 to 5.509 | 1.83% | — |
| `8/2, 1 returned` | previous | 9.971 to 10.094 | 11.361 to 11.426 | 1.267 to 1.455 | 6.17% | — |
| `8/2, 32 returned` | previous | 47.380 to 47.710 | 40.956 to 41.957 | -6.425 to -5.728 | 0.35% | — |
| `8/2, 256 returned` | previous | 169.133 to 169.417 | 170.656 to 170.942 | 1.328 to 1.711 | 0.08% | — |
| `16/3, 0 returned` | previous | 14.765 to 14.779 | 18.520 to 18.649 | 3.741 to 3.884 | 0.58% | — |
| `16/3, 1 returned` | previous | 26.427 to 26.686 | 21.079 to 21.398 | -5.552 to -5.280 | 1.01% | — |
| `16/3, 32 returned` | previous | 100.537 to 102.078 | 67.745 to 68.076 | -34.333 to -32.460 | 1.46% | — |
| `16/3, 256 returned` | previous | 289.129 to 290.198 | 206.219 to 208.322 | -82.910 to -81.547 | 1.05% | — |
| `32/2, 0 returned` | previous | 11.083 to 11.131 | 14.550 to 14.715 | 3.424 to 3.611 | 0.64% | — |
| `32/2, 1 returned` | previous | 20.754 to 20.899 | 16.813 to 16.853 | -4.055 to -3.941 | 0.49% | — |
| `32/2, 32 returned` | previous | 88.040 to 88.656 | 59.284 to 59.908 | -28.896 to -28.539 | 2.28% | — |
| `32/2, 256 returned` | previous | 264.144 to 267.609 | 181.794 to 182.367 | -85.815 to -81.846 | 1.00% | — |

### Fixed-key hosted protocol on Apple M4

This separate comparison uses the harness from the [hosted macOS run](https://github.com/shim9610/jzap-std/actions/runs/44704153767) with the pinned revisions above. It uses one native process per seed, the same zero-key builder for all implementations, and shuffled/interleaved batches. Each path accumulated at least 1.12 timed seconds per seed and workload. Batch samples and the default-builder campaign use separate protocols.

Comparator is Rust std with the same fixed-key builder. Times and differences are in **ns/op**.

#### Candidate

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `remove_cycle` | 20.548 | 16.464 | -4.084 (-19.9%) |
| `shrink_lifecycle` | 24321.430 | 25309.458 | +988.028 (+4.1%) |
| `clone` | 495.084 | 449.139 | -45.945 (-9.3%) |
| `grow` | 11702.415 | 12216.668 | +514.253 (+4.4%) |
| `shrink` | 11812.485 | 12534.780 | +722.295 (+6.1%) |

#### Previous revision rerun

| Condition | Comparator time | jzap time | jzap − comparator |
| --- | ---: | ---: | ---: |
| `remove_cycle` | 20.548 | 17.851 | -2.697 (-13.1%) |
| `shrink_lifecycle` | 24321.430 | 27928.479 | +3607.049 (+14.8%) |
| `clone` | 495.084 | 450.391 | -44.693 (-9.0%) |
| `grow` | 11702.415 | 13525.289 | +1822.875 (+15.6%) |
| `shrink` | 11812.485 | 13836.355 | +2023.870 (+17.1%) |

#### Seed spread and comparator repeats

| Condition | Revision | Comparator min–max | jzap min–max | Paired difference min–max | Max A/A | Held reason |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `remove_cycle` | candidate | 19.936 to 20.903 | 15.763 to 16.990 | -4.172 to -3.913 | 0.17% | — |
| `shrink_lifecycle` | candidate | 21838.598 to 26578.956 | 22727.664 to 27665.031 | 889.066 to 1086.075 | 0.12% | — |
| `clone` | candidate | 461.476 to 535.126 | 416.200 to 483.558 | -51.569 to -41.414 | 0.79% | — |
| `grow` | candidate | 10581.484 to 12746.478 | 11013.203 to 13316.982 | 431.719 to 570.504 | 0.10% | — |
| `shrink` | candidate | 10624.617 to 12911.272 | 11264.637 to 13725.173 | 640.020 to 813.901 | 0.10% | — |
| `remove_cycle` | previous | 19.936 to 20.903 | 17.113 to 18.454 | -2.823 to -2.449 | 0.17% | — |
| `shrink_lifecycle` | previous | 21838.598 to 26578.956 | 25081.827 to 30506.480 | 3243.229 to 3927.524 | 0.12% | — |
| `clone` | previous | 461.476 to 535.126 | 418.764 to 485.155 | -49.971 to -41.910 | 0.79% | — |
| `grow` | previous | 10581.484 to 12746.478 | 12194.408 to 14745.510 | 1612.925 to 1999.032 | 0.10% | — |
| `shrink` | previous | 10624.617 to 12911.272 | 12408.626 to 15144.903 | 1784.009 to 2233.631 | 0.10% | — |

### Timing records

Native records include both builds and all recorded timing attempts. `accepted=false` identifies intervals from rejected attempts. Point group 4 is the independent A/A pair. The fixed-key file reports accumulated seed totals from the interleaved batches.

The native matrix has 546 accepted intervals across the two builds, including comparator repeats and the packed point control.

<details>
<summary>Hash-table timing records</summary>

```csv
condition,seed,attempt,accepted,revision,implementation,seconds,operations,checksum,corpus_digest
hash_map_remove_cycle,617,1,True,previous,std,1.238479792,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,617,1,True,previous,std_control,1.236901042,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,617,1,True,previous,unlimited,1.554852625,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,617,1,True,candidate,std_control,1.232868542,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,617,1,True,candidate,unlimited,1.499136958,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,617,1,True,candidate,std,1.314709416,102892038,929760769620,5493369367503854109
hash_map_remove_cycle,6203,1,True,candidate,std_control,1.2728975,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,6203,1,True,candidate,std,1.342654834,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,6203,1,True,candidate,unlimited,1.471483417,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,6203,1,True,previous,std,1.25943125,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,6203,1,True,previous,unlimited,1.531688167,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,6203,1,True,previous,std_control,1.236589458,99329797,904377901266,16571444928825650321
hash_map_remove_cycle,96011,1,True,previous,unlimited,1.564193125,96419120,827180563008,2647082129846269003
hash_map_remove_cycle,96011,1,True,previous,std,1.1769325,96419120,827180563008,2647082129846269003
hash_map_remove_cycle,96011,1,True,previous,std_control,1.201857833,96419120,827180563008,2647082129846269003
hash_map_remove_cycle,96011,1,True,candidate,std,1.351758083,96419120,827180563008,2647082129846269003
hash_map_remove_cycle,96011,1,True,candidate,std_control,1.395693125,96419120,827180563008,2647082129846269003
hash_map_remove_cycle,96011,1,True,candidate,unlimited,1.429519,96419120,827180563008,2647082129846269003
hash_map_build,617,1,True,previous,std,1.623857,152192,63618634,5493369367503854109
hash_map_build,617,1,True,previous,std_control,1.623829834,152192,63618634,5493369367503854109
hash_map_build,617,1,True,previous,unlimited,1.298475584,152192,63618634,5493369367503854109
hash_map_build,617,1,True,candidate,std_control,1.564889875,152192,63618634,5493369367503854109
hash_map_build,617,1,True,candidate,unlimited,1.1879746660000001,152192,63618634,5493369367503854109
hash_map_build,617,1,True,candidate,std,1.568661333,152192,63618634,5493369367503854109
hash_map_build,6203,1,True,candidate,std_control,1.6059255000000001,117248,65481176,16571444928825650321
hash_map_build,6203,1,True,candidate,std,1.625869292,117248,65481176,16571444928825650321
hash_map_build,6203,1,True,candidate,unlimited,1.189340042,117248,65481176,16571444928825650321
hash_map_build,6203,1,True,previous,std,1.5857267080000002,117248,65481176,16571444928825650321
hash_map_build,6203,1,True,previous,unlimited,1.30099475,117248,65481176,16571444928825650321
hash_map_build,6203,1,True,previous,std_control,1.5888315419999999,117248,65481176,16571444928825650321
hash_map_build,96011,1,True,previous,unlimited,1.3038985410000001,118592,66865505,2647082129846269003
hash_map_build,96011,1,True,previous,std,1.5874262080000001,118592,66865505,2647082129846269003
hash_map_build,96011,1,True,previous,std_control,1.589290458,118592,66865505,2647082129846269003
hash_map_build,96011,1,True,candidate,std,1.599536834,118592,66865505,2647082129846269003
hash_map_build,96011,1,True,candidate,std_control,1.60441425,118592,66865505,2647082129846269003
hash_map_build,96011,1,True,candidate,unlimited,1.19305325,118592,66865505,2647082129846269003
hash_map_get,617,1,True,previous,std,1.217600708,257042824,1161354845880,5493369367503854109
hash_map_get,617,1,True,previous,std_control,1.220885583,257042824,1161354845880,5493369367503854109
hash_map_get,617,1,True,previous,unlimited,1.433653458,257042824,1161354845880,5493369367503854109
hash_map_get,617,1,True,candidate,std_control,1.217368458,257042824,1161354845880,5493369367503854109
hash_map_get,617,1,True,candidate,unlimited,1.437775959,257042824,1161354845880,5493369367503854109
hash_map_get,617,1,True,candidate,std,1.22448725,257042824,1161354845880,5493369367503854109
hash_map_get,6203,1,True,candidate,std_control,1.243165292,258922292,1178717797188,16571444928825650321
hash_map_get,6203,1,True,candidate,std,1.242431,258922292,1178717797188,16571444928825650321
hash_map_get,6203,1,True,candidate,unlimited,1.446651375,258922292,1178717797188,16571444928825650321
hash_map_get,6203,1,True,previous,std,1.237815083,258922292,1178717797188,16571444928825650321
hash_map_get,6203,1,True,previous,unlimited,1.452482583,258922292,1178717797188,16571444928825650321
hash_map_get,6203,1,True,previous,std_control,1.238498625,258922292,1178717797188,16571444928825650321
hash_map_get,96011,1,True,previous,unlimited,1.434513125,252955850,1085055341820,2647082129846269003
hash_map_get,96011,1,True,previous,std,1.20769975,252955850,1085055341820,2647082129846269003
hash_map_get,96011,1,True,previous,std_control,1.216360916,252955850,1085055341820,2647082129846269003
hash_map_get,96011,1,True,candidate,std,1.21319925,252955850,1085055341820,2647082129846269003
hash_map_get,96011,1,True,candidate,std_control,1.212272709,252955850,1085055341820,2647082129846269003
hash_map_get,96011,1,True,candidate,unlimited,1.429254833,252955850,1085055341820,2647082129846269003
hash_map_fixed_build,617,1,True,previous,std,1.475756041,169728,70948956,5493369367503854109
hash_map_fixed_build,617,1,True,previous,std_control,1.477600375,169728,70948956,5493369367503854109
hash_map_fixed_build,617,1,True,previous,unlimited,1.342245042,169728,70948956,5493369367503854109
hash_map_fixed_build,617,1,True,candidate,std_control,1.46021325,169728,70948956,5493369367503854109
hash_map_fixed_build,617,1,True,candidate,unlimited,1.209335791,169728,70948956,5493369367503854109
hash_map_fixed_build,617,1,True,candidate,std,1.428677042,169728,70948956,5493369367503854109
hash_map_fixed_build,6203,1,True,candidate,std_control,1.469739125,129152,72129374,16571444928825650321
hash_map_fixed_build,6203,1,True,candidate,std,1.455057083,129152,72129374,16571444928825650321
hash_map_fixed_build,6203,1,True,candidate,unlimited,1.203907583,129152,72129374,16571444928825650321
hash_map_fixed_build,6203,1,True,previous,std,1.515708,129152,72129374,16571444928825650321
hash_map_fixed_build,6203,1,True,previous,unlimited,1.33125025,129152,72129374,16571444928825650321
hash_map_fixed_build,6203,1,True,previous,std_control,1.500571625,129152,72129374,16571444928825650321
hash_map_fixed_build,96011,1,True,previous,unlimited,1.340640208,131840,74335100,2647082129846269003
hash_map_fixed_build,96011,1,True,previous,std,1.4553835,131840,74335100,2647082129846269003
hash_map_fixed_build,96011,1,True,previous,std_control,1.506401083,131840,74335100,2647082129846269003
hash_map_fixed_build,96011,1,True,candidate,std,1.464139167,131840,74335100,2647082129846269003
hash_map_fixed_build,96011,1,True,candidate,std_control,1.466385542,131840,74335100,2647082129846269003
hash_map_fixed_build,96011,1,True,candidate,unlimited,1.212325917,131840,74335100,2647082129846269003
hash_map_fixed_get,617,1,True,previous,std,1.724052792,211562724,955869496380,5493369367503854109
hash_map_fixed_get,617,1,True,previous,std_control,1.7059940839999999,211562724,955869496380,5493369367503854109
hash_map_fixed_get,617,1,True,previous,unlimited,1.187940791,211562724,955869496380,5493369367503854109
hash_map_fixed_get,617,1,True,candidate,std_control,1.770258375,211562724,955869496380,5493369367503854109
hash_map_fixed_get,617,1,True,candidate,unlimited,1.196519334,211562724,955869496380,5493369367503854109
hash_map_fixed_get,617,1,True,candidate,std,1.820495958,211562724,955869496380,5493369367503854109
hash_map_fixed_get,6203,1,True,candidate,std_control,1.833139208,210955186,960352352154,16571444928825650321
hash_map_fixed_get,6203,1,True,candidate,std,1.915631125,210955186,960352352154,16571444928825650321
hash_map_fixed_get,6203,1,True,candidate,unlimited,1.201738834,210955186,960352352154,16571444928825650321
hash_map_fixed_get,6203,1,True,previous,std,1.932366875,210955186,960352352154,16571444928825650321
hash_map_fixed_get,6203,1,True,previous,unlimited,1.19604525,210955186,960352352154,16571444928825650321
hash_map_fixed_get,6203,1,True,previous,std_control,1.954959375,210955186,960352352154,16571444928825650321
hash_map_fixed_get,96011,1,True,previous,unlimited,1.215660958,212540650,911694145980,2647082129846269003
hash_map_fixed_get,96011,1,True,previous,std,1.733788833,212540650,911694145980,2647082129846269003
hash_map_fixed_get,96011,1,True,previous,std_control,1.6704945420000001,212540650,911694145980,2647082129846269003
hash_map_fixed_get,96011,1,True,candidate,std,1.601423958,212540650,911694145980,2647082129846269003
hash_map_fixed_get,96011,1,True,candidate,std_control,1.6862775,212540650,911694145980,2647082129846269003
hash_map_fixed_get,96011,1,True,candidate,unlimited,1.206101167,212540650,911694145980,2647082129846269003
hash_set_build,617,1,True,previous,std,1.212563666,159744,66775488,5493369367503854109
hash_set_build,617,1,True,previous,std_control,1.210316416,159744,66775488,5493369367503854109
hash_set_build,617,1,True,previous,unlimited,1.3266544169999999,159744,66775488,5493369367503854109
hash_set_build,617,1,True,candidate,std_control,1.213338958,159744,66775488,5493369367503854109
hash_set_build,617,1,True,candidate,unlimited,1.216281625,159744,66775488,5493369367503854109
hash_set_build,617,1,True,candidate,std,1.2137750409999999,159744,66775488,5493369367503854109
hash_set_build,6203,1,True,candidate,std_control,1.213025042,122368,68340616,16571444928825650321
hash_set_build,6203,1,True,candidate,std,1.215109916,122368,68340616,16571444928825650321
hash_set_build,6203,1,True,candidate,unlimited,1.210726958,122368,68340616,16571444928825650321
hash_set_build,6203,1,True,previous,std,1.212875417,122368,68340616,16571444928825650321
hash_set_build,6203,1,True,previous,unlimited,1.320565708,122368,68340616,16571444928825650321
hash_set_build,6203,1,True,previous,std_control,1.214793,122368,68340616,16571444928825650321
hash_set_build,96011,1,True,previous,unlimited,1.325398542,123584,69680135,2647082129846269003
hash_set_build,96011,1,True,previous,std,1.206415167,123584,69680135,2647082129846269003
hash_set_build,96011,1,True,previous,std_control,1.210914875,123584,69680135,2647082129846269003
hash_set_build,96011,1,True,candidate,std,1.213513875,123584,69680135,2647082129846269003
hash_set_build,96011,1,True,candidate,std_control,1.211587416,123584,69680135,2647082129846269003
hash_set_build,96011,1,True,candidate,unlimited,1.211600041,123584,69680135,2647082129846269003
hash_set_owned_union,617,1,True,previous,std,1.34393925,149376,93700764,5493369367503854109
hash_set_owned_union,617,1,True,previous,std_control,1.339953625,149376,93700764,5493369367503854109
hash_set_owned_union,617,1,True,previous,unlimited,1.279360709,149376,93700764,5493369367503854109
hash_set_owned_union,617,1,True,candidate,std_control,1.3423734999999999,149376,93700764,5493369367503854109
hash_set_owned_union,617,1,True,candidate,unlimited,1.259618041,149376,93700764,5493369367503854109
hash_set_owned_union,617,1,True,candidate,std,1.346504125,149376,93700764,5493369367503854109
hash_set_owned_union,6203,1,True,candidate,std_control,1.253034959,100288,84038210,16571444928825650321
hash_set_owned_union,6203,1,True,candidate,std,1.256491792,100288,84038210,16571444928825650321
hash_set_owned_union,6203,1,True,candidate,unlimited,1.23464025,100288,84038210,16571444928825650321
hash_set_owned_union,6203,1,True,previous,std,1.252781833,100288,84038210,16571444928825650321
hash_set_owned_union,6203,1,True,previous,unlimited,1.2419926669999999,100288,84038210,16571444928825650321
hash_set_owned_union,6203,1,True,previous,std_control,1.25164,100288,84038210,16571444928825650321
hash_set_owned_union,96011,1,True,previous,unlimited,1.174461208,97344,82354545,2647082129846269003
hash_set_owned_union,96011,1,True,previous,std,1.252807084,97344,82354545,2647082129846269003
hash_set_owned_union,96011,1,True,previous,std_control,1.255933792,97344,82354545,2647082129846269003
hash_set_owned_union,96011,1,True,candidate,std,1.258727708,97344,82354545,2647082129846269003
hash_set_owned_union,96011,1,True,candidate,std_control,1.2577975000000001,97344,82354545,2647082129846269003
hash_set_owned_union,96011,1,True,candidate,unlimited,1.223822,97344,82354545,2647082129846269003
hash_set_shrink,617,1,True,previous,std,1.190700917,258048,107868096,5493369367503854109
hash_set_shrink,617,1,True,previous,std_control,1.186130791,258048,107868096,5493369367503854109
hash_set_shrink,617,1,True,previous,unlimited,1.377858,258048,107868096,5493369367503854109
hash_set_shrink,617,1,True,candidate,std_control,1.207557,258048,107868096,5493369367503854109
hash_set_shrink,617,1,True,candidate,unlimited,1.269466459,258048,107868096,5493369367503854109
hash_set_shrink,617,1,True,candidate,std,1.194,258048,107868096,5493369367503854109
hash_set_shrink,6203,1,True,candidate,std_control,1.243097292,197120,110088440,16571444928825650321
hash_set_shrink,6203,1,True,candidate,std,1.251330125,197120,110088440,16571444928825650321
hash_set_shrink,6203,1,True,candidate,unlimited,1.30079275,197120,110088440,16571444928825650321
hash_set_shrink,6203,1,True,previous,std,1.213741,197120,110088440,16571444928825650321
hash_set_shrink,6203,1,True,previous,unlimited,1.4156486670000001,197120,110088440,16571444928825650321
hash_set_shrink,6203,1,True,previous,std_control,1.217011916,197120,110088440,16571444928825650321
hash_set_shrink,96011,1,True,previous,unlimited,1.4013645000000001,192704,108651935,2647082129846269003
hash_set_shrink,96011,1,True,previous,std,1.194701292,192704,108651935,2647082129846269003
hash_set_shrink,96011,1,True,previous,std_control,1.208853875,192704,108651935,2647082129846269003
hash_set_shrink,96011,1,True,candidate,std,1.239928833,192704,108651935,2647082129846269003
hash_set_shrink,96011,1,True,candidate,std_control,1.232504292,192704,108651935,2647082129846269003
hash_set_shrink,96011,1,True,candidate,unlimited,1.286371833,192704,108651935,2647082129846269003
hash_set_phase_clone,617,1,True,previous,std,1.211874368,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,617,1,True,previous,std_control,1.213662956,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,617,1,True,previous,unlimited,1.225827622,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,617,1,True,candidate,std_control,1.215169288,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,617,1,True,candidate,unlimited,1.235548602,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,617,1,True,candidate,std,1.228501734,10528960,13257028342350,5493369367503854109
hash_set_phase_clone,6203,1,True,candidate,std_control,1.200487603,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,6203,1,True,candidate,std,1.196400215,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,6203,1,True,candidate,unlimited,1.205675577,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,6203,1,True,previous,std,1.193544821,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,6203,1,True,previous,unlimited,1.204886596,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,6203,1,True,previous,std_control,1.2116868219999999,8447488,14318177755056,16571444928825650321
hash_set_phase_clone,96011,1,True,previous,unlimited,1.249760367,8782720,14160926729240,2647082129846269003
hash_set_phase_clone,96011,1,True,previous,std,1.230629038,8782720,14160926729240,2647082129846269003
hash_set_phase_clone,96011,1,True,previous,std_control,1.23576261,8782720,14160926729240,2647082129846269003
hash_set_phase_clone,96011,1,True,candidate,std,1.22149329,8782720,14160926729240,2647082129846269003
hash_set_phase_clone,96011,1,True,candidate,std_control,1.23028448,8782720,14160926729240,2647082129846269003
hash_set_phase_clone,96011,1,True,candidate,unlimited,1.227522974,8782720,14160926729240,2647082129846269003
hash_set_phase_grow,617,1,True,previous,std,1.190196624,527744,664483212540,5493369367503854109
hash_set_phase_grow,617,1,True,previous,std_control,1.194372358,527744,664483212540,5493369367503854109
hash_set_phase_grow,617,1,True,previous,unlimited,1.377961718,527744,664483212540,5493369367503854109
hash_set_phase_grow,617,1,True,candidate,std_control,1.1929989029999999,527744,664483212540,5493369367503854109
hash_set_phase_grow,617,1,True,candidate,unlimited,1.2749970529999999,527744,664483212540,5493369367503854109
hash_set_phase_grow,617,1,True,candidate,std,1.191216723,527744,664483212540,5493369367503854109
hash_set_phase_grow,6203,1,True,candidate,std_control,1.207373077,397952,674513828724,16571444928825650321
hash_set_phase_grow,6203,1,True,candidate,std,1.1988216600000001,397952,674513828724,16571444928825650321
hash_set_phase_grow,6203,1,True,candidate,unlimited,1.291982427,397952,674513828724,16571444928825650321
hash_set_phase_grow,6203,1,True,previous,std,1.212145163,397952,674513828724,16571444928825650321
hash_set_phase_grow,6203,1,True,previous,unlimited,1.39218417,397952,674513828724,16571444928825650321
hash_set_phase_grow,6203,1,True,previous,std_control,1.206770546,397952,674513828724,16571444928825650321
hash_set_phase_grow,96011,1,True,previous,unlimited,1.41523868,397120,640301321540,2647082129846269003
hash_set_phase_grow,96011,1,True,previous,std,1.2119277290000001,397120,640301321540,2647082129846269003
hash_set_phase_grow,96011,1,True,previous,std_control,1.214840154,397120,640301321540,2647082129846269003
hash_set_phase_grow,96011,1,True,candidate,std,1.210240017,397120,640301321540,2647082129846269003
hash_set_phase_grow,96011,1,True,candidate,std_control,1.21005731,397120,640301321540,2647082129846269003
hash_set_phase_grow,96011,1,True,candidate,unlimited,1.291728515,397120,640301321540,2647082129846269003
hash_set_phase_shrink,617,1,True,previous,std,1.208278408,535552,674314276320,5493369367503854109
hash_set_phase_shrink,617,1,True,previous,std_control,1.20429422,535552,674314276320,5493369367503854109
hash_set_phase_shrink,617,1,True,previous,unlimited,1.419030466,535552,674314276320,5493369367503854109
hash_set_phase_shrink,617,1,True,candidate,std_control,1.210308504,535552,674314276320,5493369367503854109
hash_set_phase_shrink,617,1,True,candidate,unlimited,1.30317924,535552,674314276320,5493369367503854109
hash_set_phase_shrink,617,1,True,candidate,std,1.20797369,535552,674314276320,5493369367503854109
hash_set_phase_shrink,6203,1,True,candidate,std_control,1.208756929,395328,670066246386,16571444928825650321
hash_set_phase_shrink,6203,1,True,candidate,std,1.208586648,395328,670066246386,16571444928825650321
hash_set_phase_shrink,6203,1,True,candidate,unlimited,1.290754871,395328,670066246386,16571444928825650321
hash_set_phase_shrink,6203,1,True,previous,std,1.1995506009999999,395328,670066246386,16571444928825650321
hash_set_phase_shrink,6203,1,True,previous,unlimited,1.412472156,395328,670066246386,16571444928825650321
hash_set_phase_shrink,6203,1,True,previous,std_control,1.19565034,395328,670066246386,16571444928825650321
hash_set_phase_shrink,96011,1,True,previous,unlimited,1.424494536,391872,631839644124,2647082129846269003
hash_set_phase_shrink,96011,1,True,previous,std,1.20029463,391872,631839644124,2647082129846269003
hash_set_phase_shrink,96011,1,True,previous,std_control,1.199398806,391872,631839644124,2647082129846269003
hash_set_phase_shrink,96011,1,True,candidate,std,1.207681106,391872,631839644124,2647082129846269003
hash_set_phase_shrink,96011,1,True,candidate,std_control,1.209739747,391872,631839644124,2647082129846269003
hash_set_phase_shrink,96011,1,True,candidate,unlimited,1.30208368,391872,631839644124,2647082129846269003
scenario_graph,617,1,True,previous,std,1.152763583,31680,3512139330218470069,11842222284389109881
scenario_graph,617,1,True,previous,std_control,1.148905708,31680,3512139330218470069,11842222284389109881
scenario_graph,617,1,True,previous,unlimited,1.1470732080000001,31680,3512139330218470069,11842222284389109881
scenario_graph,617,1,True,candidate,std_control,1.131235875,31680,3512139330218470069,11842222284389109881
scenario_graph,617,1,True,candidate,unlimited,1.0971919589999999,31680,3512139330218470069,11842222284389109881
scenario_graph,617,1,True,candidate,std,1.1355965829999999,31680,3512139330218470069,11842222284389109881
scenario_graph,6203,1,True,candidate,std_control,1.188361416,44608,7937979882356428708,811796717652796592
scenario_graph,6203,1,True,candidate,std,1.187257875,44608,7937979882356428708,811796717652796592
scenario_graph,6203,1,True,candidate,unlimited,1.149046375,44608,7937979882356428708,811796717652796592
scenario_graph,6203,1,True,previous,std,1.20442375,44608,7937979882356428708,811796717652796592
scenario_graph,6203,1,True,previous,unlimited,1.196784292,44608,7937979882356428708,811796717652796592
scenario_graph,6203,1,True,previous,std_control,1.227141833,44608,7937979882356428708,811796717652796592
scenario_graph,96011,1,True,previous,unlimited,1.204335459,24704,13465030375225542138,14512637301797670405
scenario_graph,96011,1,True,previous,std,1.2179445,24704,13465030375225542138,14512637301797670405
scenario_graph,96011,1,True,previous,std_control,1.223992041,24704,13465030375225542138,14512637301797670405
scenario_graph,96011,1,True,candidate,std,1.194707,24704,13465030375225542138,14512637301797670405
scenario_graph,96011,1,True,candidate,std_control,1.19730075,24704,13465030375225542138,14512637301797670405
scenario_graph,96011,1,True,candidate,unlimited,1.18362925,24704,13465030375225542138,14512637301797670405
scenario_scheduler,617,1,True,previous,std,1.229569125,15232,15226701869017584676,11842222284389109881
scenario_scheduler,617,1,True,previous,std_control,1.231588542,15232,15226701869017584676,11842222284389109881
scenario_scheduler,617,1,True,previous,unlimited,1.19033525,15232,15226701869017584676,11842222284389109881
scenario_scheduler,617,1,True,candidate,std_control,1.2243699160000001,15232,15226701869017584676,11842222284389109881
scenario_scheduler,617,1,True,candidate,unlimited,1.196557833,15232,15226701869017584676,11842222284389109881
scenario_scheduler,617,1,True,candidate,std,1.226281625,15232,15226701869017584676,11842222284389109881
scenario_scheduler,6203,1,True,candidate,std_control,1.202464875,20224,15025856728178450968,811796717652796592
scenario_scheduler,6203,1,True,candidate,std,1.1979762919999999,20224,15025856728178450968,811796717652796592
scenario_scheduler,6203,1,True,candidate,unlimited,1.166074833,20224,15025856728178450968,811796717652796592
scenario_scheduler,6203,1,True,previous,std,1.211956459,20224,15025856728178450968,811796717652796592
scenario_scheduler,6203,1,True,previous,unlimited,1.170691791,20224,15025856728178450968,811796717652796592
scenario_scheduler,6203,1,True,previous,std_control,1.208426917,20224,15025856728178450968,811796717652796592
scenario_scheduler,96011,1,True,previous,unlimited,1.1997882500000001,11072,12484342567228065211,14512637301797670405
scenario_scheduler,96011,1,True,previous,std,1.224950583,11072,12484342567228065211,14512637301797670405
scenario_scheduler,96011,1,True,previous,std_control,1.231360583,11072,12484342567228065211,14512637301797670405
scenario_scheduler,96011,1,True,candidate,std,1.21751425,11072,12484342567228065211,14512637301797670405
scenario_scheduler,96011,1,True,candidate,std_control,1.226913291,11072,12484342567228065211,14512637301797670405
scenario_scheduler,96011,1,True,candidate,unlimited,1.191904833,11072,12484342567228065211,14512637301797670405
```

</details>

<details>
<summary>B+ point timing records</summary>

```csv
group,seed,attempt,accepted,revision,width,height,implementation,seconds,lookups,checksum
1,617,1,True,previous,8,2,packed,1.718870333,291717120,230339581560
1,617,1,True,previous,8,2,fixed,2.097537458,291717120,230339581560
1,617,1,True,previous,8,2,jzap,6.598536709,291717120,230339581560
1,617,1,True,previous,8,2,brie,1.662739917,291717120,230339581560
1,617,1,True,previous,32,2,jzap,5.034391792,124690432,6239030303736
1,617,1,True,previous,32,2,brie,1.644457167,124690432,6239030303736
1,617,1,True,previous,32,2,packed,1.578732417,124690432,6239030303736
1,617,1,True,previous,32,2,fixed,1.587776541,124690432,6239030303736
1,617,1,True,previous,16,3,packed,1.945436958,110272512,10799376930720
1,617,1,True,previous,16,3,fixed,2.096164041,110272512,10799376930720
1,617,1,True,previous,16,3,jzap,5.615018709,110272512,10799376930720
1,617,1,True,previous,16,3,brie,2.208127500,110272512,10799376930720
1,617,1,True,candidate,8,2,packed,1.676264959,291717120,230339581560
1,617,1,True,candidate,8,2,fixed,2.016385334,291717120,230339581560
1,617,1,True,candidate,8,2,jzap,5.953735333,291717120,230339581560
1,617,1,True,candidate,8,2,brie,1.713821958,291717120,230339581560
1,617,1,True,candidate,32,2,jzap,5.104086792,124690432,6239030303736
1,617,1,True,candidate,32,2,brie,1.760164792,124690432,6239030303736
1,617,1,True,candidate,32,2,packed,1.559025958,124690432,6239030303736
1,617,1,True,candidate,32,2,fixed,1.638626208,124690432,6239030303736
1,617,1,True,candidate,16,3,packed,1.710756541,110272512,10799376930720
1,617,1,True,candidate,16,3,fixed,1.940380083,110272512,10799376930720
1,617,1,True,candidate,16,3,jzap,5.686719292,110272512,10799376930720
1,617,1,True,candidate,16,3,brie,2.217913292,110272512,10799376930720
2,6203,1,True,candidate,8,2,packed,1.712768667,291717120,217991315520
2,6203,1,True,candidate,8,2,fixed,2.048167916,291717120,217991315520
2,6203,1,True,candidate,8,2,jzap,5.636112125,291717120,217991315520
2,6203,1,True,candidate,8,2,brie,1.686088125,291717120,217991315520
2,6203,1,True,candidate,32,2,packed,1.555693292,124690432,6195521318772
2,6203,1,True,candidate,32,2,fixed,1.654967000,124690432,6195521318772
2,6203,1,True,candidate,32,2,jzap,5.075962834,124690432,6195521318772
2,6203,1,True,candidate,32,2,brie,1.698395625,124690432,6195521318772
2,6203,1,True,candidate,16,3,brie,2.256411000,110272512,10741305369060
2,6203,1,True,candidate,16,3,packed,1.723148333,110272512,10741305369060
2,6203,1,True,candidate,16,3,fixed,1.988120500,110272512,10741305369060
2,6203,1,True,candidate,16,3,jzap,5.620171834,110272512,10741305369060
2,6203,1,True,previous,8,2,packed,1.691403250,291717120,217991315520
2,6203,1,True,previous,8,2,fixed,1.982122667,291717120,217991315520
2,6203,1,True,previous,8,2,jzap,6.702250500,291717120,217991315520
2,6203,1,True,previous,8,2,brie,1.662078292,291717120,217991315520
2,6203,1,True,previous,32,2,packed,1.549180500,124690432,6195521318772
2,6203,1,True,previous,32,2,fixed,1.619389541,124690432,6195521318772
2,6203,1,True,previous,32,2,jzap,5.063316791,124690432,6195521318772
2,6203,1,True,previous,32,2,brie,1.700920125,124690432,6195521318772
2,6203,1,True,previous,16,3,brie,2.178449375,110272512,10741305369060
2,6203,1,True,previous,16,3,packed,1.854273125,110272512,10741305369060
2,6203,1,True,previous,16,3,fixed,1.969267083,110272512,10741305369060
2,6203,1,True,previous,16,3,jzap,5.507601667,110272512,10741305369060
3,96011,1,True,previous,8,2,jzap,6.577971250,291717120,222567485400
3,96011,1,True,previous,8,2,brie,1.634072417,291717120,222567485400
3,96011,1,True,previous,8,2,packed,1.652735750,291717120,222567485400
3,96011,1,True,previous,8,2,fixed,2.018451042,291717120,222567485400
3,96011,1,True,previous,32,2,brie,1.686154500,124690432,6157506505968
3,96011,1,True,previous,32,2,packed,1.539150417,124690432,6157506505968
3,96011,1,True,previous,32,2,fixed,1.632358292,124690432,6157506505968
3,96011,1,True,previous,32,2,jzap,5.087939042,124690432,6157506505968
3,96011,1,True,previous,16,3,packed,1.642420000,110272512,10698634914408
3,96011,1,True,previous,16,3,fixed,1.984912417,110272512,10698634914408
3,96011,1,True,previous,16,3,jzap,5.620223334,110272512,10698634914408
3,96011,1,True,previous,16,3,brie,2.081406958,110272512,10698634914408
3,96011,1,True,candidate,8,2,jzap,5.784966667,291717120,222567485400
3,96011,1,True,candidate,8,2,brie,1.684815208,291717120,222567485400
3,96011,1,True,candidate,8,2,packed,1.717626916,291717120,222567485400
3,96011,1,True,candidate,8,2,fixed,2.070428458,291717120,222567485400
3,96011,1,True,candidate,32,2,brie,1.759690167,124690432,6157506505968
3,96011,1,True,candidate,32,2,packed,1.607000458,124690432,6157506505968
3,96011,1,True,candidate,32,2,fixed,1.678404292,124690432,6157506505968
3,96011,1,True,candidate,32,2,jzap,5.149587291,124690432,6157506505968
3,96011,1,True,candidate,16,3,packed,1.793730125,110272512,10698634914408
3,96011,1,True,candidate,16,3,fixed,1.905886083,110272512,10698634914408
3,96011,1,True,candidate,16,3,jzap,5.674614750,110272512,10698634914408
3,96011,1,True,candidate,16,3,brie,2.227268333,110272512,10698634914408
4,617,1,True,candidate,8,2,packed,1.699307459,291717120,230339581560
4,617,1,True,candidate,8,2,fixed,2.005684916,291717120,230339581560
4,617,1,True,candidate,8,2,jzap,6.011198541,291717120,230339581560
4,617,1,True,candidate,8,2,brie,1.647577875,291717120,230339581560
4,617,1,True,candidate,32,2,jzap,5.051941459,124690432,6239030303736
4,617,1,True,candidate,32,2,brie,1.687856417,124690432,6239030303736
4,617,1,True,candidate,32,2,packed,1.537089875,124690432,6239030303736
4,617,1,True,candidate,32,2,fixed,1.649258542,124690432,6239030303736
4,617,1,True,candidate,16,3,packed,1.852236000,110272512,10799376930720
4,617,1,True,candidate,16,3,fixed,1.881714333,110272512,10799376930720
4,617,1,True,candidate,16,3,jzap,5.715410500,110272512,10799376930720
4,617,1,True,candidate,16,3,brie,2.272245292,110272512,10799376930720
4,617,1,True,previous,8,2,packed,1.832120041,291717120,230339581560
4,617,1,True,previous,8,2,fixed,2.016061875,291717120,230339581560
4,617,1,True,previous,8,2,jzap,6.565349042,291717120,230339581560
4,617,1,True,previous,8,2,brie,1.633569500,291717120,230339581560
4,617,1,True,previous,32,2,jzap,5.027979291,124690432,6239030303736
4,617,1,True,previous,32,2,brie,1.664958000,124690432,6239030303736
4,617,1,True,previous,32,2,packed,1.609055584,124690432,6239030303736
4,617,1,True,previous,32,2,fixed,1.716224375,124690432,6239030303736
4,617,1,True,previous,16,3,packed,1.819319583,110272512,10799376930720
4,617,1,True,previous,16,3,fixed,1.972824250,110272512,10799376930720
4,617,1,True,previous,16,3,jzap,5.615173417,110272512,10799376930720
4,617,1,True,previous,16,3,brie,2.190746584,110272512,10799376930720
```

</details>

<details>
<summary>B+ range timing records</summary>

```csv
group,seed,attempt,accepted,revision,width,height,items_per_range,total_items,implementation,seconds,ranges,checksum
1,617,1,True,previous,8,2,0,0,brie_control,1.511762875,334725120,0
1,617,1,True,previous,8,2,0,0,brie,1.484643167,334725120,0
1,617,1,True,previous,8,2,0,0,jzap,3.302641416,334725120,0
1,617,1,True,previous,8,2,1,139984896,brie,1.395803542,139984896,191059834368
1,617,1,True,previous,8,2,1,139984896,jzap,1.599489750,139984896,191059834368
1,617,1,True,previous,8,2,1,139984896,brie_control,1.481920709,139984896,191059834368
1,617,1,True,previous,8,2,32,1012924416,jzap,1.296410250,31653888,1393294350336
1,617,1,True,previous,8,2,32,1012924416,brie_control,1.503828458,31653888,1393294350336
1,617,1,True,previous,8,2,32,1012924416,brie,1.499774417,31653888,1393294350336
1,617,1,True,previous,8,2,256,2060451840,brie_control,1.361664375,8048640,2810023695360
1,617,1,True,previous,8,2,256,2060451840,brie,1.362072958,8048640,2810023695360
1,617,1,True,previous,8,2,256,2060451840,jzap,1.375847833,8048640,2810023695360
1,617,1,True,candidate,8,2,0,0,brie_control,1.515141834,334725120,0
1,617,1,True,candidate,8,2,0,0,brie,1.485541375,334725120,0
1,617,1,True,candidate,8,2,0,0,jzap,2.553784708,334725120,0
1,617,1,True,candidate,8,2,1,139984896,brie,1.395331542,139984896,191059834368
1,617,1,True,candidate,8,2,1,139984896,jzap,1.327971583,139984896,191059834368
1,617,1,True,candidate,8,2,1,139984896,brie_control,1.396322625,139984896,191059834368
1,617,1,True,candidate,8,2,32,1012924416,jzap,1.235640792,31653888,1393294350336
1,617,1,True,candidate,8,2,32,1012924416,brie_control,1.499846791,31653888,1393294350336
1,617,1,True,candidate,8,2,32,1012924416,brie,1.499084875,31653888,1393294350336
1,617,1,True,candidate,8,2,256,2060451840,brie_control,1.362450542,8048640,2810023695360
1,617,1,True,candidate,8,2,256,2060451840,brie,1.362424500,8048640,2810023695360
1,617,1,True,candidate,8,2,256,2060451840,jzap,1.352420500,8048640,2810023695360
1,617,1,True,previous,16,3,0,0,brie_control,1.485230500,100376576,0
1,617,1,True,previous,16,3,0,0,brie,1.482068042,100376576,0
1,617,1,True,previous,16,3,0,0,jzap,1.871961375,100376576,0
1,617,1,True,previous,16,3,1,64921600,brie,1.715676542,64921600,11170044361000
1,617,1,True,previous,16,3,1,64921600,jzap,1.368477417,64921600,11170044361000
1,617,1,True,previous,16,3,1,64921600,brie_control,1.732960000,64921600,11170044361000
1,617,1,True,previous,16,3,32,663355392,jzap,1.411210542,20729856,118495220315136
1,617,1,True,previous,16,3,32,663355392,brie_control,2.076116833,20729856,118495220315136
1,617,1,True,previous,16,3,32,663355392,brie,2.084111666,20729856,118495220315136
1,617,1,True,previous,16,3,256,1724907520,brie_control,1.936457958,6737920,299414625413120
1,617,1,True,previous,16,3,256,1724907520,brie,1.953116250,6737920,299414625413120
1,617,1,True,previous,16,3,256,1724907520,jzap,1.403658500,6737920,299414625413120
1,617,1,True,candidate,16,3,0,0,brie_control,1.472176083,100376576,0
1,617,1,True,candidate,16,3,0,0,brie,1.485395666,100376576,0
1,617,1,True,candidate,16,3,0,0,jzap,1.871777083,100376576,0
1,617,1,True,candidate,16,3,1,64921600,brie,1.713899333,64921600,11170044361000
1,617,1,True,candidate,16,3,1,64921600,jzap,1.340364875,64921600,11170044361000
1,617,1,True,candidate,16,3,1,64921600,brie_control,1.730186667,64921600,11170044361000
1,617,1,True,candidate,16,3,32,663355392,jzap,1.412030250,20729856,118495220315136
1,617,1,True,candidate,16,3,32,663355392,brie_control,2.106318625,20729856,118495220315136
1,617,1,True,candidate,16,3,32,663355392,brie,2.081147083,20729856,118495220315136
1,617,1,True,candidate,16,3,256,1724907520,brie_control,1.958093333,6737920,299414625413120
1,617,1,True,candidate,16,3,256,1724907520,brie,1.952787500,6737920,299414625413120
1,617,1,True,candidate,16,3,256,1724907520,jzap,1.375766208,6737920,299414625413120
1,617,1,True,previous,32,2,0,0,brie_control,1.536593375,138514432,0
1,617,1,True,previous,32,2,0,0,brie,1.535155500,138514432,0
1,617,1,True,previous,32,2,0,0,jzap,2.035367542,138514432,0
1,617,1,True,previous,32,2,1,83976192,brie,1.742837792,83976192,7359535775448
1,617,1,True,previous,32,2,1,83976192,jzap,1.411853916,83976192,7359535775448
1,617,1,True,previous,32,2,1,83976192,brie_control,1.737741167,83976192,7359535775448
1,617,1,True,previous,32,2,32,748552192,jzap,1.386788292,23392256,64217734792576
1,617,1,True,previous,32,2,32,748552192,brie_control,2.048711334,23392256,64217734792576
1,617,1,True,previous,32,2,32,748552192,brie,2.062727542,23392256,64217734792576
1,617,1,True,previous,32,2,256,2014314496,brie_control,2.092304709,7868416,174974662069248
1,617,1,True,previous,32,2,256,2014314496,brie,2.078398250,7868416,174974662069248
1,617,1,True,previous,32,2,256,2014314496,jzap,1.434397125,7868416,174974662069248
1,617,1,True,candidate,32,2,0,0,brie_control,1.539938542,138514432,0
1,617,1,True,candidate,32,2,0,0,brie,1.536600750,138514432,0
1,617,1,True,candidate,32,2,0,0,jzap,2.009950250,138514432,0
1,617,1,True,candidate,32,2,1,83976192,brie,1.745636459,83976192,7359535775448
1,617,1,True,candidate,32,2,1,83976192,jzap,1.418895916,83976192,7359535775448
1,617,1,True,candidate,32,2,1,83976192,brie_control,1.739374500,83976192,7359535775448
1,617,1,True,candidate,32,2,32,748552192,jzap,1.413028459,23392256,64217734792576
1,617,1,True,candidate,32,2,32,748552192,brie_control,2.034062542,23392256,64217734792576
1,617,1,True,candidate,32,2,32,748552192,brie,2.064204459,23392256,64217734792576
1,617,1,True,candidate,32,2,256,2014314496,brie_control,2.095457750,7868416,174974662069248
1,617,1,True,candidate,32,2,256,2014314496,brie,2.095584916,7868416,174974662069248
1,617,1,True,candidate,32,2,256,2014314496,jzap,1.428221333,7868416,174974662069248
2,6203,1,True,candidate,16,3,0,0,brie_control,1.489242250,100376576,0
2,6203,1,True,candidate,16,3,0,0,brie,1.471560375,100376576,0
2,6203,1,True,candidate,16,3,0,0,jzap,1.869653166,100376576,0
2,6203,1,True,candidate,16,3,1,64921600,brie,1.728497792,64921600,11420490401200
2,6203,1,True,candidate,16,3,1,64921600,jzap,1.347167500,64921600,11420490401200
2,6203,1,True,candidate,16,3,1,64921600,brie_control,1.729676875,64921600,11420490401200
2,6203,1,True,candidate,16,3,32,663355392,jzap,1.397578250,20729856,116247796215936
2,6203,1,True,candidate,16,3,32,663355392,brie_control,2.111120917,20729856,116247796215936
2,6203,1,True,candidate,16,3,32,663355392,brie,2.096770166,20729856,116247796215936
2,6203,1,True,candidate,16,3,256,1724907520,brie_control,1.939386250,6737920,303549818306560
2,6203,1,True,candidate,16,3,256,1724907520,brie,1.938943083,6737920,303549818306560
2,6203,1,True,candidate,16,3,256,1724907520,jzap,1.375935916,6737920,303549818306560
2,6203,1,True,previous,16,3,0,0,brie_control,1.486577750,100376576,0
2,6203,1,True,previous,16,3,0,0,brie,1.482212167,100376576,0
2,6203,1,True,previous,16,3,0,0,jzap,1.864432375,100376576,0
2,6203,1,True,previous,16,3,1,64921600,brie,1.732518083,64921600,11420490401200
2,6203,1,True,previous,16,3,1,64921600,jzap,1.372094125,64921600,11420490401200
2,6203,1,True,previous,16,3,1,64921600,brie_control,1.737386875,64921600,11420490401200
2,6203,1,True,previous,16,3,32,663355392,jzap,1.404340250,20729856,116247796215936
2,6203,1,True,previous,16,3,32,663355392,brie_control,2.085231458,20729856,116247796215936
2,6203,1,True,previous,16,3,32,663355392,brie,2.116053166,20729856,116247796215936
2,6203,1,True,previous,16,3,256,1724907520,brie_control,1.934776666,6737920,303549818306560
2,6203,1,True,previous,16,3,256,1724907520,brie,1.955328000,6737920,303549818306560
2,6203,1,True,previous,16,3,256,1724907520,jzap,1.396709375,6737920,303549818306560
2,6203,1,True,candidate,32,2,0,0,brie_control,1.538120917,138514432,0
2,6203,1,True,candidate,32,2,0,0,brie,1.530207333,138514432,0
2,6203,1,True,candidate,32,2,0,0,jzap,2.006410917,138514432,0
2,6203,1,True,candidate,32,2,1,83976192,brie,1.749496917,83976192,7393266403920
2,6203,1,True,candidate,32,2,1,83976192,jzap,1.414804625,83976192,7393266403920
2,6203,1,True,candidate,32,2,1,83976192,brie_control,1.753307042,83976192,7393266403920
2,6203,1,True,candidate,32,2,32,748552192,jzap,1.380264375,23392256,64927085218560
2,6203,1,True,candidate,32,2,32,748552192,brie_control,2.065997125,23392256,64927085218560
2,6203,1,True,candidate,32,2,32,748552192,brie,2.045756000,23392256,64927085218560
2,6203,1,True,candidate,32,2,256,2014314496,brie_control,2.102123542,7868416,175974322683904
2,6203,1,True,candidate,32,2,256,2014314496,brie,2.093975750,7868416,175974322683904
2,6203,1,True,candidate,32,2,256,2014314496,jzap,1.427845583,7868416,175974322683904
2,6203,1,True,previous,32,2,0,0,brie_control,1.538045459,138514432,0
2,6203,1,True,previous,32,2,0,0,brie,1.541755375,138514432,0
2,6203,1,True,previous,32,2,0,0,jzap,2.038278667,138514432,0
2,6203,1,True,previous,32,2,1,83976192,brie,1.751433541,83976192,7393266403920
2,6203,1,True,previous,32,2,1,83976192,jzap,1.415223333,83976192,7393266403920
2,6203,1,True,previous,32,2,1,83976192,brie_control,1.757754208,83976192,7393266403920
2,6203,1,True,previous,32,2,32,748552192,jzap,1.391864958,23392256,64927085218560
2,6203,1,True,previous,32,2,32,748552192,brie_control,2.055166958,23392256,64927085218560
2,6203,1,True,previous,32,2,32,748552192,brie,2.059459292,23392256,64927085218560
2,6203,1,True,previous,32,2,256,2014314496,brie_control,2.084530250,7868416,175974322683904
2,6203,1,True,previous,32,2,256,2014314496,brie,2.105657833,7868416,175974322683904
2,6203,1,True,previous,32,2,256,2014314496,jzap,1.430432000,7868416,175974322683904
2,6203,1,True,candidate,8,2,0,0,brie_control,1.484499750,334725120,0
2,6203,1,True,candidate,8,2,0,0,brie,1.483446208,334725120,0
2,6203,1,True,candidate,8,2,0,0,jzap,2.564446875,334725120,0
2,6203,1,True,candidate,8,2,1,139984896,brie,1.407907834,139984896,192063378432
2,6203,1,True,candidate,8,2,1,139984896,jzap,1.317442875,139984896,192063378432
2,6203,1,True,candidate,8,2,1,139984896,brie_control,1.411921125,139984896,192063378432
2,6203,1,True,candidate,8,2,32,1012924416,jzap,1.229208333,31653888,1404457291776
2,6203,1,True,candidate,8,2,32,1012924416,brie_control,1.507945959,31653888,1404457291776
2,6203,1,True,candidate,8,2,32,1012924416,brie,1.507659083,31653888,1404457291776
2,6203,1,True,candidate,8,2,256,2060451840,brie_control,1.361324042,8048640,2803940935680
2,6203,1,True,candidate,8,2,256,2060451840,brie,1.362650708,8048640,2803940935680
2,6203,1,True,candidate,8,2,256,2060451840,jzap,1.352321541,8048640,2803940935680
2,6203,1,True,previous,8,2,0,0,brie_control,1.483618334,334725120,0
2,6203,1,True,previous,8,2,0,0,brie,1.485310583,334725120,0
2,6203,1,True,previous,8,2,0,0,jzap,3.302338375,334725120,0
2,6203,1,True,previous,8,2,1,139984896,brie,1.408202000,139984896,192063378432
2,6203,1,True,previous,8,2,1,139984896,jzap,1.595589375,139984896,192063378432
2,6203,1,True,previous,8,2,1,139984896,brie_control,1.407045583,139984896,192063378432
2,6203,1,True,previous,8,2,32,1012924416,jzap,1.328097333,31653888,1404457291776
2,6203,1,True,previous,8,2,32,1012924416,brie_control,1.504182792,31653888,1404457291776
2,6203,1,True,previous,8,2,32,1012924416,brie,1.509404834,31653888,1404457291776
2,6203,1,True,previous,8,2,256,2060451840,brie_control,1.362455208,8048640,2803940935680
2,6203,1,True,previous,8,2,256,2060451840,brie,1.363576166,8048640,2803940935680
2,6203,1,True,previous,8,2,256,2060451840,jzap,1.374266667,8048640,2803940935680
3,96011,1,True,previous,32,2,0,0,brie_control,1.531278041,138514432,0
3,96011,1,True,previous,32,2,0,0,brie,1.541195416,138514432,0
3,96011,1,True,previous,32,2,0,0,jzap,2.015431625,138514432,0
3,96011,1,True,previous,32,2,1,83976192,brie,1.755002250,83976192,7336027280160
3,96011,1,True,previous,32,2,1,83976192,jzap,1.414455209,83976192,7336027280160
3,96011,1,True,previous,32,2,1,83976192,brie_control,1.763514292,83976192,7336027280160
3,96011,1,True,previous,32,2,32,748552192,jzap,1.401374792,23392256,65996315268992
3,96011,1,True,previous,32,2,32,748552192,brie_control,2.026675417,23392256,65996315268992
3,96011,1,True,previous,32,2,32,748552192,brie,2.073874583,23392256,65996315268992
3,96011,1,True,previous,32,2,256,2014314496,brie_control,2.102091542,7868416,178999044083712
3,96011,1,True,previous,32,2,256,2014314496,brie,2.083046000,7868416,178999044083712
3,96011,1,True,previous,32,2,256,2014314496,jzap,1.434937416,7868416,178999044083712
3,96011,1,True,candidate,32,2,0,0,brie_control,1.533091041,138514432,0
3,96011,1,True,candidate,32,2,0,0,brie,1.538603500,138514432,0
3,96011,1,True,candidate,32,2,0,0,jzap,2.011583833,138514432,0
3,96011,1,True,candidate,32,2,1,83976192,brie,1.755980083,83976192,7336027280160
3,96011,1,True,candidate,32,2,1,83976192,jzap,1.418457667,83976192,7336027280160
3,96011,1,True,candidate,32,2,1,83976192,brie_control,1.750581917,83976192,7336027280160
3,96011,1,True,candidate,32,2,32,748552192,jzap,1.367183208,23392256,65996315268992
3,96011,1,True,candidate,32,2,32,748552192,brie_control,2.055616667,23392256,65996315268992
3,96011,1,True,candidate,32,2,32,748552192,brie,2.069024083,23392256,65996315268992
3,96011,1,True,candidate,32,2,256,2014314496,brie_control,2.086209750,7868416,178999044083712
3,96011,1,True,candidate,32,2,256,2014314496,brie,2.124258541,7868416,178999044083712
3,96011,1,True,candidate,32,2,256,2014314496,jzap,1.438683375,7868416,178999044083712
3,96011,1,True,previous,8,2,0,0,brie_control,1.486974125,334725120,0
3,96011,1,True,previous,8,2,0,0,brie,1.488109708,334725120,0
3,96011,1,True,previous,8,2,0,0,jzap,3.332012125,334725120,0
3,96011,1,True,previous,8,2,1,139984896,brie,1.413035458,139984896,193447643136
3,96011,1,True,previous,8,2,1,139984896,jzap,1.590363916,139984896,193447643136
3,96011,1,True,previous,8,2,1,139984896,brie_control,1.411854042,139984896,193447643136
3,96011,1,True,previous,8,2,32,1012924416,jzap,1.316605125,31653888,1373140715520
3,96011,1,True,previous,8,2,32,1012924416,brie_control,1.510119916,31653888,1373140715520
3,96011,1,True,previous,8,2,32,1012924416,brie,1.510211041,31653888,1373140715520
3,96011,1,True,previous,8,2,256,2060451840,brie_control,1.360506167,8048640,2798584565760
3,96011,1,True,previous,8,2,256,2060451840,brie,1.361286625,8048640,2798584565760
3,96011,1,True,previous,8,2,256,2060451840,jzap,1.373550125,8048640,2798584565760
3,96011,1,True,candidate,8,2,0,0,brie_control,1.485339792,334725120,0
3,96011,1,True,candidate,8,2,0,0,brie,1.484870291,334725120,0
3,96011,1,True,candidate,8,2,0,0,jzap,2.554526542,334725120,0
3,96011,1,True,candidate,8,2,1,139984896,brie,1.415562041,139984896,193447643136
3,96011,1,True,candidate,8,2,1,139984896,jzap,1.311459333,139984896,193447643136
3,96011,1,True,candidate,8,2,1,139984896,brie_control,1.414742416,139984896,193447643136
3,96011,1,True,candidate,8,2,32,1012924416,jzap,1.238095334,31653888,1373140715520
3,96011,1,True,candidate,8,2,32,1012924416,brie_control,1.511247375,31653888,1373140715520
3,96011,1,True,candidate,8,2,32,1012924416,brie,1.509326917,31653888,1373140715520
3,96011,1,True,candidate,8,2,256,2060451840,brie_control,1.362345042,8048640,2798584565760
3,96011,1,True,candidate,8,2,256,2060451840,brie,1.361443042,8048640,2798584565760
3,96011,1,True,candidate,8,2,256,2060451840,jzap,1.351174208,8048640,2798584565760
3,96011,1,True,previous,16,3,0,0,brie_control,1.474777875,100376576,0
3,96011,1,True,previous,16,3,0,0,brie,1.483434209,100376576,0
3,96011,1,True,previous,16,3,0,0,jzap,1.858956209,100376576,0
3,96011,1,True,previous,16,3,1,64921600,brie,1.731971208,64921600,11392859032800
3,96011,1,True,previous,16,3,1,64921600,jzap,1.389213958,64921600,11392859032800
3,96011,1,True,previous,16,3,1,64921600,brie_control,1.730873417,64921600,11392859032800
3,96011,1,True,previous,16,3,32,663355392,jzap,1.407102833,20729856,116344970007168
3,96011,1,True,previous,16,3,32,663355392,brie_control,2.107581875,20729856,116344970007168
3,96011,1,True,previous,16,3,32,663355392,brie,2.086021125,20729856,116344970007168
3,96011,1,True,previous,16,3,256,1724907520,brie_control,1.955643708,6737920,299714696995840
3,96011,1,True,previous,16,3,256,1724907520,brie,1.948129416,6737920,299714696995840
3,96011,1,True,previous,16,3,256,1724907520,jzap,1.389485708,6737920,299714696995840
3,96011,1,True,candidate,16,3,0,0,brie_control,1.475774292,100376576,0
3,96011,1,True,candidate,16,3,0,0,brie,1.482723375,100376576,0
3,96011,1,True,candidate,16,3,0,0,jzap,1.864703500,100376576,0
3,96011,1,True,candidate,16,3,1,64921600,brie,1.733329917,64921600,11392859032800
3,96011,1,True,candidate,16,3,1,64921600,jzap,1.352447000,64921600,11392859032800
3,96011,1,True,candidate,16,3,1,64921600,brie_control,1.746431000,64921600,11392859032800
3,96011,1,True,candidate,16,3,32,663355392,jzap,1.413456333,20729856,116344970007168
3,96011,1,True,candidate,16,3,32,663355392,brie_control,2.086043042,20729856,116344970007168
3,96011,1,True,candidate,16,3,32,663355392,brie,2.089054750,20729856,116344970007168
3,96011,1,True,candidate,16,3,256,1724907520,brie_control,1.942417791,6737920,299714696995840
3,96011,1,True,candidate,16,3,256,1724907520,brie,1.960332791,6737920,299714696995840
3,96011,1,True,candidate,16,3,256,1724907520,jzap,1.381781500,6737920,299714696995840
```

</details>

<details>
<summary>Fixed-key hosted protocol seed totals</summary>

```csv
condition,seed,implementation,timed_batches,total_seconds,operations
remove_cycle,617,std,294,1.418988171,71178576
remove_cycle,617,old,294,1.218049136,71178576
remove_cycle,617,jzap,294,1.121999577,71178576
remove_cycle,617,std_control,294,1.419768666,71178576
shrink_lifecycle,617,std,51,1.140498942,52224
shrink_lifecycle,617,old,51,1.309873327,52224
shrink_lifecycle,617,jzap,51,1.186929503,52224
shrink_lifecycle,617,std_control,51,1.141747628,52224
clone,617,std,2628,1.241864834,2691072
clone,617,old,2628,1.126925232,2691072
clone,617,jzap,2628,1.120024327,2691072
clone,617,std_control,2628,1.232070009,2691072
grow,617,std,104,1.126885667,106496
grow,617,old,104,1.298655721,106496
grow,617,jzap,104,1.172862038,106496
grow,617,std_control,104,1.127494379,106496
shrink,617,std,103,1.120599591,105472
shrink,617,old,103,1.308762619,105472
shrink,617,jzap,103,1.188103807,105472
shrink,617,std_control,103,1.121714607,105472
remove_cycle,6203,std,230,1.404740051,67377120
remove_cycle,6203,old,230,1.215550790,67377120
remove_cycle,6203,jzap,230,1.124617746,67377120
remove_cycle,6203,std_control,230,1.407078456,67377120
shrink_lifecycle,6203,std,42,1.143107753,43008
shrink_lifecycle,6203,old,42,1.312022711,43008
shrink_lifecycle,6203,jzap,42,1.189817666,43008
shrink_lifecycle,6203,std_control,42,1.142650971,43008
clone,6203,std,2262,1.239506737,2316288
clone,6203,old,2262,1.123759235,2316288
clone,6203,jzap,2262,1.120058527,2316288
clone,6203,std_control,2262,1.237717616,2316288
grow,6203,std,86,1.122505822,88064
grow,6203,old,86,1.298548594,88064
grow,6203,jzap,86,1.172746725,88064
grow,6203,std_control,86,1.121361854,88064
shrink,6203,std,85,1.123797124,87040
shrink,6203,old,85,1.318212342,87040
shrink,6203,jzap,85,1.194639028,87040
shrink,6203,std_control,85,1.123472670,87040
remove_cycle,96011,std,239,1.378121242,65929106
remove_cycle,96011,old,239,1.216680962,65929106
remove_cycle,96011,jzap,239,1.120113028,65929106
remove_cycle,96011,std_control,239,1.376099499,65929106
shrink_lifecycle,96011,std,44,1.128398097,45056
shrink_lifecycle,96011,old,44,1.296134453,45056
shrink_lifecycle,96011,jzap,44,1.173866126,45056
shrink_lifecycle,96011,std_control,44,1.127091985,45056
clone,96011,std,2419,1.224045083,2477056
clone,96011,old,2419,1.120231530,2477056
clone,96011,jzap,2419,1.121461056,2477056
clone,96011,std_control,2419,1.221791824,2477056
grow,96011,std,92,1.129891344,94208
grow,96011,old,92,1.308466459,94208
grow,96011,jzap,92,1.182173951,94208
grow,96011,std_control,92,1.130114653,94208
shrink,96011,std,91,1.130382951,93184
shrink,96011,old,91,1.326016332,93184
shrink,96011,jzap,91,1.198393570,93184
shrink,96011,std_control,91,1.131223933,93184
```

</details>
