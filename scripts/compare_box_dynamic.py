#!/usr/bin/env python3
"""Dynamic Box acceptance: std controls, time and separately instrumented memory."""
import argparse,hashlib,json,os,pathlib,random,statistics,subprocess
root=pathlib.Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--trials',type=int,default=15);p.add_argument('--output',type=pathlib.Path,default=root/'target/verification/box-completion/performance');p.add_argument('--extend',type=pathlib.Path);args=p.parse_args()
out=args.output;out.mkdir(parents=True,exist_ok=True)
exe=root/'tools/box-analysis/target/release/dynamic';meter=root/'tools/box-analysis/target/memory/release/dynamic'
cpu=min(os.sched_getaffinity(0));os.sched_setaffinity(0,{cpu})
previous=json.loads((args.extend/'timings.json').read_text()) if args.extend else []
base_trials=max((r['trial'] for r in previous),default=-1)+1
if args.extend:
 prior=json.loads((args.extend/'metadata.json').read_text())
 assert prior['binary_sha256']==hashlib.sha256(exe.read_bytes()).hexdigest()
 assert prior['separate_replacement_process']
(out/'metadata.json').write_text(json.dumps(dict(cpu=cpu,processes=base_trials+args.trials,new_processes=args.trials,extended_from=str(args.extend) if args.extend else None,warmup=2,retained=30,separate_replacement_process=True,acceptance_run=base_trials+args.trials>=15,binary_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),memory_binary_sha256=hashlib.sha256(meter.read_bytes()).hexdigest()),indent=2))
rows=previous
for kind in ['slice','str','zeroed','collect']:
 for length in [32,4096,1048576]:
  for trial in range(base_trials,base_trials+args.trials):
   order=['std','std_control','unmetered','metered'];shift=trial%4;order=order[shift:]+order[:shift]
   if trial%2:order.reverse()
   for label in order:
    command=[str(exe),'std' if label=='std_control' else label,kind,str(length)]
    row=json.loads(subprocess.check_output(command,text=True))
    replacement=json.loads(subprocess.check_output(command+['replace'],text=True))
    row['baseline_samples_ns']=[list(sample) for sample in row['samples_ns']]
    row['replacement_samples_ns']=replacement['samples_ns']
    for sample,replaced in zip(row['samples_ns'],replacement['samples_ns']):sample[2]=replaced[2]
    row.update(label=label,trial=trial);rows.append(row)
   (out/'timings.json').write_text(json.dumps(rows,indent=2))
  print('completed',kind,length,flush=True)
rng=random.Random(617);summary=[]
for kind in ['slice','str','zeroed','collect']:
 for length in [32,4096,1048576]:
  for label in ['std_control','unmetered','metered']:
   for index,metric in enumerate(['create','clone','clone_from','drop']):
    def values(l):return [statistics.median(sample[index] for sample in r['samples_ns']) for r in rows if r['label']==l and r['kind']==kind and r['len']==length]
    a,b=values(label),values('std');ratios=sorted(statistics.median(rng.choices(a,k=len(a)))/statistics.median(rng.choices(b,k=len(b))) for _ in range(5000));low,high=ratios[125],ratios[4874]
    summary.append(dict(kind=kind,len=length,label=label,metric=metric,ratio=statistics.median(a)/statistics.median(b),low95=low,high95=high,status='pass' if high<=1.15 else ('fail' if low>1.15 else 'inconclusive')))
for r in summary:
 control=next(x for x in summary if x['kind']==r['kind'] and x['len']==r['len'] and x['metric']==r['metric'] and x['label']=='std_control');r['control_valid']=control['status']=='pass'
(out/'summary.json').write_text(json.dumps(summary,indent=2))
mem=[];mem_summary=[]
for kind in ['slice','str','zeroed','collect']:
 for length in [32,4096,1048576]:
  for mode in ['requested','usable']:
   for label in ['std','unmetered','metered']:
    row=json.loads(subprocess.check_output([str(meter),label,kind,str(length),mode],text=True));mem.append(row)
    base=next(r for r in mem if r['kind']==kind and r['len']==length and r['mode']==mode and r['label']=='std')
    # stats: build live, clone live, peak, end live, total, calls, handle bytes.
    assert row['stats'][3]==0,row
    assert row['stats'][5]<=base['stats'][5],(base,row)
    for i,name in [(0,'build_live'),(1,'clone_live'),(2,'peak'),(4,'total'),(6,'handle')]:
     extra=(8 if name=='handle' else 8*row['n']*(1 if name=='build_live' else 2)) if label=='metered' else 0
     actual,standard=row['stats'][i],base['stats'][i];adjusted=(actual-extra)/standard
     passed=adjusted<=1 if mode=='requested' or name=='handle' else adjusted<=1.15
     mem_summary.append(dict(kind=kind,len=length,mode=mode,label=label,metric=name,raw_ratio=actual/standard,accepted_fixed_bytes=extra,adjusted_ratio=adjusted,status='pass' if passed else 'fail'))
(out/'memory.json').write_text(json.dumps(mem,indent=2));(out/'memory-summary.json').write_text(json.dumps(mem_summary,indent=2))
failed=[r for r in summary if r['status']!='pass' or not r['control_valid']]
for r in failed:print(r)
print('time failures/inconclusive',len(failed),'of',len(summary),'memory failures',sum(r['status']!='pass' for r in mem_summary))
raise SystemExit(int(bool(failed) or any(r['status']!='pass' for r in mem_summary)))
