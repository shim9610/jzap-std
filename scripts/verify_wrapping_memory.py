#!/usr/bin/env python3
"""Connect all internal modular arithmetic to numeric and byte-span proofs.

Combine all-word input checks with finite non-ZST movement and cleanup probes.
No memory-safety, overflow, unwinding, or undefined-behavior checks are disabled.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
from scanner_tools import scanner
import re
import subprocess
import tempfile
import shutil
import verus_std_specs
from datetime import datetime, timezone
from verus_composition import run as compose
from arithmetic_projection import bounded_projection, validate_bounded
from btree_bindings import is_btree
from hash_map_bindings import is_hash_map
from verify_binary_heap import is_binary_heap
from verify_deque import is_deque
from verify_scratch_slice import is_scratch_slice
from premise_status import row_premise, verified_function_names

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target/verification/wrapping-memory"
VERUS = Path(os.environ.get("JZAP_VERUS", "/workspace/.devtools/verus/verus-x86-linux/verus"))


def compact(text):
    return re.sub(r"\s+", "", text)


def call_args(text, marker):
    start = text.index(marker) + len(marker)
    assert text[start] == "(", (marker, text)
    depth, end, last, args = 1, start + 1, start + 1, []
    while depth:
        c = text[end]
        if c == "(": depth += 1
        if c == ")": depth -= 1
        if c == "," and depth == 1:
            args.append(text[last:end]); last = end + 1
        end += 1
    args.append(text[last:end - 1])
    return [arg for arg in args if arg]


def scalar(text, extra=None):
    text = compact(text)
    substitutions = {
        "self.owner.len.get()": "len", "self.len.get()": "len",
        "self.cap.get()": "cap", "self.state.read()": "s@.read",
        "self.state.written()": "s@.write", "self.tail_start": "tail_start",
    }
    substitutions.update(extra or {})
    for old, new in substitutions.items(): text = text.replace(old, new)
    text = re.sub(r"Checked::new\((\w+)\)", r"\1", text).replace(".get()", "")
    text = re.sub(r"\bself\.len\b", "len", text)
    assert re.fullmatch(r"[a-zA-Z_0-9@.()+*<>=!&| -]+", text), text
    return text


SINGLE_ELEMENT_MOVES = {
    "try_insert": "{letshifted=matchgeometry::insertion(self.len,index){Ok(shifted)=>shifted,Err(error)=>returnErr(Rejected{value,error}),};"
        "letneeded=matchself.reserve_length(1){Ok(needed)=>needed,Err(error)=>returnErr(Rejected{value,error}),};"
        "unsafe{letplace=self.ptr.as_ptr().add(index);core::ptr::copy(place,place.add(1),shifted.get());place.write(value);}"
        "self.len=Checked::new(needed);Ok(())}",
    "remove": "{let(next,shifted)=geometry::removal(self.len,index)?;"
        "unsafe{letplace=self.ptr.as_ptr().add(index);letvalue=place.read();core::ptr::copy(place.add(1),place,shifted.get());self.len=next;Ok(value)}}",
    "swap_remove": "{letnext=geometry::shortened(self.len,index)?;self.len=next;"
        "unsafe{letplace=self.ptr.as_ptr().add(index);letvalue=place.read();core::ptr::copy(self.ptr.as_ptr().add(self.len.get()),place,1);Ok(value)}}",
}


def inventory(source_root):
    files = sorted(p.relative_to(source_root) for p in (source_root / "src").rglob("*.rs") if not p.name.endswith("proofs.rs") and not is_btree(str(p.relative_to(source_root))) and not is_hash_map(str(p.relative_to(source_root))) and not is_binary_heap(str(p.relative_to(source_root))) and not is_deque(str(p.relative_to(source_root))) and not is_scratch_slice(str(p.relative_to(source_root))))
    return json.loads(subprocess.check_output([str(scanner('memory')), *map(str, files)], cwd=source_root, text=True))


def generate(source_root, census):
    def body(file, function, context, normalized=True):
        matches = [r for r in census if r["kind"] == "body" and r["file"] == file and r["function"] == function and r["context"].split("<")[0].strip() == context]
        assert len(matches) == 1, (file, function, context, len(matches))
        value = matches[0]["expression"]
        return compact(value) if normalized else value
    vector = "src/vec.rs"
    expressions = {}
    # Check the real composition: final length -> growth -> checked layout ->
    # successful storage call -> publish capacity. No invented success guard.
    assert body(vector, "reserve_length", "Vec") == (
        "{letneeded=required_len(self.len.get(),additional,self.max_len())?;"
        "ifneeded>self.cap.get(){self.grow_to(needed,false)?;}Ok(needed)}")
    growing = body(vector, "grow_to", "Vec")
    assert growing=='{letcap=geometry::growth_for_layout(self.cap.get(),needed,self.max_len(),mem::size_of::<T>(),exact,)?;self.resize_storage(cap)}', 'unbound layout-growth arguments/publication'
    resizing = body(vector, "resize_storage", "Vec")
    assert resizing.startswith("{let(capacity,new_layout)=crate::capacity::Capacity::try_new(cap)?;")
    assert "storage::allocate(new_layout,self.policy.budget())?" in resizing
    assert "storage::reallocate(self.ptr.cast(),self.layout(),new_layout,self.policy.budget(),)}?" in resizing
    assert resizing.endswith("self.ptr=ptr.cast();self.cap=capacity;Ok(())}")
    # The single-element moves take their positions from product kernels
    # verified on the source (insertion, removal, shortened in vec/geometry.rs).
    # No expression is extracted: each caller body is compared whole, so any
    # other offset, count, order or publication is unrouted.
    for function, expected in SINGLE_ELEMENT_MOVES.items():
        assert body(vector, function, "Vec") == expected, ("single-element move routing", function)
    # The public getter delegates to core's checked SliceIndex implementation.
    assert body(vector, "get_mut", "Vec") == "{self.as_mut_slice().get_mut(index).ok_or(Error::Bounds)}"
    assert body("src/boxed/slice.rs", "get_mut", "Box") == "{self.as_mut_slice().get_mut(index).ok_or(Error::Bounds)}"
    # The all-word range-validator contract discharges invalid external inputs
    # before caller mutation or pointer use. u8 physical probes then cover every
    # admitted endpoint for their two-byte owners, without omitting high inputs.
    for file, function, callee in [
        ("src/vec/convenience.rs", "try_extend_from_within", "edit::checked_range"),
        ("src/vec/convenience.rs", "try_extend_copy_from_within", "edit::checked_range"),
        ("src/vec/edit.rs", "try_drain", "checked_range"),
        ("src/vec/edit.rs", "try_extract_if", "checked_range"),
    ]:
        assert body(file, function, "Vec").startswith("{let(start,end)=" + callee + "(range,self.len.get())?;"), (file, function)
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    core_index = sysroot / "lib/rustlib/src/rust/library/core/src/slice/index.rs"
    core = compact(core_index.read_text())
    start = core.index("SliceIndex<[T]>forusize{")
    checked = core[start:core.index("unsafefnget_unchecked", start)]
    assert checked.count("ifself<slice.len()") == 2
    expressions["INDEX_REJECT"] = "!(index<len)"
    spare = body("src/vec/convenience.rs", "spare_capacity_mut", "Vec")
    expressions["SPARE_START"] = scalar(call_args(spare, "self.ptr.as_ptr().add")[0])
    copying = body("src/vec/convenience.rs", "try_extend_copy_from_within", "Vec")
    src, dst, count = call_args(copying, "core::ptr::copy_nonoverlapping")
    expressions["WITHIN_COPY_SOURCE"] = scalar(call_args(src, "self.ptr.as_ptr().add")[0])
    expressions["WITHIN_COPY_DESTINATION"] = scalar(call_args(dst, "self.ptr.as_ptr().add")[0])
    expressions["WITHIN_COPY_COUNT"] = scalar(count)
    cloning = body("src/vec/convenience.rs", "try_extend_from_within", "Vec")
    assert "foriinstart..end{" in cloning
    expressions["CLONE_SOURCE_INDEX"] = scalar(call_args(cloning, "guard.owner.ptr.as_ptr().add")[0], {"i": "index"})
    pointers = [r for r in census if r["file"] == "src/vec/convenience.rs" and r["function"] == "try_extend_from_within" and r["kind"] == "memory_method" and ". add (guard . owner . len . get ())" in r["expression"]]
    assert len(pointers) == 1
    expressions["CLONE_DESTINATION_INDEX"] = "prefix"
    assert cloning.index("self.try_reserve(count.get())?") < cloning.index("guard.owner.ptr.as_ptr().add")
    # Extract the actual source/destination expressions, not copies of old code.
    edit = "src/vec/edit.rs"
    for context, prefix in [("Compact", "COMPACT"), ("ExtractIf", "EXTRACT")]:
        text = body(edit, "drop", context)
        src, dst, _ = call_args(text, "core::ptr::copy")
        extra = {"self.state.read()": "s@.scan.read", "self.state.written()": "s@.scan.write"} if context == "ExtractIf" else {}
        for label, expression in [("SOURCE", src), ("DESTINATION", dst)]:
            expressions[prefix + "_" + label] = scalar(call_args(expression, "self.owner.ptr.as_ptr().add")[0], extra)
    drain = body(edit, "restore_tail", "Drain")
    src, dst, count = call_args(drain, "core::ptr::copy")
    assert count == "self.tail_len"
    expressions["DRAIN_SOURCE"] = scalar(call_args(src, "self.owner.ptr.as_ptr().add")[0])
    expressions["DRAIN_DESTINATION"] = scalar(call_args(dst, "self.owner.ptr.as_ptr().add")[0])
    original = json.loads((ROOT / "target/verification/panic-state/summary.json").read_text())
    expressions["DRAIN_RESTORED"] = original["bindings"]["DRAIN_RESTORED"]["expression"] if "DRAIN_RESTORED" in original["bindings"] else scalar(bounded_projection(re.search(r"Checked::new\((.*)\);self.tail_len=0", drain).group(1)), {"self.tail_len": "tail_len"})
    for function, prefix in [("move_tail", "SPLICE_MOVE"), ("stage_excess", "STAGED_TAIL")]:
        text = body(edit, function, "Splice")
        src, dst, count = call_args(text, "core::ptr::copy")
        assert count == "self.drain.tail_len", count
        for label, pointer in [("SOURCE", src), ("DESTINATION", dst)]:
            expressions[prefix + "_" + label] = scalar(call_args(pointer, "self.drain.owner.ptr.as_ptr().add")[0], {"self.drain.tail_start": "tail_start", "next_tail.get()": "destination"})
        if function == "stage_excess":
            src, dst, count = call_args(text, "core::ptr::copy_nonoverlapping")
            assert src == "extra.ptr.as_ptr()" and count == "extra.len.get()"
            expressions["STAGED_EXTRA_DESTINATION"] = scalar(call_args(dst, "self.drain.owner.ptr.as_ptr().add")[0], {"self.drain.owner.len.get()": "len"})
    # Safe capacity constructors close the multiplication invariant. Raw import
    # stays an explicit unsafe caller obligation, not an arbitrary safe escape.
    cap_source = compact((source_root / "src/capacity.rs").read_text())
    assert "pub(crate)structCapacity<T>{count:Checked<usize>,element:PhantomData<fn()->T>,}" in cap_source
    assert "pub(crate)unsafefnfrom_validated_raw(count:usize)->Self" in cap_source
    creation = body("src/capacity.rs", "try_new", "Capacity")
    assert creation.index("crate::layout::array::<T>(count)?") < creation.index("Self{count:Checked::new(count)")
    assert "logical_capacity(mem::size_of::<T>(),0,limit)" in body("src/capacity.rs", "empty", "Capacity")
    assert "logical_capacity(mem::size_of::<T>(),values.len(),limit)" in body("src/capacity.rs", "for_slice", "Capacity")
    # The element-extent lemmas are a maintained file of their own, shared with
    # the storage kernel of the area compositions.
    template = (ROOT / "tools/memory-proof/extent.verus.rs").read_text() + (ROOT / "tools/memory-proof/wrapping-memory.verus.rs").read_text()
    for key, expression in expressions.items(): template = re.sub(r"\b" + key + r"\b", lambda _: expression, template)
    # Proof arithmetic is mathematical, so pointer-chain sums must not be
    # coerced back to machine words before their upper bounds are established.
    for match in reversed(list(re.finditer(r"\bmemory_span(?=\()", template))):
        arguments = call_args(template[match.start():], "memory_span")
        if ":" in arguments[0]: continue
        opening = template.index("(", match.start()); end, depth = opening + 1, 1
        while depth:
            depth += (template[end] == "(") - (template[end] == ")"); end += 1
        converted = arguments[:2] + ["(" + arg + ") as int" for arg in arguments[2:]]
        template = template[:match.start()] + "memory_span(" + ",".join(converted) + ")" + template[end:]
    assert not re.search(r"\b(?:INDEX|INSERT|REMOVE|SWAP|SPARE|COMPACT|EXTRACT|DRAIN|SPLICE_MOVE|STAGED_TAIL|STAGED_EXTRA|WITHIN_COPY|CLONE|GROWTH)_[A-Z_]+\b", template)
    return template, expressions, dict(path=str(core_index), sha256=hashlib.sha256(core_index.read_bytes()).hexdigest())


# Each entry has a freshly verified implementation and a byte-span theorem.
# main() checks that every name is a verified exec or proof function; whether
# each call establishes the premises of its contracts is recorded per contract
# in premise_status.py and reported with each ledger row. Counts are per
# function, not per call.
FAMILIES = {
    # Each increment follows a guard on the same word, so it cannot wrap for
    # any caller: guarded_stores() pins the guard next to the increment and
    # guarded_increment has no requires.
    ("src/init/driver.rs", "", "run"): (1, ["guarded_increment"]),
    ("src/vec/staging_driver.rs", "", "run"): (1, ["guarded_increment"]),
    ("src/shared/count.rs", "LocalCount", "release"): (1, ["shared_release_credit"]),
    ("src/shared/count.rs", "AtomicCount", "buffered_clone_strong"): (1, ["shared_buffered_reservation_extent"]),
    ("src/boxed/text.rs", "CharBytes", "try_next"): (1, ["utf8_byte_cursor"]),
    ("src/boxed/text.rs", "CharBytes", "try_size_hint"): (1, ["utf8_byte_cursor"]),
    ("src/budget_numeric.rs", "", "pending"): (2, ["reserved"]),
    ("src/budget_numeric.rs", "", "reserve"): (1, ["reserve"]),
    ("src/budget_numeric.rs", "", "credit"): (1, ["commit", "cancel"]),
    ("src/budget_numeric.rs", "", "retire"): (1, ["release"]),
    ("src/capacity.rs", "Capacity", "layout"): (1, ["capacity_bytes", "external_count_before_allocation"]),
    ("src/compaction.rs", "Compaction", "remaining"): (1, ["c_remaining", "compaction_before_move"]),
    ("src/compaction.rs", "Compaction", "previous_kept"): (1, ["c_previous_kept"]),
    ("src/compaction.rs", "Compaction", "restored_len"): (1, ["c_restored_len", "compaction_before_move"]),
    ("src/compaction.rs", "Compaction", "keep_next"): (2, ["c_keep_next", "compaction_before_element"]),
    ("src/compaction.rs", "Compaction", "remove_next"): (1, ["c_remove_next", "compaction_before_element"]),
    ("src/compaction.rs", "Compaction", "keep_live"): (2, ["c_keep_live"]),
    ("src/compaction.rs", "Compaction", "remove_live"): (1, ["c_remove_live"]),
    ("src/compaction.rs", "Extraction", "unvisited_len"): (1, ["e_unvisited_len", "extraction_before_move"]),
    ("src/compaction.rs", "Extraction", "restored_len"): (1, ["e_restored_len", "extraction_before_move"]),
    ("src/compaction.rs", "Extraction", "written"): (1, ["e_written"]),
    ("src/compaction.rs", "Extraction", "remaining"): (1, ["e_remaining"]),
    ("src/compaction.rs", "Extraction", "keep_next"): (1, ["e_keep_next"]),
    ("src/compaction.rs", "Extraction", "remove_next"): (2, ["e_remove_next"]),
    ("src/span.rs", "Span", "len"): (1, ["cursor_remaining", "cursor_before_read"]),
    ("src/span.rs", "Span", "next"): (1, ["iterator_cursor", "cursor_before_read"]),
    ("src/span.rs", "Span", "next_back"): (1, ["iterator_cursor", "cursor_before_read"]),
    ("src/vec.rs", "Vec", "remaining_limit"): (1, ["owner_remaining_limit"]),
    ("src/vec/convenience.rs", "Vec", "spare_capacity_mut"): (1, ["owner_spare_capacity", "spare_view_before_slice"]),
    ("src/vec/convenience.rs", "Vec", "split_at_spare_mut"): (1, ["owner_spare_capacity", "spare_view_before_slice"]),
    ("src/vec/edit.rs", "Vec", "try_dedup_by"): (1, ["c_previous_kept"]),
    ("src/vec/edit.rs", "Drain", "restore_tail"): (1, ["drain_before_tail_restore"]),
    # Calls inside functions verified directly on the product source: Verus
    # checks each call's bounded precondition there (verify_direct_verus.py
    # target fixed-bplus); coverage requires the enclosing verus_spec.
    ("src/collections/btree_map/view/bplus/fixed.rs", "FixedTree", "assemble"): (1, ["direct:fixed-bplus"]),
    ("src/collections/btree_map/view/bplus/fixed.rs", "FixedTree", "find"): (3, ["direct:fixed-bplus"]),
    ("src/collections/btree_map/view/bplus/fixed.rs", "", "leaf_rank"): (1, ["direct:fixed-bplus"]),
    ("src/collections/btree_map/view/bplus/fixed.rs", "", "summarize"): (3, ["direct:fixed-bplus"]),
    ("src/collections/btree_map/view/bplus/fixed.rs", "", "summarize_branch"): (2, ["direct:fixed-bplus"]),
    # The partial-key pass core, verified directly (target bplus-partial).
    ("src/collections/btree_map/view/bplus/partial.rs", "", "pass_over"): (7, ["direct:bplus-partial"]),
}


def guarded_stores(texts):
    """Each of the two driver increments directly follows its guard on the same word.

    `texts` maps the two driver files to their source. The loop word is read
    once into the incremented local and is not rebound before the increment.
    """
    compact = lambda text: re.sub(r"\s+", "", re.sub(r"(?m)^\s*#\[cfg[^\n]*\n(?:\s*(?:let|proof!)[^\n]*\n)?", "", text))
    driver = compact(texts["src/init/driver.rs"])
    assert "whilestate.get()<end{" in driver, "init driver guard moved"
    loop = driver[driver.index("whilestate.get()<end{"):]
    assert loop.startswith("whilestate.get()<end{letindex=state.get();"), "init driver guard moved"
    assert loop.count("letindex=") == 1 and "index=" not in loop.replace("letindex=", "", 1).split("wrapping_add(index,1)")[0], \
        "init driver index rebound before the increment"
    assert driver.count("wrapping_add(") == 1 and "*state=Checked::new(wrapping_add(index,1));" in loop, "init driver increment changed"
    staging = compact(texts["src/vec/staging_driver.rs"])
    assert staging.count("wrapping_add(") == 1 and \
        "iflen>=limit{return(Ok(false),len,cap);}letnext=wrapping_add(len,1);" in staging, "staging driver guard moved"


def coverage(census):
    families = FAMILIES
    guarded_stores({name: (ROOT / name).read_text() for name in ("src/init/driver.rs", "src/vec/staging_driver.rs")})
    found, ledger = {}, []
    for row in census:
        kind = row["kind"]
        if kind == "modular_import":
            assert not re.search(r"\bas\b", row["expression"]), "modular alias requires an explicit proof binding"
        elif kind == "modular_method":
            assert row["file"] == "src/arithmetic/bounded.rs", ("unbound modular method", row)
        elif kind == "modular_macro":
            assert row["file"] == "src/arithmetic.rs" and "unsafe" not in row["expression"], row
        elif kind == "modular_reference":
            raise AssertionError(("unbound modular function reference", row))
        elif kind == "modular_call":
            key = (row["file"], row["context"].split("<")[0].strip(), row["function"])
            assert key in families, ("modular call has no verified contract", row)
            found[key] = found.get(key, 0) + 1
            ledger.append(dict(site=row, contracts=families[key][1], premise=row_premise(families[key][1])))
    assert {k: v[0] for k, v in families.items()} == found, (families, found)
    return ledger


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mutations", action="store_true")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "summary.json").unlink(missing_ok=True)
    env = dict(os.environ, PATH=str(VERUS.parent) + os.pathsep + os.environ["PATH"])
    def run(name, command, extra=None):
        with (OUT / (name + ".log")).open("w") as log:
            result = subprocess.run(command, cwd=ROOT, env=dict(env, **(extra or {})), stdout=log, stderr=subprocess.STDOUT)
        assert result.returncode == 0, (name, result.returncode, str(OUT / (name + ".log")))
        return dict(command=command, environment=extra or {}, exit_code=result.returncode)
    inputs = sorted((ROOT / "src").rglob("*.rs")) + sorted((ROOT / "tools/memory-proof").glob("*.verus.rs"))
    inputs += sorted((ROOT / "tools/direct-verus").glob("*.rs")) + [ROOT / "scripts/verus_composition.py"]
    inputs += sorted((ROOT / "tools/panic-audit/src").rglob("*.rs"))
    inputs += [ROOT / "tools/panic-audit/Cargo.toml", ROOT / "tools/panic-audit/Cargo.lock"]
    inputs += [ROOT / "scripts" / name for name in ["verify_wrapping_memory.py", "premise_status.py", "verify_source.py", "verify_panic_state.py", "verify_state_types.py", "arithmetic_projection.py", "btree_bindings.py", "hash_map_bindings.py", "verify_scratch_slice.py"]]
    fingerprint = lambda: hashlib.sha256(b"".join(str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes() for p in inputs)).hexdigest()
    before = fingerprint()
    report = dict(source_sha256=before, results={},
        utc=datetime.now(timezone.utc).isoformat(),
        baseline_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
        kani=subprocess.check_output(["cargo", "kani", "--version"], text=True),
        verus=subprocess.check_output([str(VERUS), "--version"], text=True, env=env))
    controls = ["--mutations"] if args.mutations else []
    report["results"]["numeric"] = run("numeric", ["python3", "scripts/verify_panic_state.py", *controls])
    report["results"]["vec-cursors"] = run("vec-cursors", ["python3", "scripts/verify_source.py", "--group", "vec-cursors", "--output", str(OUT / "vec-cursors"), *controls])
    cursor_report=json.loads((OUT / "vec-cursors/summary.json").read_text())
    assert cursor_report["status"]=="pass" and cursor_report["source_unchanged"]
    run("parser-build", ["cargo", "build", "--locked", "--offline", "--manifest-path", "tools/panic-audit/Cargo.toml", "--bin", "memory", "--target-dir", "target/panic-inventory"])
    validate_bounded(ROOT)
    census = inventory(ROOT)
    ledger = coverage(census)
    (OUT / "memory-inventory.json").write_text(json.dumps(census, indent=2))
    generated, expressions, core = generate(ROOT, census)
    base = (ROOT / "target/verification/panic-state/panic-state.generated.rs").read_text()
    # Use this run's numeric prerequisite, not a standalone development output.
    # An earlier state-types run may describe a different compaction source.
    state_dir = ROOT / "target/verification/panic-state/closed-state-types"
    state_report = json.loads((state_dir / "summary.json").read_text())
    for file in ('src/compaction.rs', 'src/span.rs'):
        assert state_report['source_sha256'][file] == hashlib.sha256((ROOT / file).read_bytes()).hexdigest()
    without_main = lambda text: re.sub(r"fn main\(\)\s*\{\}", "", text)
    combined = without_main(base) + generated
    # Exec and proof functions only: a spec fn of the same name is not a proof.
    known = verified_function_names(combined + "".join((ROOT / "tools/direct-verus" / name).read_text() for name in
        ("state-adapters.rs", "budget-adapters.rs", "vec-adapters.rs", "layout-adapters.rs", "init-adapters.rs")))
    for row in ledger:
        direct = [c for c in row["contracts"] if c.startswith("direct:")]
        for contract in direct:
            # The enclosing product function itself carries the Verus
            # specification that the named direct target verifies.
            from verify_direct_verus import TARGETS
            assert contract[len("direct:"):] in TARGETS, contract
            site = row["site"]
            source = (ROOT / site["file"]).read_text()
            head = re.search(r"\bfn\s+" + site["function"] + r"\b", source)
            assert head and "verus_spec(" in source[source.rfind("\n\n", 0, head.start()):head.start()], \
                ("direct contract without a verus_spec on the enclosing function", site)
        for contract in [c for c in row["contracts"] if not c.startswith("direct:")]: assert contract in known, contract
    path = OUT / "memory.generated.rs"; path.write_text(combined)
    report["results"]["verus"] = run("memory-verus", [str(VERUS), str(ROOT / "src/lib.rs"),
        "--crate-type", "lib", "--edition=2024", "--cfg", 'feature="alloc"',
        "--cfg", "jzap_proof_composition", "--crate-name", "jzap_wrapping_memory", "--no-cheating",
        *verus_std_specs.build(VERUS)],
        {"JZAP_PROOF_COMPOSITION": str(path)})
    # Verus's unpinned usize domain is Either32Or64. Original state methods and
    # caller contracts are checked together; no usize -> u32 text translation.
    report["word_domain"] = state_report["word_domain"]
    report.update(core_index=core, expressions=expressions, coverage=ledger, covered_modular_calls=len(ledger),
                  premise_established_calls=sum(row["premise"] == "established" for row in ledger),
                  premise_stated_not_proved_calls=sum(row["premise"] == "stated-not-proved" for row in ledger))
    proof_source = (ROOT / "src/wrapping_memory_proofs.rs").read_text()
    names = re.findall(r"^fn ((?:every_wrapping_result_|admitted_|all_word_range_)\w+)\(", proof_source, re.M)
    names += ["full_constructor_establishes_order", "both_transitions_preserve_order_and_exact_length", "constructors_establish_every_numeric_bound", "both_transitions_close_all_invariant_states_and_queries", "constructors_and_all_transitions_preserve_range_and_cleanup", "constructors_and_reconstructed_layout_preserve_each_byte_bound"]
    names += ["local_cursor_publication_preserves_every_word"]
    command = ["cargo", "kani", "--lib", "--solver", "cadical", "--harness-timeout", "180s", "-Z", "unstable-options"]
    for name in names: command += ["--harness", name]
    command += ["--cbmc-args", "--object-bits", "10"]
    report["results"]["kani"] = run("memory-kani", command)
    assert f"Complete - {len(names)} successfully verified harnesses, 0 failures, {len(names)} total." in (OUT / "memory-kani.log").read_text()
    report["kani_harnesses"] = names
    report["direct_cursor_harnesses"] = [name.rsplit("::",1)[-1] for name in cursor_report["groups"][0]["kani"]["harnesses"]]
    assert len(report["direct_cursor_harnesses"])==4
    if args.mutations:
        report["mutations"] = verify_mutations(env, census, combined, generated)
    report["source_unchanged"] = before == fingerprint()
    assert report["source_unchanged"]
    report["status"] = "pass"
    (OUT / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{len(ledger)} internal modular calls covered ({report['premise_established_calls']} with established premises, "
          f"{report['premise_stated_not_proved_calls']} with stated premises not proved); "
          f"source-linked byte spans and {len(names)} real Rust/state harnesses verified.")


def verify_mutations(env, census, combined, generated):
    results = []
    # Move a destination beyond the allocated end in the source. The caller no
    # longer has the reviewed routing to its kernel and must be refused.
    from arithmetic_projection import replace_once
    with tempfile.TemporaryDirectory(prefix="jzap-wrapped-span-") as tmp:
        root = Path(tmp); shutil.copytree(ROOT / "src", root / "src")
        file = root / "src/vec.rs"; text = file.read_text()
        text = replace_once(text, "core::ptr::copy(place, place.add(1), shifted.get());", "core::ptr::copy(place, place.add(2), shifted.get());")
        file.write_text(text)
        try: generate(root, inventory(root))
        except AssertionError as error: assert "single-element move routing" in str(error), error
        else: raise AssertionError("moved destination accepted")
    results.append(dict(name="destination_routing", detected=True, mechanism="caller body differs from the reviewed kernel routing"))
    # The same defect in the maintained span statement must reach the solver.
    mutated = replace_once(combined, "memory_span(accepted_cap, size,( index + 1) as int,",
                           "memory_span(accepted_cap, size,( index + 2) as int,")
    path = OUT / "mutation-destination.generated.rs"; path.write_text(mutated)
    _, result = compose(VERUS, ROOT, path, "bad_span", env)
    text = result.stdout + result.stderr; (OUT / "mutation-destination.log").write_text(text)
    assert result.returncode and re.search(r"verification results:: \d+ verified, [1-9]\d* errors", text) and "error[E" not in text
    results.append(dict(name="destination_span", detected=True, exit_code=result.returncode))
    # The guarded increment: without its guard the solver must refuse the
    # product's wrapping_add, and a moved guard must fail the source pin.
    mutated = combined + "\nverus! { fn unguarded_increment(current: usize) -> usize { crate::arithmetic::bounded::wrapping_add(current, 1) } }\n"
    path = OUT / "mutation-unguarded.generated.rs"; path.write_text(mutated)
    _, result = compose(VERUS, ROOT, path, "unguarded_increment", env)
    text = result.stdout + result.stderr; (OUT / "mutation-unguarded.log").write_text(text)
    assert result.returncode and re.search(r"verification results:: \d+ verified, 1 errors", text) and "error[E" not in text \
        and "unguarded_increment" in text, text[-3000:]
    results.append(dict(name="unguarded_increment", detected=True, exit_code=result.returncode))
    drivers = {name: (ROOT / name).read_text() for name in ("src/init/driver.rs", "src/vec/staging_driver.rs")}
    for name, file, before, after in [
        ("init_guard_moved", "src/init/driver.rs", "while state.get() < end {", "while state.get() <= end {"),
        ("staging_guard_moved", "src/vec/staging_driver.rs", "if len >= limit {", "if len > limit {"),
    ]:
        try: guarded_stores({**drivers, file: replace_once(drivers[file], before, after)})
        except AssertionError: results.append(dict(name=name, detected=True))
        else: raise AssertionError("moved guard accepted: " + name)
    # Direct calls, aliased imports and macro bodies cannot evade this census.
    for name, extra in [
        ("unbound_modular_call", "fn unbound(value:usize)->usize { value.wrapping_add(1) }"),
        ("aliased_modular_call", "use crate::arithmetic::bounded::wrapping_add as add_hidden; fn unbound(value:usize)->usize { add_hidden(value,1) }"),
        ("macro_modular_call", "macro_rules! unbound { ($x:expr) => { $x.wrap_add(1) }; }"),
    ]:
        with tempfile.TemporaryDirectory(prefix="jzap-wrapped-call-") as tmp:
            root = Path(tmp); shutil.copytree(ROOT / "src", root / "src")
            file = root / "src/vec.rs"
            file.write_text(file.read_text() + "\n" + extra + "\n")
            try: coverage(inventory(root))
            except AssertionError: results.append(dict(name=name, detected=True))
            else: raise AssertionError("unbound modular call accepted: " + name)
    # The independent default Kani object checks must catch the real memory bug,
    # not a compiler error or merely a failure of the model translator.
    with tempfile.TemporaryDirectory(prefix="jzap-wrapped-index-") as tmp:
        root = Path(tmp)
        for folder in ["src", "examples", "tests"]: shutil.copytree(ROOT / folder, root / folder)
        for name in ["Cargo.toml", "Cargo.lock"]: shutil.copy2(ROOT / name, root / name)
        file = root / "src/vec.rs"; text = file.read_text()
        start = text.index("pub fn remove("); opening = text.index("{", start)
        end, depth = opening + 1, 1
        while depth:
            depth += (text[end] == "{") - (text[end] == "}"); end += 1
        block = text[opening:end]
        changed = replace_once(block, "self.ptr.as_ptr().add(index)", "self.ptr.as_ptr().add(self.cap.get())")
        text = text[:opening] + changed + text[end:]
        file.write_text(text)
        command = ["cargo", "kani", "--lib", "--harness", "admitted_indices_are_bounded_before_single_element_moves", "--harness-timeout", "180s", "--solver", "cadical", "-Z", "unstable-options", "--cbmc-args", "--object-bits", "10"]
        result = subprocess.run(command, cwd=root, env=dict(env, CARGO_TARGET_DIR=str(OUT / "mutation-cargo")), capture_output=True, text=True)
        log = result.stdout + result.stderr; (OUT / "mutation-one-past-read.log").write_text(log)
        assert result.returncode and "VERIFICATION:- FAILED" in log and "FAILURE" in log and "error[E" not in log, log[-3000:]
        blocks = re.split(r"\nCheck \d+:", log)
        failed_memory = [block for block in blocks if "Status: FAILURE" in block
            and any(marker in block.lower() for marker in ["pointer dereference", "outside object bounds", "object bounds"]) ]
        assert failed_memory, log[-3000:]
        results.append(dict(name="one_past_allocation_read", detected=True, exit_code=result.returncode,
                            failed_memory_checks=[block.split("\n\n")[0] for block in failed_memory]))
    (OUT / "mutations.json").write_text(json.dumps(results, indent=2) + "\n")
    print("Negative controls:", results)
    return results


if __name__ == "__main__":
    main()
