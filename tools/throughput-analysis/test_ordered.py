#!/usr/bin/env python3
"""Preflight ordered workloads against std; this is not a timing report."""
import argparse,json,subprocess
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',required=True);a=p.parse_args()
    tasks=subprocess.check_output([a.binary,'tasks'],text=True).strip().split(',');assert len(set(tasks))==len(tasks)
    processes=0
    for seed in [617,6203]:
        for profile in ['mixed','small']:
            for task in tasks:
                rows=[json.loads(subprocess.check_output([a.binary,label,task,str(seed),'2',profile],text=True)) for label in ['std','unlimited','local','concurrent']]
                keys=['epochs','operations','elements','corpus_digest','checksum','cases','min_size','max_size']
                assert len({tuple(r[k] for k in keys)for r in rows})==1,(seed,profile,task,rows)
                processes+=len(rows)
    print(processes,'ordered workload processes match std inputs/work/checksums.')
if __name__=='__main__':main()
