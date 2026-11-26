#!/usr/bin/env python3
"""Verify source-linked numeric transitions and accounting composition.

Pointer provenance and arbitrary payload code are outside these arithmetic
models. A passing model is not a certificate for an unbound final-link edge.
"""
from arithmetic_projection import bounded_projection, unparenthesized, replace_once, validate_bounded
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import shutil
import tempfile
from verus_composition import run as compose

ROOT = Path(__file__).resolve().parents[1]


def body(text, marker):
    start = text.index("{", text.index(marker))
    end, depth = start + 1, 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start + 1:end - 1]


def check(source_root, out):
    validate_bounded(source_root)
    out.mkdir(parents=True, exist_ok=True)
    source_files = sorted(p for p in (source_root / "src").rglob("*.rs") if not p.name.endswith("proofs.rs"))
    source_hash = hashlib.sha256(b"".join(
        str(p.relative_to(source_root)).encode() + b"\0" + p.read_bytes() for p in source_files)).hexdigest()
    subprocess.run(["cargo", "build", "--locked", "--offline", "--manifest-path",
                    "tools/panic-audit/Cargo.toml", "--target-dir", "target/panic-inventory"],
                   cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    inventory = json.loads(subprocess.check_output(
        [str(ROOT / "target/panic-inventory/debug/jzap-panic-inventory"),
         *(str(p.relative_to(source_root)) for p in source_files)], cwd=source_root, text=True))
    # Preserve real calls and locations; selectors use the mathematical
    # projection whose native overflow checks the generated contracts prove.
    for row in inventory:
        row["source_expression"] = row["expression"]
        if row.get("bounded_wrapping"):
            row["expression"] = unparenthesized(bounded_projection(row["expression"]))
    bindings = {}

    def select(file, function, prefix, count=1):
        rows = [r for r in inventory if r["file"] == file and r["function"] == function
                and r["expression"].startswith(prefix)
                and r["kind"] in ("add", "subtract", "multiply")]
        assert len(rows) == count, (file, function, prefix, rows)
        return rows

    replacements = {
        "self . drain . owner . max_len ()": "limit",
        # The owner's words are read through the getters of its closed extent.
        "self . drain . owner . extent . length ()": "owner_len",
        "self . drain . owner . extent . capacity ()": "cap",
        "guard . owner . extent . length ()": "len",
        "extra . extent . length ()": "extra_len",
        "self . extent . capacity ()": "cap", "self . extent . length ()": "len",
        "self . drain . owner . len": "owner_len",
        "self . drain . owner . cap": "cap",
        "self . drain . tail_start": "tail_start",
        "self . drain . tail_len": "tail_len",
        "guard . owner . len": "len",
        "self . max_len ()": "limit", "self . cap": "cap", "self . len": "len",
        "self . front": "front", "self . back": "back", "self . position": "position",
        "mem :: size_of :: < T > ()": "size", "extra . len": "extra_len", "new_len": "target",
    }

    def translate(row):
        text = row["expression"]
        for old, new in replacements.items():
            text = text.replace(old, new)
        return text

    def bind(name, rows, index=0, statement=False):
        bindings[name] = dict(sites=rows, expression=translate(rows[index]) + (";" if statement else ""))

    # Transparent checked operations are projected to their success value. The
    # arithmetic verifier proves their full-input Result semantics separately;
    # these numeric contracts prove that success is possible under each bound.
    def normalized(text):
        text = re.sub(r"\.\s*get\s*\(\s*\)", "", text)
        marker = "Checked :: new"
        while marker in text:
            start = text.index(marker); opening = text.index("(", start)
            end, depth = opening + 1, 1
            while depth:
                depth += (text[end] == "(") - (text[end] == ")"); end += 1
            text = text[:start] + "(" + text[opening + 1:end - 1] + ")" + text[end:]
        text = re.sub(r"\.\s*map_err\s*\([^)]*\)\s*\?", "", text)
        for old, new in replacements.items(): text = text.replace(old, new)
        for old,new in [("self . count","cap"),("prefix . len","prefix"),
                        ("initialized . len","len"),("self . len","len")]:
            text=text.replace(old,new)
        return text

    def pick(file, function, includes, count=1):
        rows=[r for r in inventory if r["file"]==file and r["function"]==function
              and includes in r["expression"] and r["kind"] in ("add","subtract","multiply")]
        assert len(rows)==count,(file,function,includes,rows)
        return rows

    def projected(name, rows, assignment=None, index=0):
        expression=normalized(rows[index]["expression"])
        if assignment: expression=assignment+" = "+expression+";"
        elif expression.rstrip().endswith(";"): pass
        bindings[name]=dict(sites=rows,expression=expression,
            lowering="Checked success-value projection; overflow absence is proved here under the stated state bounds.")

    capacity_body=body((source_root/"src/capacity.rs").read_text(),"pub(crate) fn layout(")
    capacity_call="letbytes=wrapping_mul(self.count.get(),mem::size_of::<T>());"
    assert capacity_call in re.sub(r"\s+","",capacity_body), "Unbound actual capacity byte calculation"
    bindings["DIRECT_CAPACITY_BYTES"]=dict(sites=pick("src/capacity.rs","layout","self . count"),
        related_proofs="Original bounded::wrapping_mul plus Capacity actual-Rust constructors/layout checks")
    vector_queries=(source_root/"src/vec.rs").read_text()
    convenience_queries=(source_root/"src/vec/convenience.rs").read_text()
    compact_query=lambda text: re.sub(r"\s+","",re.sub(r"//[^\n]*","",text))
    assert compact_query(body(vector_queries,"pub fn remaining_limit("))=="crate::arithmetic::bounded::wrapping_sub(self.max_len(),self.extent.len())", "Unbound remaining-limit query"
    spare_body=compact_query(body(convenience_queries,"pub fn spare_capacity_mut("))
    split_body=compact_query(body(convenience_queries,"pub fn split_at_spare_mut("))
    suffix="core::slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.extent.len()).cast(),crate::arithmetic::bounded::wrapping_sub(self.extent.cap(),self.extent.len()),)"
    assert spare_body=="unsafe{"+suffix+"}", "Unbound spare-capacity query"
    assert split_body=="unsafe{(core::slice::from_raw_parts_mut(self.ptr.as_ptr(),self.extent.len()),"+suffix+",)}", "Unbound spare partition query"
    bindings["DIRECT_VEC_SCALAR_QUERIES"]=dict(
        related_proofs="Original bounded::wrapping_sub, original length policies, and vec::query_proofs::every_valid_zst_owner_query_and_spare_partition",
        boundary="All-word numeric queries and actual ZST owner states; nonzero-element provenance/aliasing remain with the existing physical scenarios.")
    ranges=pick("src/vec/convenience.rs","try_extend_from_within","Checked :: new (end)")+pick("src/vec/convenience.rs","try_extend_copy_from_within","Checked :: new (end)")
    projected("INTERNAL_RANGE_LENGTH",ranges)
    projected("CLONE_INPUT_OFFSET",pick("src/vec.rs","try_extend_cloned","Checked :: new (index)"))
    projected("SPLIT_LENGTH",pick("src/vec.rs","try_split_off","self . extent . length () -"))
    projected("DRAIN_TAIL_LENGTH",pick("src/vec/edit.rs","try_drain","self . extent . length () -"))
    layout=body((source_root/"src/layout.rs").read_text(),"pub fn array<T>(")
    expected="letsize=core::mem::size_of::<T>();letbytes=numeric::array_bytes(count,size)?;Layout::from_size_align(bytes,core::mem::align_of::<T>()).map_err(|_|Error::Layout)"
    assert re.sub(r"\s+","",re.sub(r"//[^\n]*","",layout))==expected, "Unbound actual array-layout numeric arguments"
    vector=(source_root/"src/vec.rs").read_text()
    split=body(vector,"pub fn try_split_off(")
    bindings["SPLIT_REJECT"]=dict(expression=re.search(r"if (.*?) \{",split,re.S).group(1).replace("self.extent.len()","len"))
    reserve_sites=[]
    for fn in ["try_reserve","try_reserve_exact"]:
        method=body(vector,"pub fn "+fn+"(")
        assert "if additional > spare.get()" in method
        reserve_sites+=pick("src/vec.rs",fn,"self . extent . capacity () -")
    bindings["RESERVE_GROW"]=dict(sites=reserve_sites,expression="additional > ("+normalized(reserve_sites[0]["expression"])+")")
    convenience=(source_root/"src/vec/convenience.rs").read_text()
    owned=body(convenience,"pub fn try_resize_owned(")
    condition=re.search(r"if (.*?) \{",owned,re.S).group(1)
    bindings["RESIZE_SHRINK"]=dict(expression=condition.replace("new_len","target").replace("self.extent.len()","len"))
    projected("RESIZE_ADDITIONAL",pick("src/vec/convenience.rs","try_resize_owned","Checked :: new (new_len) - self"))
    condition=re.search(r"while (.*?) \{",owned,re.S).group(1)
    bindings["OWNED_RESIZE_GUARD"]=dict(expression=condition.replace("guard.owner.extent.len()","len").replace("last","(target - 1)"),sites=pick("src/vec/convenience.rs","try_resize_owned","Checked :: new (new_len) - Checked :: new (1)"))
    rows=pick("src/vec/convenience.rs","try_resize_owned","guard . owner . extent . length () +",2)
    projected("OWNED_RESIZE_ADVANCE",rows,assignment="len")
    projected("OWNED_RESIZE_FINAL",rows,assignment="len",index=1)
    projected("WITHIN_ADVANCE",pick("src/vec/convenience.rs","try_extend_from_within","guard . owner . extent . length () +"),assignment="len")
    normal_resize=body(vector,"pub fn try_resize_with_init<E>(")
    condition=re.search(r"if (.*?) \{",normal_resize,re.S).group(1)
    bindings["INIT_RESIZE_SHRINK"]=dict(expression=condition.replace("new_len","target").replace("self.extent.len()","len"))
    condition=re.search(r"while (.*?) \{",normal_resize,re.S).group(1)
    bindings["INIT_RESIZE_GUARD"]=dict(expression=condition.replace("guard.owner.extent.len()","len").replace("new_len","target"))
    projected("INIT_RESIZE_ADDITIONAL",pick("src/vec.rs","try_resize_with_init","Checked :: new (new_len)"))
    projected("INIT_RESIZE_ADVANCE",pick("src/vec.rs","try_resize_with_init","guard . owner . extent . length () +"),assignment="len")
    assert normal_resize.index("initialize(index, &mut slot)") < normal_resize.index("slot.finish()") < normal_resize.index(".set_len_within(next.get())")
    # The native UTF-8 cursor still has a maximum of four encoded bytes.
    projected("UTF8_ADVANCE",pick("src/boxed/text.rs","try_next","self . position"),assignment="position")
    projected("UTF8_REMAINING",pick("src/boxed/text.rs","try_size_hint","self . len -"))
    edit=(source_root/"src/vec/edit.rs").read_text()
    projected("SPLICE_TAIL_DESTINATION",pick("src/vec/edit.rs","move_tail","Checked :: new (self . drain . tail_start)"))
    projected("SPLICE_TAIL_ADVANCE",pick("src/vec/edit.rs","move_tail","Checked :: new (self . drain . tail_start)"),assignment="tail_start")
    # Finish contains one limit expression and one capacity expression.
    available=[r for r in inventory if r["file"]=="src/vec/edit.rs" and r["function"]=="try_finish" and "? - Checked :: new (self . drain . tail_len)" in r["expression"]]
    assert len(available)==2
    projected("SPLICE_AVAILABLE",available)
    projected("SPLICE_SPARE",available,index=1)
    projected("SPLICE_FIRST_RESERVATION",pick("src/vec/edit.rs","try_finish","Checked :: new (self . drain . tail_len) +"))
    from init_bindings import check as init_bindings
    init_bindings(source_root)
    bindings['DIRECT_SPLICE_FILL']=dict(sites=pick('src/vec/edit.rs','fill','Checked :: new (self . drain . tail_start) -'),
        related_proofs='Original initialization cursor and actual all-word Splice hole/tail adapter; no fill-loop or increment projection')
    projected("SPLICE_ADVANCE_ONE",pick("src/vec/edit.rs","try_finish","self . drain . owner . extent . length () +"),assignment="owner_len")
    projected("STAGED_GROWTH_COUNT",pick("src/vec/edit.rs","stage_excess","Checked :: new (needed) -"))
    # The staged leaf owns its length increment since the executable staging controller.
    projected("EXCESS_ADVANCE",pick("src/vec/edit.rs","stage_push","extra . extent . length () +"),assignment="extra_len")
    projected("STAGED_TAIL_DESTINATION",pick("src/vec/edit.rs","stage_excess","Checked :: new (self . drain . tail_start) +"))
    projected("STAGED_TAIL_ADVANCE",pick("src/vec/edit.rs","stage_excess","Checked :: new (self . drain . tail_start) +"),assignment="tail_start")
    projected("STAGED_OWNER_ADVANCE",pick("src/vec/edit.rs","stage_excess","self . drain . owner . extent . length () +"),assignment="owner_len")
    # The representation has closed constructor/transition proofs, so bind the
    # shared arithmetic once rather than reintroducing every old cursor formula.
    state_out=out/"closed-state-types"
    state_command=["python3","scripts/verify_state_types.py","--source-root",str(source_root),"--output",str(state_out)]
    # Each caller mutant still proves the actual state methods. Repeating the
    # unrelated state defects is unnecessary; the unmutated campaign runs all eleven.
    if source_root.resolve()!=ROOT:state_command.append('--no-controls')
    subprocess.run(state_command,cwd=ROOT,check=True)
    state_report=json.loads((state_out/"summary.json").read_text())
    assert state_report["contracts"]==31 and state_report["kind"]=="direct-product-source"
    bindings["CLOSED_COMPACTION_STATES"]=dict(
        sites=[r for r in inventory if r["file"]=="src/compaction.rs" and r["kind"] in ("add","subtract","multiply")],
        report=state_report,boundary="Numeric constructors and all transitions are closed; payload-pointer ownership remains separate.")
    bindings["CLOSED_ORDERED_SPAN"]=dict(
        sites=[r for r in inventory if r["file"]=="src/span.rs" and r["kind"] in ("add","subtract","multiply")],
        report=state_report, related_proofs="span::proofs::{full_constructor_establishes_order,both_transitions_preserve_order_and_exact_length}")
    checked_sites=[]
    for row in inventory:
        expression=row["expression"]
        if row["kind"] not in ("add","subtract","multiply"):continue
        explicit="Checked :: new" in expression
        field=re.match(r"(?:self \. (?:drain \. owner \. |owner \. )?|guard \. owner \. |extra \. )(?:len|cap|count) [+-] ",expression)
        if explicit or field:
            checked_sites.append(row)
    bindings["FALLIBLE_CHECKED_OPERATORS"]=dict(sites=checked_sites,
        related_proofs="arithmetic::proofs::checked_operators_preserve_all_primitive_results",
        boundary="Sealed primitive Checked arithmetic returns errors for every out-of-range input; this does not certify pointer preconditions.")

    # Bridge the accounting lemmas to the real storage operation order. This is
    # an explicit source abstraction boundary, not verification of an allocator.
    storage = (source_root / "src/storage.rs").read_text()
    allocate = body(storage, "fn allocate_impl(")
    reallocate = body(storage, "pub(crate) unsafe fn reallocate(")
    deallocate = body(storage, "pub(crate) unsafe fn deallocate(")
    assert allocate.index("budget.reserve(layout.size())?") < allocate.index("r.commit().retain();")
    assert allocate.index(".ok_or(Error::Allocation") < allocate.index("r.commit().retain();")
    assert reallocate.index("budget.reserve(new.size())?") < reallocate.index("alloc::alloc::realloc")
    assert reallocate.index("reservation.commit().retain();") < reallocate.index("budget.release(old.size());")
    assert deallocate.index("alloc::alloc::dealloc") < deallocate.index("budget.release(layout.size());")
    env = os.environ.copy()
    from verify_direct_verus import verify as direct
    direct(root=source_root,out=out/'foundation',targets=['local-budget','init-prefix','buffer'])
    bindings['DIRECT_BUFFER_RESERVATION']=dict(
        sites=pick('src/boxed/buffer_numeric.rs','reservation','Checked :: new (len)')
            +pick('src/boxed/buffer_numeric.rs','reservation','Checked :: new (cap)'),
        related_proofs='Original numeric reservation contract, actual Buffer word-state publication and actual collection cursor')
    budget='include!('+json.dumps(str(ROOT/'tools/direct-verus/budget-adapters.rs'))+');\n'
    ledger_path = out / "budget-ledger-calls.rs"
    ledger_path.write_text('include!(' + json.dumps(str(ROOT/'tools/direct-verus/budget-ledger.rs'), ensure_ascii=False) + ');\n')
    template = (ROOT / "tools/memory-proof/panic-state.verus.rs").read_text()
    template += '\ninclude!(' + json.dumps(str(ROOT/'tools/direct-verus/init-adapters.rs')) + ');\n'
    for name, binding in bindings.items():
        if "expression" in binding:
            template = re.sub(r"\b" + name + r"\b", lambda _: binding["expression"], template)
    generated = out / "panic-state.generated.rs"
    generated.write_text(budget + "\n" + (state_out / "state-calls.rs").read_text() + "\ninclude!(" + json.dumps(str(ROOT/"tools/direct-verus/vec-adapters.rs"), ensure_ascii=False) + ");\n" + "include!(" + json.dumps(str(ROOT/"tools/direct-verus/layout-adapters.rs"), ensure_ascii=False) + ");\n" + template)
    binary = env.get("JZAP_VERUS", "/workspace/.devtools/verus/verus-x86-linux/verus")
    command, r = compose(binary, source_root, generated, "jzap_panic_state_proof")
    (out / "verus.log").write_text(r.stdout + r.stderr)
    ledger_command, ledger_result = compose(binary, source_root, ledger_path, "jzap_budget_ledger")
    ledger_log = ledger_result.stdout + ledger_result.stderr
    (out / "budget-ledger.log").write_text(ledger_log)
    final_hash = hashlib.sha256(b"".join(
        str(p.relative_to(source_root)).encode() + b"\0" + p.read_bytes() for p in source_files)).hexdigest()
    report = dict(source_sha256=source_hash, bindings=bindings, command=command, exit_code=r.returncode,
                  verification=re.findall(r"verification results:: (\d+) verified, (\d+) errors", r.stdout + r.stderr),
                  ledger=dict(command=ledger_command, kind="actual-numeric-dual-spec-calls",
                      exit_code=ledger_result.returncode,
                      verification=re.findall(r"verification results:: (\d+) verified, (\d+) errors", ledger_log)),
                  source_unchanged=final_hash == source_hash,
                  boundary="Source-linked numeric projection; allocator/pointer validity, Rust-to-ledger ownership refinement and arbitrary payload code are not proved.")
    (out / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(r.stdout + r.stderr + ledger_log)
    return r.returncode or ledger_result.returncode or int(final_hash != source_hash)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mutations", action="store_true")
    args = parser.parse_args()
    out = ROOT / "target/verification/panic-state"
    code = check(ROOT, out)
    if code or not args.mutations:
        return code
    results = []
    cases = [
        ("within_advance", "vec/convenience.rs", "(guard.owner.extent.length() + Checked::new(1)).map_err(|_| Error::Capacity)?;", "(guard.owner.extent.length() + Checked::new(2)).map_err(|_| Error::Capacity)?;"),
        ("utf8_advance", "boxed/text.rs", "self.position = wrapping_add(self.position, 1);", "self.position = wrapping_add(self.position, 2);"),
        ("staged_owner_length", "vec/edit.rs", "(self.drain.owner.extent.length() + extra.extent.length())",
         "(self.drain.owner.extent.length() + Checked::new(1))"),
    ]
    for name, file, old, new in cases:
        with tempfile.TemporaryDirectory(prefix="jzap-panic-state-") as tmp:
            source_root = Path(tmp)
            shutil.copytree(ROOT / "src", source_root / "src")
            path = source_root / "src" / file
            text = path.read_text()
            path.write_text(replace_once(text,old,new))
            mutant_out = out / ("mutation-" + name)
            code = check(source_root, mutant_out)
            log = (mutant_out / "verus.log").read_text()
            semantic_failure = code != 0 and any(int(n) > 0 for n in re.findall(
                r"verification results:: \d+ verified, (\d+) errors", log)) and "error[E" not in log
            assert semantic_failure, (name, log)
            results.append(dict(name=name, detected=True, exit_code=code))
    (out / "mutations.json").write_text(json.dumps(results, indent=2) + "\n")
    print(results)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
