#!/usr/bin/env python3
"""Check the library arithmetic gate and reject the original bypass patterns."""
import hashlib,json,os,subprocess,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/arithmetic-policy'
OUT.mkdir(parents=True,exist_ok=True)
config=(ROOT/'clippy.toml').read_bytes()
assert tomllib.loads(config.decode())=={'arithmetic-side-effects-allowed-binary':[
    ['arithmetic::checked::Checked','arithmetic::checked::Checked'],
    ['capacity::Capacity','arithmetic::checked::Checked']]}
assert '#![deny(clippy::arithmetic_side_effects)]' in (ROOT/'src/lib.rs').read_text()
project=OUT/'probe';project.mkdir(exist_ok=True)
(project/'Cargo.toml').write_text('[package]\nname="jzap-arithmetic-policy-probe"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[lib]\npath="probe.rs"\n[dependencies]\njzap-std={path='+json.dumps(str(ROOT))+',default-features=false}\n')
env=dict(os.environ,CLIPPY_CONF_DIR=str(ROOT));rows=[]
def run(name,command,expected_failure=False):
 r=subprocess.run(command,cwd=ROOT,env=env,capture_output=True,text=True);log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log)
 passed=(r.returncode!=0 and 'arithmetic_side_effects' in log and 'error[E' not in log) if expected_failure else r.returncode==0
 rows.append(dict(name=name,command=command,exit_code=r.returncode,expected_failure=expected_failure,passed=passed))
 assert passed,log
run('product',['cargo','clippy','--locked','--offline','--lib','--all-features','--','-D','warnings'])
checks=[
 ('explicit', 'use jzap::{TryArithmetic,WrappingArithmetic}; pub fn checked(a:usize,b:usize)->Result<usize,jzap::ArithmeticError>{a.try_add(b)} pub fn wrapping(a:usize,b:usize)->usize{a.wrap_add(b)}',False),
 ('raw_add','pub fn f(a:usize,b:usize)->usize{a+b}',True),
 ('wrapped_raw_add','pub fn f(a:usize,b:usize)->jzap::Checked<usize>{jzap::Checked::new(a+b)}',True),
 ('extracted_raw_add','pub fn f(a:jzap::Checked<usize>,b:usize)->usize{a.get()+b}',True),
 ('raw_mul','pub fn f(a:usize,b:usize)->usize{a*b}',True),
]
for name,code,negative in checks:
 (project/'probe.rs').write_text('#![no_std]\n'+code+'\n')
 run(name,['cargo','clippy','--offline','--manifest-path',str(project/'Cargo.toml'),'--lib','--','-D','clippy::arithmetic_side_effects'],negative)
(OUT/'summary.json').write_text(json.dumps(dict(config_sha256=hashlib.sha256(config).hexdigest(),checks=rows),indent=2)+'\n')
print('Product policy and explicit APIs passed; four native-arithmetic bypasses rejected.')
