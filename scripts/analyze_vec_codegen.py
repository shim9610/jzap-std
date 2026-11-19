#!/usr/bin/env python3
"""Inspect the actual Vec benchmark's monomorphized loops; keep dumps local."""
import hashlib,json,pathlib,re,subprocess
root=pathlib.Path(__file__).resolve().parents[1];out=root/'target/verification/vec/codegen';out.mkdir(parents=True,exist_ok=True)
command=['cargo','rustc','--locked','--manifest-path','tools/vec-analysis/Cargo.toml','--release','--bin','jzap-vec-analysis','--','--emit=asm']
with (out/'build.log').open('w') as f:subprocess.run(command,cwd=root,stdout=f,stderr=subprocess.STDOUT,check=True)
asm=max((root/'tools/vec-analysis/target/release/deps').glob('jzap_vec_analysis-*.s'),key=lambda p:p.stat().st_mtime)
text=asm.read_text();rows=[]
for name,body in re.findall(r'^([^\s.][^\n]*):\n(.*?)^\.Lfunc_end\d+:',text,re.M|re.S):
 if 'jzap_vec_analysis3run' not in name:continue
 calls=re.findall(r'^\s*(?:callq|jmpq)\s+(.*)',body,re.M)
 assert not any('Allocation' in c and '4drop' in c for c in calls), name
 assert not any('Owner' in c and ('clone_owner' in c or '4copy' in c) for c in calls),name
 rows.append(dict(symbol=name,instructions=len(re.findall(r'^\t[a-z]',body,re.M)),calls=calls))
assert len(rows)>=40,len(rows) # LLVM may merge equivalent instantiations.
(out/'summary.json').write_text(json.dumps(dict(command=command,assembly_sha256=hashlib.sha256(text.encode()).hexdigest(),functions=rows),indent=2))
print(f'{len(rows)} operation-specialized benchmark loops checked; no out-of-line allocation-guard destructor.')
