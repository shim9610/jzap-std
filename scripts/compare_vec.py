#!/usr/bin/env python3
"""Direct Vec/std comparison: isolated timing processes and separate memory hooks."""
import argparse,collections,hashlib,json,os,pathlib,random,statistics,subprocess
root=pathlib.Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--extend',type=pathlib.Path);p.add_argument('--memory-only',action='store_true');p.add_argument('--trials',type=int,default=15);p.add_argument('--output',type=pathlib.Path,default=root/'target/verification/vec/performance');p.add_argument('--sizes',default='32,4096,131072');p.add_argument('--ops',default='grow,reserved,copy,clone');p.add_argument('--memory-ops',default='grow');p.add_argument('--memory-sizes',default='0,1,3,8,17,32,65,4096,131072');p.add_argument('--timing-binary',type=pathlib.Path);p.add_argument('--memory-binary',type=pathlib.Path);args=p.parse_args()
out=args.output;out.mkdir(parents=True,exist_ok=True)
exe=(args.timing_binary or root/'tools/vec-analysis/target/release/jzap-vec-analysis').resolve();meter=(args.memory_binary or root/'tools/vec-analysis/target/memory/release/jzap-vec-analysis').resolve()
# Proof modules are compiled only by Kani, never by this timing executable.
source_files=sorted(p for p in (root/'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))+sorted((root/'tools/vec-analysis/src').glob('*.rs'))
assert not (args.memory_only and args.extend), "Memory-only runs cannot extend timing evidence"
for binary in ([meter] if args.memory_only else [exe,meter]):
 assert binary.exists(),f'Build {binary} before measuring'
 stale=[str(p.relative_to(root)) for p in source_files if p.stat().st_mtime>binary.stat().st_mtime]
 assert not stale,f'Rebuild {binary}: newer sources {stale}'
source_digest=hashlib.sha256(b''.join(str(p.relative_to(root)).encode()+b'\0'+p.read_bytes() for p in source_files)).hexdigest()
cpu=min(os.sched_getaffinity(0));os.sched_setaffinity(0,{cpu})
labels=['std','std_control','unlimited','limited','metered','limited_metered'];sizes=list(map(int,args.sizes.split(',')));ops=[] if args.memory_only else args.ops.split(',');rows=[]
base_trials=0
if args.extend:
 previous=json.loads((args.extend/'metadata.json').read_text())
 assert previous['binary_sha256']==hashlib.sha256(exe.read_bytes()).hexdigest()
 assert previous['sizes']==sizes and previous['operations']==ops and previous['cpu']==cpu
 rows=json.loads((args.extend/'raw.json').read_text());base_trials=previous['processes']

metadata=dict(timing_binary=str(exe),memory_binary=str(meter),source_sha256=source_digest,cpu=cpu,processes=0 if args.memory_only else base_trials+args.trials,extended_from=str(args.extend) if args.extend else None,warmup=2,retained=30,binary_sha256=None if args.memory_only else hashlib.sha256(exe.read_bytes()).hexdigest(),memory_binary_sha256=hashlib.sha256(meter.read_bytes()).hexdigest(),memory_operations=args.memory_ops.split(','),memory_sizes=list(map(int,args.memory_sizes.split(','))),sizes=sizes,operations=ops,acceptance=not args.memory_only and base_trials+args.trials>=15,memory_only=args.memory_only,platform=dict(zip(['sysname','nodename','release','version','machine'],os.uname())))
(out/'metadata.json').write_text(json.dumps(metadata,indent=2))
for n in sizes:
 for op in ops:
  for trial in range(base_trials,base_trials+args.trials):
   shift=trial%len(labels);order=labels[shift:]+labels[:shift]
   if trial%2:order=order[::-1]
   for label in order:
    row=json.loads(subprocess.check_output([str(exe),'std' if label=='std_control' else label,op,str(n)],text=True));row.update(label=label,trial=trial);rows.append(row)
  (out/'raw.json').write_text(json.dumps(rows,indent=2));print(n,op,'done',flush=True)
rng=random.Random(617);summary=[]
for n in sizes:
 for op in ops:
  def values(label):return [statistics.median(r['samples_ns'])/r['repetitions'] for r in rows if r['label']==label and r['n']==n and r['op']==op]
  base=values('std')
  for label in labels[1:]:
   a=values(label);ratios=sorted(statistics.median(rng.choices(a,k=len(a)))/statistics.median(rng.choices(base,k=len(base))) for _ in range(5000));low,high=ratios[125],ratios[4874]
   summary.append(dict(n=n,op=op,label=label,std_ns=statistics.median(base),ns=statistics.median(a),ratio=statistics.median(a)/statistics.median(base),low95=low,high95=high,status='pass' if high<=1.15 else 'fail' if low>1.15 else 'inconclusive'))
for r in summary:r['control_valid']=next(x for x in summary if x['n']==r['n'] and x['op']==r['op'] and x['label']=='std_control')['status']=='pass'
(out/'summary.json').write_text(json.dumps(summary,indent=2));print(collections.Counter(x['status'] for x in summary),flush=True)
mem=[]
for n in map(int,args.memory_sizes.split(',')):
 for label in labels:
  if label=='std_control':continue
  for operation in args.memory_ops.split(','):
   for mode in ['memory','memory-moving']:
    row=json.loads(subprocess.check_output([str(meter),label,mode+'-'+operation,str(n)],text=True));row['allocator_mode']=mode;row['operation']=operation;mem.append(row)
(out/'memory.json').write_text(json.dumps(mem,indent=2))
memory_checks=[]
for row in mem:
 base=next(r for r in mem if r['n']==row['n'] and r['label']=='std' and r['allocator_mode']==row['allocator_mode'] and r['operation']==row['operation'])
 checks={'final_requested_zero':row['end'][0]==0,'final_usable_zero':row['end'][2]==0,
         'allocation_calls':row['end'][5]<=base['end'][5],
         'requested_total':row['end'][4]<=base['end'][4]}
 for phase in ['build','cloned']:
  for index,metric in [(0,'requested_live'),(1,'requested_peak'),(2,'usable_live'),(3,'usable_peak')]:
   factor=1.0 if index<2 else 1.15
   checks[phase+'_'+metric]=row[phase][index]<=base[phase][index]*factor
 expected_words={'std':3,'unlimited':3,'limited':4,'metered':4,'limited_metered':5}
 checks['handle_layout']=row['handle']==base['handle']//3*expected_words[row['label']]
 memory_checks.append(dict(operation=row['operation'],label=row['label'],n=row['n'],allocator_mode=row['allocator_mode'],checks=checks,handle_bytes=row['handle'],additional_handle_bytes=row['handle']-base['handle']))
(out/'memory-summary.json').write_text(json.dumps(memory_checks,indent=2))
assert all(all(r['checks'].values()) for r in memory_checks), memory_checks

for r in summary:
 if r['status']!='pass' or not r['control_valid']:print(r,flush=True)
raise SystemExit(int(any(r['status']!='pass' or not r['control_valid'] for r in summary)))
