"""Pinned Rust 1.95 core predicate text and the maintained proof program around it.

`pinned()` reads the predicate bodies from the installed core source and checks
their shape; nothing is taken from product source. `program()` places the
documented conditions (core-contracts.verus.rs) in front of the predicate
template. verify_pointer_checks.py proves the program and its controls; the
storage kernel (storage_kernel.py) reuses the same text in area compositions.
"""
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def pinned():
    rustc = subprocess.check_output(["rustc", "--version"], text=True).strip()
    assert rustc.startswith("rustc 1.95.0 "), rustc
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    source_root = sysroot / "lib/rustlib/src/rust/library/core/src"
    inputs = {name: (source_root / name).read_text()
              for name in ["ptr/mod.rs", "ptr/mut_ptr.rs", "ptr/const_ptr.rs", "ub_checks.rs", "slice/index.rs"]}
    replacement=re.search(r'pub const unsafe fn replace<T>.*?(?=\n///)',inputs['ptr/mod.rs'],re.S)
    assert replacement and replacement[0].count('assert_unsafe_precondition!')==1
    replacement_source=re.sub(r'\s+','',re.sub(r'//[^\n]*','',replacement[0]))
    for argument in ['addr:*const()=dstas*const(),','align:usize=align_of::<T>(),','is_zst:bool=T::IS_ZST,']:
        assert argument in replacement_source,('ptr::replace argument changed',argument)
    assert ')=>ub_checks::maybe_is_aligned_and_not_null(addr,align,is_zst)' in replacement_source
    guard=re.search(r'pub\(crate\) const fn maybe_is_aligned_and_not_null\(.*?\) -> bool \{(.*?)\n\}',inputs['ub_checks.rs'],re.S)
    assert guard
    replacement_body=re.sub(r'\s+','',re.sub(r'//[^\n]*','',guard[1]))
    assert replacement_body=='maybe_is_aligned(ptr,align)&&(is_zst||!ptr.is_null())'
    aligned_helper=re.search(r'pub\(crate\) const fn maybe_is_aligned\(.*?\) -> bool \{(.*?)\n\}',inputs['ub_checks.rs'],re.S)
    assert aligned_helper
    assert re.sub(r'\s+','',re.sub(r'//[^\n]*','',aligned_helper[1]))=='const_eval_select!(@capture{ptr:*const(),align:usize}->bool:ifconst{true}else{ptr.is_aligned_to(align)})'
    replacement_body=replacement_body.replace('maybe_is_aligned(ptr,align)','aligned').replace('!ptr.is_null()','base!=0')
    m = re.search(r"(let Some\(byte_offset\) = count\.checked_mul\(size\).*?"
                  r"byte_offset <= \(isize::MAX as usize\) && !overflow)", inputs["ptr/mut_ptr.rs"], re.S)
    assert m
    add_body = m[1].replace("this.addr()", "base")
    old = "let (_, overflow) = base.overflowing_add(byte_offset);"
    assert add_body.count(old) == 1
    add_body = add_body.replace(old, "let overflow = base.checked_add(byte_offset).is_none();")
    # ptr::sub: the same predicate text in the mutable and the const pointer.
    subs = [re.search(r"fn runtime_sub_nowrap\(this: \*const \(\), count: usize, size: usize\) -> bool \{.*?\} else \{\s*(let Some\(byte_offset\) = count\.checked_mul\(size\).*?this\.addr\(\) >= byte_offset)",
                      inputs[name], re.S) for name in ("ptr/mut_ptr.rs", "ptr/const_ptr.rs")]
    assert all(subs) and re.sub(r"\s+", "", subs[0][1]) == re.sub(r"\s+", "", subs[1][1])
    assert subs[0][1].count("this.addr()") == 1
    sub_body = subs[0][1].replace("this.addr()", "base")
    m = re.search(r"fn is_valid_allocation_size\(size: usize, len: usize\) -> bool \{(.*?)\n\}",
                  inputs["ub_checks.rs"], re.S)
    assert m
    size_body = m[1]
    m = re.search(r"let src_usize = src.addr\(\);(.*?)diff >= size", inputs["ub_checks.rs"], re.S)
    assert m
    copy_body = "let src_usize = src;" + m[1] + "diff >= size"
    copy_body = copy_body.replace("dst.addr()", "dst")
    copy_body, n = re.subn(r"else \{\s*crate::panicking::panic_nounwind\(.*?\)\s*\};",
                          "else { return false; };", copy_body, flags=re.S)
    assert n == 1
    assert "let diff = src_usize.abs_diff(dst_usize);" in copy_body
    copy_body = copy_body.replace("let diff = src_usize.abs_diff(dst_usize);",
        "let diff = if src_usize <= dst_usize { dst_usize - src_usize } else { src_usize - dst_usize };")
    alignment = re.search(r"pub fn is_aligned_to\(self, align: usize\) -> bool \{(.*?)\n    \}",
                          inputs["ptr/const_ptr.rs"], re.S)
    assert alignment and "if !align.is_power_of_two()" in alignment[1]
    assert "self.addr() & (align - 1) == 0" in alignment[1]
    scalar=re.findall(r'\(this: usize = self, len: usize = slice.len\(\)\) => ([^\n]+)',inputs['slice/index.rs'])
    ranges=re.findall(r'\(end: usize = self.end\(\), len: usize = slice.len\(\)\) => ([^\n]+)',inputs['slice/index.rs'])
    assert len(scalar)==2 and len(ranges)==2 and len(set(scalar))==len(set(ranges))==1
    bodies = dict(POINTER_ADD_BODY=add_body, POINTER_SUB_BODY=sub_body, SLICE_SIZE_BODY=size_body, COPY_NONOVERLAP_BODY=copy_body,
                  INDEX_BODY=scalar[0].strip(),INDEX_RANGE_BODY=ranges[0].strip(),REPLACE_BODY=replacement_body)
    return rustc, inputs, bodies


def program():
    """The predicate template with the documented conditions inserted, bodies not yet substituted."""
    template = (ROOT / "tools/memory-proof/pointer-checks.verus.rs").read_text()
    # The documented conditions are maintained Rust, inserted unchanged after
    # the template's imports. They are the premises of every predicate below.
    contracts = (ROOT / "tools/memory-proof/core-contracts.verus.rs").read_text()
    imports, marker, predicates = template.partition("verus! {")
    assert marker and "verus!" not in imports and not re.search(r"^\s*use\s", contracts, re.M)
    return imports + contracts + marker + predicates, contracts


def substituted():
    """The proof program with the pinned bodies in place."""
    _, _, bodies = pinned()
    text, _ = program()
    for key, value in bodies.items():
        text = text.replace(key, value)
    return text
