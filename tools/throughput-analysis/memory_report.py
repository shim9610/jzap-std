#!/usr/bin/env python3
"""Summarize equal-work allocator observations, without retaining raw rows in Git."""
import argparse,csv,json,subprocess
from pathlib import Path
FIELDS=['calls','held_bytes','peak_bytes','held_actual','peak_actual']
def summarize(rows):
    keys=[(r['task'],r['label'],r['case']) for r in rows]
    assert len(keys)==len(set(keys)), 'duplicate allocation case'
    baseline={(r['task'],r['case']):r for r in rows if r['label']=='std'}
    groups={}
    for row in rows:
        if row['label']!='std':groups.setdefault((row['task'],row['label']),[]).append(row)
    result=[]
    for (task,label),group in sorted(groups.items()):
        expected={case for name,case in baseline if name==task}
        assert {r['case'] for r in group}==expected, 'missing allocation case'
        assert all(r['input_bytes']==baseline[task,r['case']]['input_bytes'] for r in group)
        handles={r['handle_bytes'] for r in group};assert len(handles)==1
        out=dict(task=task,label=label,cases=len(group),handle_bytes=handles.pop())
        for field in FIELDS:
            total=sum(r[field] for r in group);base=sum(baseline[task,r['case']][field] for r in group)
            ratios=[r[field]/baseline[task,r['case']][field] for r in group if baseline[task,r['case']][field]]
            assert not any(r[field] and not baseline[task,r['case']][field] for r in group),'new allocation on zero-allocation case'
            out[field+'_mean']=total/len(group)
            out['std_'+field+'_mean']=base/len(group)
            out[field+'_ratio']=total/base if base else 1.0
            out[field+'_max_case_ratio']=max(ratios,default=1.0)
        result.append(out)
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    source=parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--input',type=Path)
    source.add_argument('--heap-binary','--collection-binary',type=Path,help='Observe each BinaryHeap label/seed/case/width in a fresh process')
    parser.add_argument('--records',type=Path,help='Optional raw child observations; use an ignored target path')
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--format',choices=['linear','btree','btree-set','hash-map','hash-set','binary-heap','deque'],default='linear')
    parser.add_argument('--max-ratio',type=float,help='Fail if any held/peak allocation case exceeds this std ratio')
    args=parser.parse_args()
    if args.heap_binary:
        if args.format not in ('binary-heap','deque'):parser.error('--collection-binary requires binary-heap or deque format')
        prefix='deque_' if args.format=='deque' else 'binary_heap_'
        binary=str(args.heap_binary.resolve());tasks=set(subprocess.check_output([binary,'tasks'],text=True).strip().split(','));raw=[]
        for seed in [617,6203,96011]:
            for width in [8,256]:
                expected={t for t in tasks if t.endswith('_'+str(width))}
                for case in range(64):
                    for label in ['std','unlimited','local','concurrent']:
                        rows=[json.loads(line)for line in subprocess.check_output([binary,'memory',label,str(seed),str(case),str(width)],text=True).splitlines()]
                        assert {prefix+r['operation']for r in rows}==expected and len(rows)==len(expected),'missing/duplicate isolated operation'
                        assert all((r['seed'],r['case'],r['label'],r['width'])==(seed,case,label,width)for r in rows),'isolated input mismatch'
                        raw.extend(rows)
                print(f'Isolated seed={seed} width={width}: complete',flush=True)
    else:raw=[json.loads(line) for line in args.input.read_text().splitlines()]
    if args.records:
        args.records.parent.mkdir(parents=True,exist_ok=True)
        args.records.write_text(''.join(json.dumps(r,separators=(',',':'))+'\n'for r in raw))
    if args.format in ('btree','btree-set','hash-map','hash-set','binary-heap','deque'):
        prefix,width=('btree_',16) if args.format=='btree' else ('hash_map_',16) if args.format=='hash-map' else ('hash_set_',8) if args.format=='hash-set' else ('binary_heap_',8) if args.format=='binary-heap' else ('deque_',8) if args.format=='deque' else ('btree_set_',8)
        raw=[dict(task=prefix+r['operation'],label=r['label'],case=str(r['seed'])+':'+str(r['case']),
                  input_bytes=r['size']*r.get('width',width),handle_bytes=r['handle'],calls=r['calls'],held_bytes=r['held'],peak_bytes=r['peak'],
                  held_actual=r['actual_held'],peak_actual=r['actual_peak']) for r in raw]
    rows=summarize(raw)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    with args.output.open('w',newline='') as stream:
        writer=csv.DictWriter(stream,fieldnames=list(rows[0]),lineterminator="\n");writer.writeheader();writer.writerows(rows)
    worst=max(r[k+'_max_case_ratio'] for r in rows for k in FIELDS if k!='calls')
    print(f'{len(raw)} allocation cases -> {len(rows)} aggregate rows; worst allocated-memory ratio={worst:.4f}')
    if args.max_ratio is not None:assert worst<=args.max_ratio,('allocation acceptance limit',worst,args.max_ratio)
if __name__=='__main__':main()
