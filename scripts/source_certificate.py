"""Attach source-derived guard proofs to every retained fixture panic edge.

Completion combines verified numeric/state/credit/loop projections, exhaustive
publication and actual-argument checks, the pinned core predicates and a fresh
final-link inventory. A symbol name, hash or finite memory test alone is never
accepted as a numeric/guard certificate. Payload/provenance remain the explicitly
agreed compiler/Miri/unsafe-caller verification boundary.
"""
import argparse,fcntl,hashlib,json,os,py_compile,re,subprocess,time
from pathlib import Path

from certificate_artifacts import collect, digest, identity, publish
from link_call_graph import PAIR_WINDOW_PATH, is_pair_window_edge

ROOT=Path(__file__).resolve().parents[1]
PROFILES='release,release-lto,release-checked,debug'

# Owner prerequisites stay with verify_owner_protocol.py and run from scratch.
STAGES={
    'preflight': ('link-audit panic-audit', ('link-fixture-build','source-routing-build')),
    'budget-native': ('kani', ('budget-native',)),
    'direct-foundation': ('verus kani panic-audit', ('direct-foundation',)),
    'direct-verus': ('verus', ('direct-verus',)),
    'shared': ('verus kani panic-audit shared-loom', ('shared',)),
    'string': ('verus kani panic-audit', ('string',)),
    'btree': ('verus kani panic-audit', ('btree',)),
    'btree-set': ('verus kani panic-audit', ('btree-set',)),
    'hash-map': ('verus kani panic-audit', ('hash-map',)),
    'hash-set': ('verus kani panic-audit', ('hash-set',)),
    'binary-heap': ('verus kani miri panic-audit', ('binary-heap',)),
    'deque': ('verus kani panic-audit', ('deque',)),
    'scratch-slice': ('verus kani panic-audit', ('scratch-slice',)),
    'owner-protocol': ('verus kani panic-audit', ('owner-protocol',)),
    'native-policy': ('kani clippy', ('utf8-native','arithmetic-policy')),
    'link': ('link-audit', ('link',)),
    'bplus': ('verus kani miri consumer-audit', ('bplus-debug','bplus-kani',
              'fixed-bplus-kani','bplus-range-kani','fixed-bplus-miri','fixed-bplus-miri-32','bplus-release',
              'bplus-strided-kani','bplus-strided-miri','bplus-strided-miri-32','bplus-strided-release')),
}
BPLUS='collections::btree_map::view::bplus::'



def inputs():
    files=sorted((ROOT/'src').rglob('*.rs'))
    files+=sorted((ROOT/'scripts').glob('*.py'))
    files+=sorted((ROOT/'tests').rglob('*.rs'))
    files+=sorted((ROOT/'tools/memory-proof').glob('*.verus.rs'))
    files+=sorted((ROOT/'tools/panic-audit/src').rglob('*.rs'))
    files+=sorted(p for p in (ROOT/'tools/link-audit').rglob('*') if p.is_file() and 'target' not in p.relative_to(ROOT).parts)
    files+=sorted(p for p in (ROOT/'tools/shared-loom').rglob('*') if p.is_file() and 'target' not in p.relative_to(ROOT).parts)
    files+=sorted((ROOT/'tools/direct-verus').glob('*.rs'))
    files+=sorted(p for p in (ROOT/'tools/consumer-audit').rglob('*')
                  if p.is_file() and p.suffix in ('.rs','.toml','.lock','.py','.c')
                  and 'target' not in p.relative_to(ROOT).parts)
    files+=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'clippy.toml',
            ROOT/'tools/panic-audit/Cargo.toml',ROOT/'tools/panic-audit/Cargo.lock',
            ROOT/'tools/throughput-analysis/src/bplus_pool.rs']
    return hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode()+b'\0'+p.read_bytes() for p in files)).hexdigest()


def classify(branch):
    name=branch['function']
    if is_pair_window_edge(branch):
        assert branch.get('caller_paths')==[list(PAIR_WINDOW_PATH)],('unbound B+ pair-window caller',branch)
        return 'bplus-pair-window-advance',['pair_window_advance']
    if 'core::slice::index::slice_index_fail' in name:
        return 'forwarding',[]
    if 'core::slice::index::into_slice_range' in name:
        return 'standard-deque-drop-full-range',['normalized_range','direct:deque']
    if 'core::ptr::swap_nonoverlapping::precondition_check' in name:
        return 'standard-slice-rotation-swap',['copy_nonoverlap_precondition','direct:deque','kani:deque::proofs::actual_ring_moves_and_contiguous_growth']
    if 'copy_from_slice_impl::len_mismatch_fail' in name or ('slice::<impl [T]>::' in name and any(n in name for n in ('copy_within','swap_with_slice'))) or 'split_at_mut_unchecked::precondition_check' in name:
        return 'checked-slice-admitted-core-guard',['direct:scratch-slice','kani:slice::proofs::endpoints_all_words','kani:slice::proofs::actual_swap_and_overlap_preserve_checked_state','kani:slice::proofs::actual_copy_swap_and_disjoint_partition_guards','kani:scratch::proofs::actual_zst_all_word_lengths_retire_without_wrap']
    if name.endswith('<core::panicking::panic>:'):
        return 'forwarding',[]
    if 'jzap::deque::VecDeque<T,P,L>::try_from_std_in' in name and any('/core/src/slice/mod.rs:'+str(n) in branch['location'] for n in (3885,3931)):
        return 'std-ring-rotation',['direct:deque','kani:deque::proofs::actual_ring_moves_and_contiguous_growth']
    if 'hint::assert_unchecked::precondition_check' in name:
        return 'ring-pop-spare',['direct:deque','kani:deque::proofs::geometry_all_words','kani:deque::proofs::actual_clone_limits_and_consuming_fold']
    if 'slice::<impl [T]>::rotate_' in name:
        return 'ring-initialized-rotation',['direct:deque','kani:deque::proofs::actual_ring_moves_and_contiguous_growth','kani:deque::proofs::actual_shrink_refusal_restores_ring_and_accounting']
    if 'core::ptr::replace::precondition_check' in name:
        return 'typed-slot-replacement',['replacement_precondition','btree_set_replace_slot','kani:collections::btree_map::proofs::set_replacement_every_initialized_slot','kani:collections::hash_map::proofs::hash_set_replace_preserves_slot_class_length_and_credit','kani:collections::binary_heap::proofs::actual_owner_pop_peek_and_drain_protocols']
    if 'fmt::builders::DebugMap' in name or (any(t in name for t in ('jzap::collections::btree_map::BTreeMap','jzap::collections::hash_map::HashMap')) and 'core::fmt::Debug' in name and '/core/src/fmt/builders.rs:' in branch['location']):
        return 'debug-map-protocol',['btree_core_map_new','btree_core_map_key_with','btree_core_map_value_with','btree_core_map_entry','btree_core_map_entries','btree_core_map_finish']
    if 'impl u32' in name and 'unchecked_shl::precondition_check' in name:return 'hash-masked-shift',['hash_core_u32_shift']
    if 'impl u64' in name and 'unchecked_shl::precondition_check' in name:return 'hash-masked-shift',['hash_core_u64_shift']
    if 'split_at_unchecked::precondition_check' in name:return 'hash-checked-chunk',['hash_core_split_chunk']
    if 'core::num' in name and 'is_multiple_of' in name:
        return 'nonzero-remainder',['btree_core_multiple_of']
    if 'impl u16' in name and 'unchecked_shl::precondition_check' in name:
        return 'masked-u16-shift',['btree_core_u16_shift','btree_actual_cursor_pack']
    if 'core::panicking::panic_const::panic_const_rem_by_zero' in name:
        return 'forwarding',[]
    if 'core::sync::atomic::' in name:
        assert any(token in name for token in ('atomic_load','atomic_store','atomic_compare_exchange','fence')),('unknown atomic guard',branch)
        return 'atomic-ordering',['shared_core_atomic_order','kani:shared::proofs::atomic_core_orderings_are_valid_for_all_counts','kani:budget::concurrent::proofs::actual_concurrent_reservation_commit_and_cancel']
    if any('<core::panicking::'+n+'>' in name for n in ('panic_nounwind','panic_nounwind_fmt','panic_cannot_unwind','panic_bounds_check')):
        return 'forwarding',[]
    if 'Layout::from_size_align' in name:return 'layout',['layout_caller','aligned_array_layout']
    if 'Alignment::new_unchecked' in name:return 'alignment',['alignment_argument_caller','layout_caller']
    if 'is_aligned_to' in name:return 'alignment',['alignment_argument_caller']
    if 'unchecked_add::precondition_check' in name:return 'offset',['pointer_add_precondition','offset_storage','cursor_successor']
    if 'NonNull<T>::new_unchecked' in name:return 'nonnull',['offset_storage','slice_caller']
    if 'write_bytes::precondition_check' in name:return 'write',['slice_caller','alignment_mask_precondition']
    if 'copy_nonoverlapping::precondition_check' in name or 'maybe_is_nonoverlapping::runtime' in name:
        return 'nonoverlap',['copy_nonoverlap_precondition','inside_nonoverlap_caller','distinct_nonoverlap_caller']
    if 'copy::precondition_check' in name:return 'copy',['overlapping_copy_caller']
    if 'IndexRange' in name and 'get_unchecked' in name:return 'index-range',['unchecked_index_range_precondition','span_caller']
    if '<usize as core::slice::index::SliceIndex' in name:return 'index',['unchecked_index_precondition','initialized_caller']
    if 'encode_utf8_raw::do_panic' in name:return 'utf8',['utf8_output_caller','utf8_byte_cursor']
    if 'core::slice::raw::from_raw_parts' in name:return 'slice',['slice_caller']
    if 'char::convert::from_u32_unchecked' in name or 'hint::unreachable_unchecked' in name:
        return 'UTF-8/UTF-16-local-scalar',['string_core_byte_cursor','string_core_negated_byte_step','kani:string::proofs::scalar_encoding_fits_four_bytes_and_is_utf8','kani:string::proofs::utf16_pair_decoder_returns_only_scalars_or_errors']
    if 'str>::is_char_boundary' in name:return 'string-boundary',['string_core_boundary']
    if 'SliceIndex<str>' in name:return 'string-index',['string_core_boundary','string_core_range','string_core_str_arithmetic']
    if 'offset_from_unsigned::precondition_check' in name:return 'byte-difference',['string_core_byte_cursor','string_core_str_arithmetic']
    if 'unchecked_neg::precondition_check' in name:return 'byte-step',['string_core_negated_byte_step']
    if 'unchecked_mul::precondition_check' in name or 'from_raw_parts_in::precondition_check' in name:
        return 'standard-byte-owner',['string_core_raw_vec']
    raise AssertionError(('unrecognized originating panic edge',branch))

# Evidence files each step leaves for the composition below. A resumed campaign
# accepts a recorded step only while these files and its log are byte-identical.
RECEIPTS={
    'budget-native':['target/verification/budget-native/summary.json'],
    'direct-foundation':['target/verification/direct-source/summary.json'],
    'direct-verus':['target/verification/direct-verus/summary.json'],
    'shared':['target/verification/shared/proofs/summary.json'],
    'string':['target/verification/string/proofs/summary.json'],
    'btree':['target/verification/btree/proofs/summary.json'],
    'btree-set':['target/verification/btree-set/proofs/summary.json'],
    'hash-map':['target/verification/hash-map/proofs/summary.json'],
    'hash-set':['target/verification/hash-set/proofs/summary.json'],
    'binary-heap':['target/verification/binary-heap/proofs/summary.json'],
    'deque':['target/verification/vec-deque/proofs/summary.json'],
    'scratch-slice':['target/verification/scratch-slice/proofs/summary.json'],
    'owner-protocol':['target/verification/owner-protocol/summary.json',
                      'target/verification/owner-protocol/owner-protocol.generated.rs'],
}
# Incremental builds are repeated in every invocation.
REPEATED=('link-fixture-build','source-routing-build')


class Campaign:
    """Run each certificate step once for one input fingerprint.

    A campaign may span several invocations of this script. Every step is still
    executed by this script, with the same command, on the same fingerprint; a
    later invocation skips a step only if its recorded log and receipts are
    unchanged. A different fingerprint is refused rather than restarted.
    """
    def __init__(self,out,fingerprint,resume,root=ROOT,link_receipts=None):
        self.out=out;self.root=root;self.path=out/'campaign.json';self.steps=[]
        self.link_receipts=link_receipts or (lambda:[])
        state=json.loads(self.path.read_text()) if resume and self.path.exists() else None
        if state is not None and state['source_sha256']!=fingerprint:
            raise AssertionError('certificate inputs differ from the recorded campaign; start a new campaign without --resume')
        if state is None:
            state=dict(source_sha256=fingerprint,started_ns=time.time_ns(),invocations=[],steps={})
        state['invocations'].append(time.time_ns());self.state=state
        self.invocation=len(state['invocations']);self.save()
    def save(self):
        temporary=self.path.with_suffix('.tmp')
        temporary.write_text(json.dumps(self.state,indent=2)+'\n');os.replace(temporary,self.path)
    def receipt_paths(self,name):
        paths=[self.root/p for p in RECEIPTS.get(name,[])]
        if name=='link':paths+=self.link_receipts()
        if name in ('bplus-release','bplus-strided-release'):paths.append(self.out/name/'summary.json')
        return paths
    def receipts(self,name):
        return {str(p):digest(p) for p in self.receipt_paths(name)}
    def run(self,name,command,extra=None):
        log_path=self.out/(name+'.log');old=self.state['steps'].get(name)
        if old is not None and name not in REPEATED:
            assert old['command']==command and old['environment']==(extra or {}),('recorded step differs',name)
            assert log_path.exists() and digest(log_path)==old['log_sha256'],('recorded log changed',name)
            current={p:(digest(Path(p)) if Path(p).exists() else None) for p in old['receipts']}
            assert current==old['receipts'],('recorded receipt changed',name)
            self.steps.append(old);print(name,old['exit_code'],'recorded in invocation',old['invocation'],flush=True)
            return old['exit_code']
        env=dict(os.environ,**(extra or {}));begin=time.time_ns()
        with log_path.open('w') as log:
            r=subprocess.run(command,cwd=self.root,env=env,stdout=log,stderr=log)
        row=dict(name=name,command=command,environment=extra or {},exit_code=r.returncode,started_ns=begin,
                 finished_ns=time.time_ns(),invocation=self.invocation)
        self.steps.append(row);print(name,r.returncode,flush=True)
        # A step ended by a signal is not evidence and is run again on resume.
        if r.returncode>=0:
            row['log_sha256']=digest(log_path)
            row['receipts']=self.receipts(name) if r.returncode==0 or (name=='link' and r.returncode==1) else {}
            self.state['steps'][name]=row;self.save()
        return r.returncode


FOUNDATION_GROUPS=(
    'arithmetic slot clone state budget '
    'vec-geometry vec-conversion layout vec-queries init-prefix '
    'buffer owner-metadata vec-cursors vec-layout-growth vec-compaction '
    'vec-compaction-values vec-extraction vec-extraction-values vec-splice-fill vec-staging '
).split()


def commands(out,profiles):
    miri=['cargo','+nightly-2026-10-30','miri','test','--locked']
    miri_env={'MIRIFLAGS':'-Zmiri-strict-provenance'}
    return {
        'link-fixture-build': (['cargo','check','--locked','--offline',
        '--manifest-path','tools/link-audit/Cargo.toml'],{}),
        'source-routing-build': (['cargo','build','--locked','--offline',
        '--manifest-path','tools/panic-audit/Cargo.toml','--target-dir','target/panic-inventory',
        '--bin','memory','--bin','unsafe_sites','--bin','unsafe_operations'],{}),
        'budget-native': (['python3','scripts/verify_budget_native.py','--mutations'],{}),
        'direct-foundation': (['python3','scripts/verify_source.py',
            *[arg for group in FOUNDATION_GROUPS for arg in ('--group',group)],'--mutations'],{}),
        'direct-verus': (['python3','scripts/verify_direct_verus.py','--mutations'],{}),
        'shared': (['python3','scripts/verify_shared.py','--mutations','--kani','--loom'],{}),
        'string': (['python3','scripts/verify_string.py','--mutations','--kani'],{}),
        'btree': (['python3','scripts/verify_btree.py','--mutations','--kani'],{}),
        'btree-set': (['python3','scripts/verify_btree_set.py','--mutations','--kani'],{}),
        'hash-map': (['python3','scripts/verify_hash_map.py','--kani','--mutations'],{}),
        'hash-set': (['python3','scripts/verify_hash_set.py','--kani','--mutations'],{}),
        'binary-heap': (['python3','scripts/verify_binary_heap.py','--kani','--mutations'],{}),
        'deque': (['python3','scripts/verify_deque.py','--kani','--mutations'],{}),
        'scratch-slice': (['python3','scripts/verify_scratch_slice.py','--kani','--mutations'],{}),
        'owner-protocol': (['python3','scripts/verify_owner_protocol.py','--mutations'],{}),
        'utf8-native': (['cargo','kani','--lib','-Z','loop-contracts','--exact','--harness',
            'boxed::text::panic_proofs::character_byte_cursor_preserves_every_valid_state'],
            {'RUSTFLAGS':'--cfg jzap_loop_contracts'}),
        'arithmetic-policy': (['python3','scripts/verify_arithmetic_policy.py'],{}),
        'link': (['python3','scripts/check_link_exits.py','--profiles',profiles,'--output',str(out/'link')],{}),
        'bplus-debug': (['python3','tools/consumer-audit/check_bplus_debug.py',
                         '--output',str(out/'bplus-debug')],{}),
        'bplus-kani': (['cargo','kani','--lib','--harness',BPLUS+'proofs::'],{}),
        'fixed-bplus-kani': (['cargo','kani','--lib','--harness',BPLUS+'fixed::proofs::'],{}),
        'bplus-range-kani': (['cargo','kani','--lib','--harness',BPLUS+'fixed::range::proofs::'],{}),
        'fixed-bplus-miri': ([*miri,'--target','x86_64-unknown-linux-gnu',
                             '--test','btree','bplus'],miri_env),
        'fixed-bplus-miri-32': ([*miri,'--target','i686-unknown-linux-gnu',
                                '--test','btree','bplus'],miri_env),
        'bplus-strided-kani': (['cargo','kani','--lib','--harness',BPLUS+'strided::proofs::'],{}),
        'bplus-strided-miri': ([*miri,'--target','x86_64-unknown-linux-gnu',
                               '--lib','bplus_strided'],miri_env),
        'bplus-strided-miri-32': ([*miri,'--target','i686-unknown-linux-gnu',
                                  '--lib','bplus_strided'],miri_env),
        'bplus-strided-release': (['python3','scripts/check_consumer_link.py',
                                  '--source','tools/consumer-audit/cases/bplus_strided.rs',
                                  '--require-success','--output',str(out/'bplus-strided-release')],{}),
        'bplus-release': (['python3','scripts/check_consumer_link.py',
                           '--source','tools/consumer-audit/cases/bplus_pool.rs',
                           '--source-dependency','tools/throughput-analysis/src/bplus_pool.rs',
                           '--require-success','--output',str(out/'bplus-release')],{}),
    }


def check_routes():
    from verify_wrapping_memory import inventory,coverage
    from verify_scratch_slice import bindings as scratch_bindings
    coverage(inventory(ROOT))
    scratch_bindings()


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,default=ROOT/'target/verification/panic-proof')
    p.add_argument('--profiles',default=PROFILES)
    p.add_argument('--resume',action='store_true',help='Continue the campaign recorded in --output for identical inputs; each step still runs exactly once')
    p.add_argument('--reuse-models',action='store_true',help='Development only; never produces a complete certificate')
    modes=p.add_mutually_exclusive_group()
    modes.add_argument('--list-stages',action='store_true',help='Print the CI matrix as JSON')
    modes.add_argument('--stage',choices=STAGES,help='Run one fresh CI stage and export its evidence')
    modes.add_argument('--assemble',type=Path,help='Check and compose all stages from this Actions attempt')
    args=p.parse_args()
    if not __debug__:
        raise RuntimeError('Verification requires Python assertions; remove -O/PYTHONOPTIMIZE')
    if args.list_stages:
        print(json.dumps({'include':[dict(stage=name,tools=spec[0]) for name,spec in STAGES.items()]}))
        return 0
    distributed=args.stage or args.assemble
    if distributed and (args.resume or args.reuse_models or args.profiles!=PROFILES):
        p.error('CI stages and assembly require a fresh run with all four link profiles')
    out=args.output.resolve()
    out.mkdir(parents=True,exist_ok=True)
    for source in (ROOT/'scripts').glob('*.py'):
        py_compile.compile(str(source),doraise=True,
            invalidation_mode=py_compile.PycInvalidationMode.CHECKED_HASH)
    lock_path=ROOT/'target/verification/.source-certificate.lock'
    lock_path.parent.mkdir(parents=True,exist_ok=True)
    lock=lock_path.open('a')
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    (out/'summary.json').unlink(missing_ok=True)
    before=inputs()

    def link_receipts():
        return [out/'link/summary.json']+sorted((out/'link').glob('*/product/retained-panic-calls.json'))

    specs=commands(out,args.profiles)
    assert set(specs)=={name for _, names in STAGES.values() for name in names}
    context=identity(ROOT,before) if distributed else None
    collected=None
    if args.assemble:
        collected, files=collect(args.assemble,context,STAGES,RECEIPTS,
                                 commands(Path('{output}'),PROFILES),PROFILES.split(','))
        started=min(row['started_ns'] for row in collected.values())
        steps=[]
        invocations=[]
    else:
        campaign=Campaign(out,before,args.resume,link_receipts=link_receipts)
        started=campaign.state['started_ns']
        steps=campaign.steps
        invocations=campaign.state['invocations']

    def run(name):
        if collected is not None:
            steps.append(collected[name])
            return collected[name]['exit_code']
        command,env=specs[name]
        return campaign.run(name,command,env)

    if args.stage:
        for name in STAGES[args.stage][1]:
            # A fresh CI checkout must produce these outputs during this job.
            stale=[path for path in campaign.receipt_paths(name) if path.exists()]
            assert not stale,(name,'stage already has evidence',stale)
            code=run(name)
            if code not in ((0,1) if name=='link' else (0,)):
                print((out/(name+'.log')).read_text()[-10000:],flush=True)
                raise AssertionError((name,code))
        if args.stage=='preflight':
            check_routes()
        assert inputs()==before,'certificate inputs changed during stage'
        publish(out/'evidence'/args.stage,args.stage,context,steps,ROOT,out)
        return 0

    def evidence(path):
        if collected is None:
            return path
        if path.is_relative_to(out):
            key='output/'+path.relative_to(out).as_posix()
        else:
            key='checkout/'+path.relative_to(ROOT).as_posix()
        return files[key]

    # Type-check all consumer roots before spending time on solver campaigns.
    assert run('link-fixture-build')==0
    # Reject a missing domain route before running expensive solver campaigns.
    assert run('source-routing-build')==0
    if collected is None:
        check_routes()
    assert run('budget-native')==0
    assert run('direct-foundation')==0
    foundation=json.loads(evidence(ROOT/'target/verification/direct-source/summary.json').read_text())
    assert foundation['status']=='pass' and foundation['source_unchanged'] and foundation['mutations_run']
    assert {row['group'] for row in foundation['groups']}==set(FOUNDATION_GROUPS)
    assert len(foundation['controls'])==55 and all(row['detected'] for row in foundation['controls'])
    assert run('direct-verus')==0
    direct_verus=json.loads(evidence(ROOT/'target/verification/direct-verus/summary.json').read_text())
    assert direct_verus['status']=='pass' and direct_verus['source_unchanged'] and direct_verus['mutations_run']
    budget_native=json.loads(evidence(ROOT/'target/verification/budget-native/summary.json').read_text())
    assert budget_native['status']=='pass' and budget_native['source_unchanged'] and budget_native['mutations_run']
    assert len(budget_native['controls'])==1 and budget_native['controls'][0]['detected']
    assert run('shared')==0
    shared=json.loads(evidence(ROOT/'target/verification/shared/proofs/summary.json').read_text())
    assert shared['status']=='pass' and shared['source_unchanged'] and shared['kani_run'] and shared['loom_run']
    assert shared['direct_numeric']['status']=='pass' and shared['direct_numeric']['source_unchanged'] and shared['direct_numeric']['mutations_run']
    assert run('string')==0
    string=json.loads(evidence(ROOT/'target/verification/string/proofs/summary.json').read_text())
    assert string['status']=='pass' and string['source_unchanged'] and string['kani_run']
    assert string['direct_cursors']['status']=='pass' and string['direct_cursors']['source_unchanged'] and string['direct_cursors']['mutations_run']
    assert run('btree')==0
    btree=json.loads(evidence(ROOT/'target/verification/btree/proofs/summary.json').read_text())
    assert btree['direct_source']['status']=='pass' and btree['direct_source']['source_unchanged'] and btree['direct_source']['mutations_run']
    assert {r['group'] for r in btree['direct_source']['groups']}=={'btree-search-cursors','btree-plan-counts','btree-frontiers','btree-delete-decisions','btree-packed-shape','btree-node-spans'}
    assert btree['status']=='pass' and btree['source_unchanged'] and btree['complete']
    assert run('btree-set')==0
    btree_set=json.loads(evidence(ROOT/'target/verification/btree-set/proofs/summary.json').read_text())
    assert btree_set['status']=='pass' and btree_set['source_unchanged'] and btree_set['complete']
    assert run('hash-map')==0
    hash_map=json.loads(evidence(ROOT/'target/verification/hash-map/proofs/summary.json').read_text())
    assert hash_map['complete'] and hash_map['source_unchanged']
    assert run('hash-set')==0
    hash_set=json.loads(evidence(ROOT/'target/verification/hash-set/proofs/summary.json').read_text())
    assert hash_set['complete'] and hash_set['source_unchanged']
    assert run('binary-heap')==0
    binary_heap=json.loads(evidence(ROOT/'target/verification/binary-heap/proofs/summary.json').read_text())
    assert binary_heap['complete'] and binary_heap['source_unchanged']
    assert run('deque')==0
    deque=json.loads(evidence(ROOT/'target/verification/vec-deque/proofs/summary.json').read_text())
    assert deque['complete'] and deque['source_unchanged']
    assert run('scratch-slice')==0
    scratch=json.loads(evidence(ROOT/'target/verification/scratch-slice/proofs/summary.json').read_text())
    assert scratch['complete'] and scratch['source_unchanged']
    if not args.reuse_models:
        assert run('owner-protocol')==0
    owner_path=evidence(ROOT/'target/verification/owner-protocol/summary.json')
    owner=json.loads(owner_path.read_text())
    assert owner['status']=='pass' and owner['source_unchanged'] and owner['source_closure_complete']
    if not args.reuse_models:
        assert not owner['reused_prerequisites']
        if collected is None:
            assert owner_path.stat().st_mtime_ns>=started
    assert owner['source_bindings']['state_writes']==87
    assert owner['source_bindings']['constructor_fields']==132
    assert owner['source_bindings']['unsafe_callers']==179
    assert len(owner['owner_expressions']['caller_bindings'])==179
    generated=evidence(ROOT/'target/verification/owner-protocol/owner-protocol.generated.rs').read_text()
    native=next(r for r in owner['results'] if r['name']=='owner-protocol')
    assert hashlib.sha256(generated.encode()).hexdigest()==native['generated_sha256'],'generated proof input differs from the verified input'
    verified=set(re.findall(r'\b(?:proof\s+)?fn\s+(\w+)\b',generated)) | set(string['verified_functions']) | set(shared['verified_functions']) | set(btree['verified_functions']) | {'kani:'+h for h in budget_native['kani_harnesses']+string['core']['kani_harnesses']+shared['kani_harnesses']+btree['kani_harnesses']}
    # These compatibility calls are checked inside the actual product crate in
    # the owner composition. Their bodies only invoke the original state methods.
    producer_root=Path(collected['owner-protocol']['checkout']) if collected is not None else ROOT
    for name in ('state-adapters.rs','budget-adapters.rs','budget-ledger.rs','vec-adapters.rs','layout-adapters.rs','init-adapters.rs'):
        path=ROOT/'tools/direct-verus'/name
        producer_path=producer_root/path.relative_to(ROOT)
        assert 'include!('+json.dumps(str(producer_path),ensure_ascii=False)+')' in generated
        verified |= set(re.findall(r'\b(?:(?:proof|spec)\s+)?fn\s+(\w+)\b',path.read_text()))
    verified |= {'direct:'+row['target'] for row in direct_verus['targets']}
    verified |= set(scratch.get('verified_functions',[]))|set(binary_heap.get('verified_functions',[]))|set(deque.get('verified_functions',[]))
    verified |= {'kani:'+h for h in btree_set['kani_harnesses']+hash_set['kani_harnesses']+binary_heap['kani_harnesses']+deque['kani_harnesses']+scratch['kani_harnesses']}
    verified |= set(hash_map['verified_functions']) | {'kani:'+h for h in hash_map['kani_harnesses']}
    assert all(r['exit_code']==0 and all(int(errors)==0 for _,errors in r['verification'])
               for r in owner['results'] if r['name']=='owner-protocol')
    if not args.reuse_models:
        # Prove the actual UTF-8 adapter for every valid cursor and Unicode char;
        # the scalar 4-byte output lemma alone would not establish its producer.
        assert run('utf8-native')==0
        assert 'Complete - 1 successfully verified harnesses, 0 failures, 1 total.' in evidence(out/'utf8-native.log').read_text()
    assert run('arithmetic-policy')==0
    assert run('bplus-debug')==0
    verified.add('pair_window_advance')
    bplus=None
    if collected is not None:
        for name in STAGES['bplus'][1][1:]:
            assert run(name)==0
        for name,count in (('bplus-kani',3),('fixed-bplus-kani',3),('bplus-range-kani',2),('bplus-strided-kani',3)):
            assert f'Complete - {count} successfully verified harnesses, 0 failures, {count} total.' in evidence(out/(name+'.log')).read_text()
        for name in ('fixed-bplus-miri','fixed-bplus-miri-32'):
            assert re.search(r'test result: ok\. 6 passed; 0 failed;',evidence(out/(name+'.log')).read_text())
        for name in ('bplus-strided-miri','bplus-strided-miri-32'):
            assert re.search(r'test result: ok\. 3 passed; 0 failed;',evidence(out/(name+'.log')).read_text())
        bplus={}
        for name in ('bplus-release','bplus-strided-release'):
            summary=json.loads(evidence(out/name/'summary.json').read_text())
            for profile in ('release-checked','release-lto-checked'):
                outcomes={r['name']:r['status'] for r in summary['results'] if r['profile']==profile}
                assert outcomes=={'positive':'pass','consumer':'pass','overflow-control':'control-detected',
                                  'bounds-control':'control-detected','abort-control':'control-detected'}
            assert len(summary['results'])==10
            bplus[name]=summary
    link_code=run('link')
    assert link_code in (0,1)
    link=json.loads(evidence(out/'link/summary.json').read_text());branches=[];forward=[]
    if collected is not None:
        assert link['source_commit']==context['commit'],'link summary was produced for a different commit'
        assert link_code==int(any(r['status']=='retained-exit-edge' for r in link['results']))
    expected={(profile,None) for profile in args.profiles.split(',')}
    expected|={(profile,c) for profile in args.profiles.split(',') if profile in ('release','release-lto')
               for c in ('panic-control','bounds-control','oom-control','abort-control')}
    observed=[(r['profile'],r['control']) for r in link['results']]
    assert set(observed)==expected and len(observed)==len(expected),'incomplete link matrix'
    for r in link['results']:
        if r['control']:
            assert r['status']=='control-detected',r
            continue
        assert r['status'] in ('pass','retained-exit-edge'),r
        if r['status']=='pass':continue
        assert r.get('diagnostic_link_exit')==0,r
        path=evidence(out/'link'/r['profile']/'product/retained-panic-calls.json')
        assert path.exists()
        edges=json.loads(path.read_text())
        if collected is not None:
            assert edges and len(edges)==r['retained_panic_call_sites'],r
            assert r['diagnostic_seeds']==65 and 'diagnostic_runtime_error' not in r,r
        for edge in edges:
            family,contracts=classify(edge)
            assert all(name in verified for name in contracts),(edge,contracts)
            item=dict(edge,profile=r['profile'],family=family,contracts=contracts,
                      source_queries=[b['function'] for b in owner['owner_expressions']['caller_bindings']]+sorted({b['site']['function'] for b in btree['bindings']+hash_map['bindings']+binary_heap['bindings']+deque['bindings']+scratch['bindings'] if b['site']['function']}),
                      disposition='guard-unreachable-under-verified-source-contracts')
            if family=='forwarding':forward.append(item)
            else:branches.append(item)
    # Forwarders are closed only after every originating guard is bound. The
    # callback contract excludes user panic/abort, rather than proving their code.
    for row in forward:row['disposition']='no-unproved-library-originating-edge'
    assert inputs()==before,'certificate inputs changed during verification'
    source_obligations=[dict(site=b['site'],contract=b['contract'],role=b['role'],
        disposition='solver-and-source-connected' if b['contract'] in verified or b['contract'].startswith('kani:') else 'unproved')
        for b in owner['source_bindings']['bindings']]
    source_obligations += [dict(site=b['site'],contract=b['contract'],role='String-typed-memory-boundary',disposition='solver-and-source-connected') for b in string['bindings']]
    assert all(b['contract'] in verified for b in shared['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in shared['bindings']]
    assert all(b['contract'] in verified for b in btree['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in btree['bindings']]
    assert all(b['contract'] in verified for b in hash_map['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in hash_map['bindings']]
    assert all(b['contract'] in verified for b in binary_heap['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in binary_heap['bindings']]
    assert all(b['contract'] in verified for b in deque['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in deque['bindings']]
    assert all(b['contract'] in verified for b in scratch['bindings'])
    source_obligations += [dict(site=b['site'],contract=b['contract'],role=b['role'],disposition='solver-and-source-connected') for b in scratch['bindings']]
    source_obligations.append(dict(
        site=dict(file='src/collections/btree_map/view/bplus.rs',function='descend',
                  expression='ancestor.assume_init_ref()'),
        contract='lookup_path_read',role='initialized-ancestor-prefix',
        disposition='solver-and-source-connected'))
    open_source=sum(b['disposition']=='unproved' for b in source_obligations)
    unproved_edges=sum(b['disposition'] not in ('guard-unreachable-under-verified-source-contracts','no-unproved-library-originating-edge') for b in branches+forward)
    full_profiles=set(args.profiles.split(','))=={'release','release-lto','release-checked','debug'}
    complete=not args.reuse_models and full_profiles and not open_source and not unproved_edges and owner['source_closure_complete'] and all(
        row['disposition']=='guard-unreachable-under-verified-source-contracts' for row in branches)
    report=dict(complete=complete,status='pass' if complete else 'development-only',
        source_sha256=before,source_unchanged=True,models=owner,budget_native=budget_native,direct_foundation=foundation,direct_verus=direct_verus,string=string,shared=shared,btree=btree,btree_set=btree_set,hash_map=hash_map,hash_set=hash_set,binary_heap=binary_heap,deque=deque,scratch=scratch,link=link,
        branches=branches+forward,unproved_link_edges=unproved_edges,open_source_obligations=open_source,
        steps=steps,reused_models=args.reuse_models,bplus=bplus,
        campaign=dict(started_ns=started,invocations=invocations,actions=context),
        scope='Numeric overflow-to-memory and internal guard unreachability for the checked Box/Vec/String/Rc/Arc/BTreeMap/BTreeSet/HashMap/HashSet/BinaryHeap/VecDeque/Scratch/checked-slice and concurrent-budget source contracts and Linux x86_64 link fixtures. Direct Rust/Verus kernels and reviewed source projections remain distinct evidence. Rust/compiler/core ABI, valid allocator, safe ownership, public unsafe and returning callback contracts remain explicit; actual alias/initialization/drop scenarios are checked by the separate Miri/ASan/compiler suites.')
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
    (out/'source-obligations.json').write_text(json.dumps(source_obligations,indent=2)+'\n')
    print(f'{len(branches)} originating guard calls and {len(forward)} forwarding calls attached; complete={complete}',flush=True)
    return int(not complete)


if __name__=='__main__':raise SystemExit(main())
