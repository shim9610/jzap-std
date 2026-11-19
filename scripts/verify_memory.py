#!/usr/bin/env python3
"""Run extra Kani checks per real harness, with bounded compiler time and evidence."""
import concurrent.futures, hashlib, json, os, pathlib, re, signal, subprocess
root = pathlib.Path(__file__).resolve().parents[1]
out = root/'target/verification/memory-proof'
out.mkdir(parents=True, exist_ok=True)
target = root/'target/memory-proof'
target.mkdir(parents=True, exist_ok=True)
source = (root/'src/proofs.rs').read_text()
# Absolute #[path] modules use the containing directory for nested modules.
# Preserve original child source paths explicitly in the generated Box module.
boxed = (root/'src/boxed.rs').read_text()
boxed = re.sub(r'^(?:pub\(crate\) )?mod (\w+);', lambda m: '#[path = "'+str(root/'src/boxed'/(m.group(1)+'.rs'))+'"]\n'+m.group(0), boxed, flags=re.M)
(target/'boxed.rs').write_text(boxed)
vector = (root/'src/vec.rs').read_text()
vector = re.sub(r'^(?:pub\(crate\) )?mod (\w+);', lambda m: '#[path = "'+str(root/'src/vec'/(m.group(1)+'.rs'))+'"]\n'+m.group(0), vector, flags=re.M)
(target/'vec.rs').write_text(vector)
header = '#![no_std]\n#![allow(dead_code)]\nextern crate alloc;\n'
for module in ['budget','clone','default','error','init','layout','storage','boxed','vec']:
    location = target/(module+'.rs') if module in ['boxed','vec'] else root/'src'/(module+'.rs')
    header += f'#[path = "{location}"] mod {module};\n'
header += 'pub use clone::{TryClone, TryCloneIn};\npub use default::TryDefault;\npub use error::{Error, Rejected};\n'
jobs=[]
for match in re.finditer(r'#\[kani::proof\]\s*(?:#\[kani::unwind\(\d+\)\]\s*)?fn (\w+)',source):
    start=source.index('{',match.end());end=start+1;depth=1
    while depth:
        depth+=(source[end]=='{')-(source[end]=='}');end+=1
    name=match.group(1)
    p=target/(name+'.rs');p.write_text(header+source[match.start():end])
    jobs.append((name,p))

def run(job):
    name,p=job
    command=['kani',str(p),'-Z','uninit-checks','-Z','valid-value-checks']
    env=dict(os.environ,RUSTFLAGS='--edition=2024 --cfg=feature="alloc"')
    with (out/(name+'-extra.log')).open('w') as log:
        process=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
        try: code=process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGTERM);process.wait();code=124
    return dict(harness=name,exit_code=code,command=command,compiler_timeout_seconds=60,
                source_sha256=hashlib.sha256(p.read_bytes()).hexdigest())
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool: results=list(pool.map(run,jobs))
(out/'extra-status.json').write_text(json.dumps(results,indent=2))
for r in results: print(r['harness'],r['exit_code'])
raise SystemExit(int(any(r['exit_code'] for r in results)))
