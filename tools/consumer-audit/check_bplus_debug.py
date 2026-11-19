#!/usr/bin/env python3
"""Replay the source-bound guard audit for the Linux x86-64 debug B+ fixture."""
import argparse
from collections import defaultdict
import hashlib
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from bplus_bindings import check as check_bplus
from link_call_graph import LinkCallGraph, PAIR_WINDOW_NEXT, PAIR_WINDOW_PATH
SLICE = "core::slice::<impl [T]>::"
LAYOUT = "core::alloc::layout::Layout::"
GUARD = "::precondition_check"
ZERO = ("core::alloc::global::GlobalAlloc::alloc_zeroed", "__rustc::__rust_alloc_zeroed")
VEC_SLICE = ("core::slice::raw::from_raw_parts", "jzap::vec::Vec<T,P,L>::as_slice")
VEC_MUT_SLICE = ("core::slice::raw::from_raw_parts_mut", "jzap::vec::Vec<T,P,L>::as_mut_slice")
CAPACITY = (LAYOUT + "from_size_align_unchecked", "jzap::capacity::Capacity<T>::layout")
RESERVE = "jzap::budget::concurrent::ConcurrentMemoryBudget::reserve"
SEARCH = {
    # The inlined FixedTree lookup calls binary_search_by directly.
    (SLICE + "binary_search_by", "jzap_consumer_audit::consumer::check_width"),
    (SLICE + "binary_search_by", SLICE + "binary_search",
     "jzap_consumer_audit::consumer::check_width"),
    (SLICE + "binary_search_by", SLICE + "partition_point",
     "jzap::collections::btree_map::view::bplus::leaf_index"),
    (SLICE + "binary_search_by", SLICE + "partition_point",
     "jzap_consumer_audit::consumer::pool::leaf_index"),
}

# Search and ordering use the pinned core arithmetic proofs. The other guards instantiate
# the baseline Vec storage/layout/budget contracts and the fixture allocator.
CALLERS = {
    PAIR_WINDOW_NEXT: {PAIR_WINDOW_PATH},
    "core::hint::assert_unchecked" + GUARD: SEARCH,
    "<usize as core::slice::index::SliceIndex<[T]>>::get_unchecked" + GUARD: SEARCH,
    LAYOUT + "from_size_align_unchecked" + GUARD: {CAPACITY},
    LAYOUT + "from_size_alignment_unchecked" + GUARD: {
        (LAYOUT + "from_size_alignment_unchecked", "__rustc::__rust_" + name)
        for name in ("alloc", "alloc_zeroed", "dealloc", "realloc")
    },
    "core::ptr::alignment::Alignment::new_unchecked" + GUARD: {
        (LAYOUT + "from_size_align", "jzap::layout::array"),
        (LAYOUT + "from_size_align_unchecked" + GUARD, *CAPACITY),
    },
    "core::ptr::write_bytes" + GUARD: {ZERO},
    "core::ptr::const_ptr::<impl *const T>::is_aligned_to": {
        ("core::ptr::write_bytes" + GUARD, *ZERO),
        ("core::slice::raw::from_raw_parts" + GUARD, *VEC_SLICE),
        ("core::slice::raw::from_raw_parts_mut" + GUARD, *VEC_MUT_SLICE),
    },
    "core::ptr::non_null::NonNull<T>::new_unchecked" + GUARD: {
        ("core::ptr::non_null::NonNull<T>::new", "jzap::storage::" + operation)
        for operation in ("allocate_impl", "reallocate")
    },
    "core::slice::raw::from_raw_parts" + GUARD: {VEC_SLICE},
    "core::slice::raw::from_raw_parts_mut" + GUARD: {VEC_MUT_SLICE},
    "core::sync::atomic::atomic_load": {
        ("core::sync::atomic::AtomicUsize::load", RESERVE),
    },
    "core::sync::atomic::atomic_compare_exchange": {
        ("core::sync::atomic::AtomicUsize::compare_exchange", RESERVE),
    },
}
PANICKING = "core::panicking::"
FORWARDING = {
    (PANICKING + "panic_nounwind", PANICKING + "panic_nounwind_fmt"),
    (PANICKING + "panic_nounwind_fmt", PANICKING + "panic_cannot_unwind"),
    (PANICKING + "panic_cannot_unwind", PANICKING + "panic_nounwind"),
    ("core::slice::index::slice_index_fail", PANICKING + "panic_fmt"),
    ("__rustc::rust_begin_unwind", "jzap_link_audit_panic_reachable"),
}
DIAGNOSTIC = {"jzap_link_audit_panic_reachable", "rust_eh_personality"}
EXIT_NAMES = {"abort", "exit", "_Exit", "_exit", "quick_exit", "pthread_exit", "__assert_fail"}
PINNED_CALLER_SOURCES = {
    "tools/consumer-audit/cases/bplus_pool.rs": "c0c685f58d33d718a7ad6e004d0dd0a18a250b602455ec4b514bfc7fcd24c079",
}


def inspect(assembly, relocations):
    graph = LinkCallGraph(assembly, relocations)
    observed = defaultdict(set)
    count = 0
    for target, target_name in graph.names.items():
        exit_sink = target_name.split("@", 1)[0] in EXIT_NAMES
        panic_sink = any(word in target_name for word in (
            PANICKING, "unwrap_failed", "handle_alloc_error", "jzap_link_audit_panic_reachable",
            "core::slice::index::slice_index_fail", "rust_eh_personality",
        ))
        if not (exit_sink or panic_sink):
            continue
        for caller in graph.incoming(target):
            name = graph.names[caller]
            edge = (name, target_name)
            if exit_sink:
                if name not in DIAGNOSTIC:
                    raise ValueError(f"unexpected exit edge: {edge}")
                continue
            count += 1
            if edge in FORWARDING:
                continue
            if name not in CALLERS:
                raise ValueError(f"unbound panic edge: {edge}")
            observed[name].update(graph.verify_path(caller, CALLERS[name]))
    for name, paths in CALLERS.items():
        if observed[name] != paths:
            raise ValueError(f"missing caller paths for {name}: {sorted(paths - observed[name])}")
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=ROOT / "target/verification/bplus-debug")
    args = parser.parse_args()
    if (os.uname().sysname, os.uname().machine) != ("Linux", "x86_64"):
        parser.error("this caller audit requires Linux x86-64 and GNU objdump")
    check_bplus(ROOT)
    for path, expected in PINNED_CALLER_SOURCES.items():
        if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != expected:
            raise SystemExit(f"unbound debug caller source: {path}")

    for name in ("check_bplus_prefix.py", "check_bplus_core_guards.py", "check_bplus_fixed.py"):
        subprocess.run([sys.executable, str(HERE / name)], check=True)

    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.pop("RUSTFLAGS", None)
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env.update(CARGO_PROFILE_DEV_PANIC="abort", CARGO_PROFILE_DEV_OVERFLOW_CHECKS="true",
               CARGO_PROFILE_DEV_DEBUG_ASSERTIONS="true", CARGO_PROFILE_DEV_OPT_LEVEL="0",
               CARGO_PROFILE_DEV_LTO="false",
               JZAP_CONSUMER_SOURCE=str(HERE / "cases/bplus_pool.rs"))
    subprocess.run(
        ["cargo", "build", "--locked", "--offline", "--manifest-path", str(HERE / "Cargo.toml"),
         "--target-dir", str(out)], env=env, cwd=ROOT, check=True,
    )
    binary = out / "diagnostic"
    cc = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-ffunction-sections", "-fdata-sections",
          "-DAUDIT_REQUIRE_SUCCESS", str(HERE / "driver.c")]
    libraries = [str(out / "debug/libjzap_consumer_audit.a"), "-Wl,--gc-sections",
                 "-ldl", "-lpthread", "-lm"]
    strict = subprocess.run(
        [*cc, *libraries, "-o", str(out / "strict")], capture_output=True, text=True,
    )
    (out / "strict.log").write_text(strict.stderr)
    if strict.returncode == 0 or "undefined reference" not in strict.stderr or (
        "jzap_link_audit_panic_reachable" not in strict.stderr
    ):
        raise SystemExit("expected the strict debug link to retain the panic sink")
    print("Strict debug link retains the unresolved panic sink", flush=True)
    subprocess.run(
        [*cc, str(ROOT / "tools/link-audit/diagnostic.c"), *libraries, "-o", str(binary)],
        check=True,
    )
    assembly = subprocess.check_output(["objdump", "-dC", str(binary)], text=True)
    relocations = subprocess.check_output(["objdump", "-RC", str(binary)], text=True)
    (out / "diagnostic.asm").write_text(assembly)
    (out / "relocations.txt").write_text(relocations)
    count = inspect(assembly, relocations)
    print(f"{count} resolved panic edges, {len(CALLERS)} guard kinds bound to their callers", flush=True)

    # Changing only the predecessor must fail even when the panic sink and
    # precondition-check symbol still have their expected names.
    mutant = assembly.replace(SLICE + "binary_search_by", "unreviewed_binary_search_by")
    try:
        inspect(mutant, relocations)
    except ValueError as error:
        if "unexpected callers" not in str(error):
            raise
        print(f"Caller mutation rejected: {error}", flush=True)
    else:
        raise SystemExit("unreviewed caller passed the guard check")
    subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    main()
