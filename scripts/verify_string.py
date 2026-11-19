#!/usr/bin/env python3
"""Direct String cursor and edit kernels, retained core models and unsafe routing.

Verus checks executable cursor functions and actual-Rust checks cover callers.
The separate core UTF-8/scalar models,
Rust borrows, allocator contracts and explicit unsafe caller obligations remain
boundaries; Miri/ASan and allocation-failure tests exercise actual pointer code.
"""
import argparse,fcntl,hashlib,json,os,re,subprocess
from pathlib import Path
from scanner_tools import scanner
from verify_edit_loops import block
from verus_composition import run as compose
import storage_kernel
from verify_source import verify as direct
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/string/proofs'
FILES=['src/string.rs','src/string/edit.rs','src/string/convert.rs','src/string/traits.rs','src/string/encoding.rs','src/string/numeric.rs','src/fmt.rs']
def compact(text):return re.sub(r',(?=\))','',re.sub(r'\s+','',text))
def is_string(file):return file=='src/string.rs' or file.startswith('src/string/')
EXPECTED={
 ('src/string/encoding.rs', 'write_scalar'):['ifcode<0x80{destination.write(codeasu8);1}elseifcode<0x800{destination.write(((code>>6)|0xc0)asu8);destination.add(1).write(((code&0x3f)|0x80)asu8);2}elseifcode<0x10000{destination.write(((code>>12)|0xe0)asu8);destination.add(1).write((((code>>6)&0x3f)|0x80)asu8);destination.add(2).write(((code&0x3f)|0x80)asu8);3}else{destination.write(((code>>18)|0xf0)asu8);destination.add(1).write((((code>>12)&0x3f)|0x80)asu8);destination.add(2).write((((code>>6)&0x3f)|0x80)asu8);destination.add(3).write(((code&0x3f)|0x80)asu8);4}'],
 ('src/string.rs', 'try_push'):['encoding::write_scalar(ch,self.bytes.as_mut_ptr().add(plan.start));self.bytes.try_set_len(plan.len)'],
 ('src/string.rs','as_str'):['core::str::from_utf8_unchecked(self.bytes.as_slice())'],
 ('src/string.rs','as_mut_str'):['core::str::from_utf8_unchecked_mut(self.bytes.as_mut_slice())'],
 ('src/string/edit.rs','try_replace_range'):['core::ptr::copy(ptr.add(plan.end),ptr.add(plan.destination),plan.tail);core::ptr::copy_nonoverlapping(text.as_ptr(),ptr.add(plan.start),text.len());self.bytes.try_set_len(plan.len)'],
 ('src/string/edit.rs','try_retain'):['core::str::from_utf8_unchecked(bytes)','letptr=guard.owner.bytes.as_mut_ptr();letsource=ptr.add(guard.read.get());letdestination=ptr.add(guard.written.get());matchwidth{1=>core::ptr::copy(source,destination,1),2=>core::ptr::copy(source,destination,2),3=>core::ptr::copy(source,destination,3),_=>core::ptr::copy(source,destination,4),}'],
 ('src/string/edit.rs','drop'):['letptr=self.owner.bytes.as_mut_ptr();core::ptr::copy(ptr.add(self.read.get()),ptr.add(self.written.get()),remaining);','letowner=self.owner.as_mut();let(remaining,length)=super::numeric::drain(self.start,self.end,owner.len());letptr=owner.bytes.as_mut_ptr();core::ptr::copy(ptr.add(self.end),ptr.add(self.start),remaining);owner.bytes.truncate(length);'],
 ('src/string/convert.rs','into_std'):['alloc::string::String::from_utf8_unchecked(self.bytes.into_std())'],
 ('src/string/convert.rs','try_from_std_in'):['alloc::string::String::from_utf8_unchecked(e.value)'],
 ('src/string/convert.rs','from_raw_parts'):['Vec::from_raw_parts(ptr,len,cap,policy,limit)'],
 ('src/string/convert.rs','try_into_boxed_str'):['crate::boxed::Box::from_utf8_unchecked(bytes)'],
 ('src/string/convert.rs','leak'):['core::str::from_utf8_unchecked_mut(self.bytes.leak())'],
}
from callsite_projection import PINNED_BODIES
RESERVATION={name:PINNED_BODIES[('src/vec.rs',name,'Vec')] for name in ('try_reserve','grow','grow_to','resize_storage')}
# Maintained callers and the product kernels each must call.
CALLERS={'string_replace_caller':['crate::string::edit::Edit::new(len,start,end,inserted,limit)','string_try_reserve(len,cap,limit,plan.additional)','overlapping_copy_caller(s,plan.end,plan.destination,plan.tail)','distinct_nonoverlap_caller(text,s,0,plan.start,inserted)'],
 'string_push_caller':['crate::string::edit::Edit::new(len,len,len,width,usize::MAX)','string_try_reserve(len,cap,limit,plan.additional)','slice_caller(s,plan.start,width)'],
 'string_try_reserve':['crate::vec::required_len(len,additional,limit)','crate::vec::geometry::growth_for_layout(cap,needed,limit,1,false)']}
SOLVER_CONTROLS=[
 ('replace_without_reservation','    if after!=reserved {return;}\n    assert(byte_extent(plan.end','    if after!=cap {return;}\n    assert(byte_extent(plan.end'),
 ('replace_reserves_nothing','let Some(reserved)=string_try_reserve(len,cap,limit,plan.additional) else {return;};\n    if after!=reserved {return;}\n    assert(byte_extent(plan.end','let Some(reserved)=string_try_reserve(len,cap,limit,0) else {return;};\n    if after!=reserved {return;}\n    assert(byte_extent(plan.end'),
 ('push_skips_growth','    if plan.len>cap {\n','    if plan.len>limit {\n'),
 ('replace_text_inside_storage','raw_valid(text),text.cap==inserted,same_element(text,s),apart(text,s),','raw_valid(text),text.cap==inserted,same_element(text,s),'),
 ('replace_tail_by_length','    overlapping_copy_caller(s,plan.end,plan.destination,plan.tail);','    overlapping_copy_caller(s,plan.end,plan.destination,len);'),
 ('push_storage_of_old_capacity','    requires len<=cap, 1<=width<=4,raw_valid(s),s.cap==after,','    requires len<=cap, 1<=width<=4,raw_valid(s),s.cap==cap,'),
 ('reserve_keeps_capacity','        Some(grown)\n    } else {\n        Some(cap)\n    }\n}\n\n// One byte extent','        Some(cap)\n    } else {\n        Some(cap)\n    }\n}\n\n// One byte extent'),
]
def bindings(root=ROOT):
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
 blocks={};out=[]
 for row in rows:
  if row['kind']!='unsafe_block':continue
  expr=compact(row['expression']);assert expr.startswith('unsafe{') and expr.endswith('}')
  expr=expr[7:-1];key=(row['file'],row['function']);i=blocks.get(key,0)
  assert key in EXPECTED and i<len(EXPECTED[key]) and expr==EXPECTED[key][i],('unbound String unsafe argument',row)
  blocks[key]=i+1;out.append(dict(site=row,contract='edit-geometry-and-capacity-admission' if key[1] in ('try_replace_range','try_push') else 'retain-cursor-and-restore' if key[1] in ('try_retain','drop') else 'UTF-8-owner-transfer',boundary='typed storage and UTF-8 invariants; actual-pointer tests'))
 assert blocks=={k:len(v) for k,v in EXPECTED.items()}
 unsafe_fn=[r for r in rows if r['kind']=='unsafe_fn'];assert {r['function'] for r in unsafe_fn}=={'from_raw_parts','from_utf8_unchecked','as_mut_vec','write_scalar'} and len(unsafe_fn)==4
 impls=[r for r in rows if r['kind']=='unsafe_impl'];assert len(impls)==2 and {r['context'] for r in impls}=={"Drain < '_ , P , L >"}
 push=compact(re.sub(r'//[^\n]*','',block((root/'src/string.rs').read_text(),'pub fn try_push(')))
 assert 'letplan=edit::Edit::new(old,old,old,ch.len_utf8(),usize::MAX)?;ifplan.len>self.capacity(){self.bytes.try_reserve(plan.additional)?;}' in push
 owner=(root/'src/string.rs').read_text()
 for name,body in {'len':'self.bytes.len()','capacity':'self.bytes.capacity()','max_len':'self.bytes.max_len()','as_bytes':'self.bytes.as_slice()','is_char_boundary':'self.as_str().is_char_boundary(at)'}.items():
  assert compact(block(owner,'pub fn '+name+'('))==body, ('unbound String accessor',name)
 assert compact(block(owner,'fn boundary('))=='ifat>self.len(){returnErr(Error::Bounds);}if!self.is_char_boundary(at){returnErr(Error::CharBoundary);}Ok(())'
 assert compact(block(owner,'fn range('))=='let(start,end)=crate::vec::edit::checked_range(range,self.len())?;self.boundary(start)?;self.boundary(end)?;Ok((start,end))'
 source=(root/'src/string/edit.rs').read_text();replace=compact(block(source,'pub fn try_replace_range'))
 expected='let(start,end)=self.range(range)?;letplan=Edit::new(self.len(),start,end,text.len(),self.max_len())?;self.bytes.try_reserve(plan.additional)?;letptr=self.bytes.as_mut_ptr();'
 # Strip comments before enforcing complete pre-access ordering/no shadowing.
 clean=compact(re.sub(r'//[^\n]*','',block(source,'pub fn try_replace_range')))
 assert clean.startswith(expected) and clean.count('letplan=')==1 and clean.count('letptr=')==1,'String edit guard/order changed'
 retain=compact(re.sub(r'//[^\n]*','',block(source,'pub fn try_retain')))
 assert 'whileguard.read.get()<original{' in retain and 'ifretain&&guard.written!=guard.read{' in retain
 assert 'letretain=keep(ch)?;letwidth=ch.len_utf8();' in retain
 assert 'ifretain{guard.written=Checked::new(super::numeric::advance(guard.written.get(),width));}guard.read=Checked::new(super::numeric::advance(guard.read.get(),width));' in retain
 # These complete envelopes establish the mathematical preconditions before
 # unsafe effects: no shadowed descriptor or intervening storage mutation.
 assert compact(re.sub(r'//[^\n]*','',block((root/'src/string.rs').read_text(),'pub fn try_push(')))=='letold=self.len();letplan=edit::Edit::new(old,old,old,ch.len_utf8(),usize::MAX)?;ifplan.len>self.capacity(){self.bytes.try_reserve(plan.additional)?;}unsafe{encoding::write_scalar(ch,self.bytes.as_mut_ptr().add(plan.start));self.bytes.try_set_len(plan.len)}', "String phase envelope changed"
 assert compact(re.sub(r'//[^\n]*','',block((root/'src/string/edit.rs').read_text(),'pub fn try_replace_range')))=='let(start,end)=self.range(range)?;letplan=Edit::new(self.len(),start,end,text.len(),self.max_len())?;self.bytes.try_reserve(plan.additional)?;letptr=self.bytes.as_mut_ptr();unsafe{core::ptr::copy(ptr.add(plan.end),ptr.add(plan.destination),plan.tail);core::ptr::copy_nonoverlapping(text.as_ptr(),ptr.add(plan.start),text.len());self.bytes.try_set_len(plan.len)}', "String phase envelope changed"
 assert compact(re.sub(r'//[^\n]*','',block((root/'src/string/edit.rs').read_text(),'pub fn try_drain')))=='let(start,end)=self.range(range)?;letowner=NonNull::from(&mut*self);letchars=self.as_str().get(start..end).ok_or(Error::Bounds)?.chars();Ok(Drain{owner,chars,start,end,marker:PhantomData,})', "String phase envelope changed"
 assert retain.startswith('letoriginal=self.len();letmutguard=Retain{owner:self,read:Checked::new(0),written:Checked::new(0),original:Checked::new(original),};'), "unbound String retain constructor"
 assert 'letSome(bytes)=guard.owner.as_bytes().get(guard.read.get()..)else{break;};' in retain, "unbound String scalar producer"
 drain_drop=compact(re.sub(r'//[^\n]*','',block(source[source.index('Drop for Drain'):],'fn drop')))
 assert drain_drop.startswith('self.chars=\"\".chars();'), 'String drain must end its original borrow before owner access'
 restore=compact(block(source[source.index('Drop for Retain'):],'fn drop'))
 assert restore.startswith('let(remaining,len)=super::numeric::restore(self.read.get(),self.written.get(),self.original.get());'), 'unbound String restoration arguments'
 # The reservation chain whose result `string_try_reserve` reproduces.
 vec=(root/'src/vec.rs').read_text()
 for name,text in RESERVATION.items():
  assert '{'+compact(re.sub(r'//[^\n]*','',block(vec,'fn '+name+'(')))+'}'==compact(text),('String reservation body changed',name)
 owner=(ROOT/'src/string.rs').read_text()
 assert compact(re.sub(r'//[^\n]*','',block(owner,'pub fn pop'))) == 'letmutchars=self.as_str().chars();letch=chars.next_back()?;self.bytes.truncate(chars.as_str().len());Some(ch)'
 return out

def core_bindings():
 sysroot=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())
 base=sysroot/'lib/rustlib/src/rust/library'
 files=['core/src/str/mod.rs','core/src/str/traits.rs','core/src/str/validations.rs','core/src/char/convert.rs','core/src/ptr/non_null.rs','alloc/src/raw_vec/mod.rs','alloc/src/vec/mod.rs','core/src/str/iter.rs','core/src/ptr/const_ptr.rs']
 texts={name:(base/name).read_text() for name in files}
 clean=lambda text:compact(re.sub(r'//[^\n]*','',text))
 boundary=clean(block(texts[files[0]],'pub const fn is_char_boundary'))
 assert boundary=='ifindex==0{returntrue;}ifindex>=self.len(){index==self.len()}else{self.as_bytes()[index].is_utf8_char_boundary()}',boundary
 assert 'end>=start&&end<=len' in clean(texts[files[1]])
 assert 'self.start<=self.end&&slice.is_char_boundary(self.start)&&slice.is_char_boundary(self.end)' in clean(texts[files[1]])
 assert 'char_try_from_u32(i).is_ok()' in clean(texts[files[3]])
 assert 'unchecked_neg()' in texts[files[4]] and 'unchecked_mul' in texts[files[5]]
 # Core's safe str/slice invariants provide complete UTF-8 scalars and a valid
 # allocation extent. Native Kani checks every scalar's forward/reverse decoder
 # and arbitrary UTF-16 pairs; all-word bounds are the Verus obligations above.
 return dict(files={name:hashlib.sha256(text.encode()).hexdigest() for name,text in texts.items()},
  contracts=['string_core_boundary','string_core_range','string_core_str_arithmetic','string_core_byte_cursor','string_core_raw_vec','string_core_negated_byte_step'],
  unicode_producer='actual-source scalar encode/decode Kani for every char; arbitrary UTF-16 pairs; safe str and caller-supplied UTF-8 validity contracts',
  memory_boundary='typed same-allocation byte extents and validated Vec capacity; compiler/Miri/ASan cover actual provenance')

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--mutations',action='store_true');p.add_argument('--kani',action='store_true');args=p.parse_args()
 OUT.mkdir(parents=True,exist_ok=True)
 lock=(OUT/'.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX)
 (OUT/'summary.json').unlink(missing_ok=True)
 inputs=sorted((ROOT/'src').rglob('*.rs'))+sorted((ROOT/'tools/direct-verus').glob('*.rs'))+[Path(__file__),ROOT/'scripts/callsite_projection.py',ROOT/'scripts/verify_source.py',ROOT/'scripts/verify_direct_verus.py',ROOT/'scripts/verus_composition.py',ROOT/'tools/memory-proof/string.verus.rs',*storage_kernel.INPUTS]
 digest=lambda:hashlib.sha256(b''.join(str(x.relative_to(ROOT)).encode()+b'\0'+x.read_bytes() for x in inputs)).hexdigest()
 before=digest();cursor_report=direct(['string-cursors'],OUT/'direct-cursors',args.mutations);core=core_bindings();rows=bindings();text=storage_kernel.kernel()+(ROOT/'tools/memory-proof/string.verus.rs').read_text();expressions={};results=[]
 binary=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))
 def verify(name,text,negative=False):
  path=OUT/(name+'.generated.rs');path.write_text(text)
  env=dict(os.environ,PATH=str(binary.parent)+os.pathsep+os.environ['PATH'])
  command,r=compose(binary,ROOT,path,'jzap_string_proof',env); log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log)
  count=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
  passed=bool(count) and (r.returncode!=0 and any(int(e) for _,e in count) and 'error[E' not in log if negative else r.returncode==0 and all(int(e)==0 for _,e in count))
  assert passed,log[-4000:]
  results.append(dict(name=name,command=command,exit_code=r.returncode,verification=count,negative_control=negative))
 for name,calls in CALLERS.items():
  body=text[text.index('fn '+name+'('):];body=body[:body.index('\n}\n')]
  for call in calls:assert call in body,('maintained String caller lost a product call',name,call)
 verify('runtime-calls-and-core-models',text)
 if args.mutations:
  for name,old,new in SOLVER_CONTROLS:
   assert text.count(old)==1,name
   verify('control-'+name,text.replace(old,new),negative=True)
 if args.mutations:
  import tempfile,shutil
  with tempfile.TemporaryDirectory(prefix='jzap-string-control-') as tmp:
   root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');file=root/'src/string/edit.rs'
   file.write_text(file.read_text().replace('ptr.add(plan.destination)','ptr.add(plan.len)'))
   build=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')],capture_output=True,text=True)
   assert build.returncode==0,build.stderr
   try:bindings(root)
   except AssertionError as e:assert 'unbound String unsafe argument' in str(e)
   else:raise AssertionError('accepted bad destination')
   results.append(dict(name='mutation-pointer-destination',compiled=True,detected=True))
 if args.kani:
  command=['cargo','kani','--lib','--harness','string::proofs::','-Z','unstable-options','--cbmc-args','--object-bits','10']
  with (OUT/'kani.log').open('w') as log:r=subprocess.run(command,cwd=ROOT,stdout=log,stderr=log)
  assert r.returncode==0,(OUT/'kani.log').read_text()[-3000:]
  assert 'Complete - 9 successfully verified harnesses, 0 failures, 9 total.' in (OUT/'kani.log').read_text()
  core['kani_harnesses']=re.findall(r'Checking harness (.*?)\.\.\.',(OUT/'kani.log').read_text())
  assert len(core['kani_harnesses'])==9
  results.append(dict(name='actual-source-kani',command=command,exit_code=r.returncode))
 assert digest()==before,'String verification inputs changed'
 (OUT/'summary.json').write_text(json.dumps(dict(status='pass',source_sha256=before,source_unchanged=True,unsafe_blocks=len(rows),unsafe_functions=4,unsafe_impls=2,bindings=rows,core=core,verified_functions=re.findall(r'fn\s+(\w+)\s*\(',text),expressions=expressions,direct_cursors=cursor_report,word_domain='Verus Either32Or64; no word-type body rewriting',results=results,kani_run=args.kani,scope='Direct executable String cursor arithmetic and actual typed callers; original all-word Edit Kani, retained core UTF-8 models and read-only pointer routing. Typed-owner/provenance boundaries also use compiler/Miri/ASan'),indent=2)+'\n')
 print('String: direct cursor kernels/callers, retained core models, read-only unsafe routing and selected controls pass.')
if __name__=='__main__':main()
