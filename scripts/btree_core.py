"""Source-connected core guard projections reached by BTreeMap consumers.

Formatting callbacks are abstract success/error replies. They borrow Formatter,
not the parent builder's private protocol flags. All writes precede the captured
flag assignment, so a returned error conservatively keeps the old flag.
"""
import hashlib,re,subprocess
from pathlib import Path

def compact(text):return re.sub(r'\s+','',re.sub(r'//[^\n]*','',text))

def rust_block(source,marker):
    # Preserve offsets while masking strings/comments, including literal braces.
    tokens=r'//[^\n]*|/\*.*?\*/|r(#+)".*?"\1|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\''
    masked=re.sub(tokens,lambda m:' '*len(m[0]),source,flags=re.S)
    at=masked.index('{',source.index(marker));end=at+1;depth=1
    while depth:
        depth+=(masked[end]=='{')-(masked[end]=='}');end+=1
    return source[at+1:end-1]

def projection(root,core_root=None):
    root=Path(root)
    if core_root is None:
        sysroot=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())
        core_root=sysroot/'lib/rustlib/src/rust/library/core/src'
    core_root=Path(core_root)
    paths=[core_root/'fmt/builders.rs',core_root/'num/uint_macros.rs']
    builders,uints=(p.read_text() for p in paths)
    start=builders.index("impl<'a, 'b: 'a> DebugMap")
    methods=builders[start:]
    new=rust_block(builders,'fn debug_map_new')
    assert 'has_key:false' in compact(new)
    assert compact(rust_block(methods,'pub fn entry('))=='self.key(key).value(value)'
    assert compact(rust_block(methods,'pub fn key('))=='self.key_with(|f|key.fmt(f))'
    assert compact(rust_block(methods,'pub fn value('))=='self.value_with(|f|value.fmt(f))'
    assert compact(rust_block(methods,'pub fn entries<'))=='for(k,v)inentries{self.entry(&k,&v);}self'
    flags={};guards={};bodies={}
    for name,wanted in [('key_with',True),('value_with',False),('finish',None)]:
        body=rust_block(methods,'pub fn '+name+('<' if name.endswith('_with') else '('));bodies[name]=body
        normalized=compact(body)
        assert normalized.startswith('self.result=self.result.and_then(|_|{assert!('),(name,normalized)
        assert len(re.findall(r'\bassert!\(',body))==1 and not re.search(r'\b(?:panic|abort)!?\(',body)
        guard=re.search(r'assert!\(\s*(!?self\.has_key)',body)[1];guards[name]=guard.replace('self.has_key','s.pending')
        writes=re.findall(r'self\.has_key\s*=\s*(true|false)',body)
        assert len(writes)==(0 if wanted is None else 1),(name,writes)
        if wanted is not None:
            flags[name]=writes[0]
            # No callback/error exit after successful flag publication.
            tail=normalized[normalized.index('self.has_key='):]
            expected='self.has_key='+writes[0]+';Ok(())});'+('self.has_fields=true;' if name=='value_with' else '')+'self'
            assert tail==expected,(name,tail)
    product=(root/'src/collections/btree_map.rs').read_text()
    uses=[(str(p.relative_to(root)),len(re.findall(r'\.debug_map\(',p.read_text()))) for p in (root/'src').rglob('*.rs')]
    assert sorted((p,n) for p,n in uses if n)==[('src/collections/btree_map.rs',1),('src/collections/hash_map.rs',1)],'unbound product debug-map constructor'
    assert compact(rust_block(product,'fn fmt('))=='f.debug_map().entries(self.iter()).finish()','BTree debug_map call protocol changed'
    assert compact(rust_block((root/'src/collections/hash_map.rs').read_text(),'fn fmt('))=='f.debug_map().entries(self.iter()).finish()','HashMap debug-map protocol changed'
    multiple=rust_block(uints,'pub const fn is_multiple_of(')
    assert compact(multiple)=='matchrhs{0=>self==0,_=>self%rhs==0,}'
    wrapping=compact(rust_block(uints,'pub const fn wrapping_shl('))
    assert wrapping=='unsafe{self.unchecked_shl(rhs&(Self::BITS-1))}'
    unchecked=rust_block(uints,'pub const unsafe fn unchecked_shl(')
    assert ')=>rhs<<$ActualT>::BITS,' in compact(unchecked)
    parts=['''#[derive(Copy,Clone)]
struct BtreeFmtState {pending:bool,ok:bool}
fn btree_core_map_new(ok:bool)->(s:BtreeFmtState)
 ensures !s.pending,
{BtreeFmtState{pending:false,ok}}
''']
    for name,condition,post in [('key_with','!s.pending','out.pending'),('value_with','s.pending','!out.pending')]:
        parts.append('fn btree_core_map_'+name+'(mut s:BtreeFmtState,write_ok:bool)->(out:BtreeFmtState)\n requires s.ok ==> '+condition+',\n ensures out.ok ==> '+post+',out.ok==(s.ok&&write_ok),out.pending==if s.ok&&write_ok{'+flags[name]+'}else{s.pending},\n{if s.ok{assert('+guards[name]+');if write_ok{s.pending='+flags[name]+';}else{s.ok=false;}}s}\n')
    parts.append('''spec fn btree_fmt_entry(s:BtreeFmtState,key_ok:bool,value_ok:bool)->BtreeFmtState {
 BtreeFmtState{ok:s.ok&&key_ok&&value_ok,pending:if !s.ok||!key_ok{s.pending}else{!value_ok}}
}
fn btree_core_map_entry(s:BtreeFmtState,key_ok:bool,value_ok:bool)->(out:BtreeFmtState)
 requires s.ok ==> !s.pending,
 ensures out.ok ==> !out.pending,out.ok==btree_fmt_entry(s,key_ok,value_ok).ok,
  out.pending==btree_fmt_entry(s,key_ok,value_ok).pending,
{btree_core_map_value_with(btree_core_map_key_with(s,key_ok),value_ok)}
fn btree_core_map_finish(s:&BtreeFmtState)
 requires s.ok ==> !s.pending,
{if s.ok {assert('''+guards['finish']+''');}}
spec fn btree_fmt_entries(s:BtreeFmtState,replies:Seq<(bool,bool)>)->BtreeFmtState
 decreases replies.len(),
{if replies.len()==0{s}else{btree_fmt_entries(btree_fmt_entry(s,replies[0].0,replies[0].1),replies.subrange(1,replies.len() as int))}}
proof fn btree_core_map_entries(start:BtreeFmtState,replies:Seq<(bool,bool)>)
 requires start.ok ==> !start.pending,
 ensures btree_fmt_entries(start,replies).ok ==> !btree_fmt_entries(start,replies).pending,
 decreases replies.len(),
{if replies.len()>0 {
 let next=btree_fmt_entry(start,replies[0].0,replies[0].1);
 assert(next.ok ==> !next.pending);
 btree_core_map_entries(next,replies.subrange(1,replies.len() as int));
}}
fn btree_core_multiple_of(value:usize,rhs:usize)->bool
{'''+multiple.replace('self','value')+'''}
fn btree_core_u16_shift(rhs:u32)->(masked:u32)
 ensures masked<16,
{let masked=rhs&(16-1);assert((rhs&15u32)<16u32) by(bit_vector);masked}
''')
    metadata=dict(files={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},
        fmt_guards=guards,fmt_flag_writes=flags,scope='Actual private flag guards and publication order; formatting writes are arbitrary returning success/error replies. Parent builder flags remain inaccessible to safe payload callbacks.')
    return '\n'.join(parts),metadata,paths
