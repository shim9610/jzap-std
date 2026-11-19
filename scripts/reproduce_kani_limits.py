#!/usr/bin/env python3
"""Characterize compiler limitations separately from product verification."""
import json, pathlib, subprocess
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'target/verification/memory-proof'
out.mkdir(parents=True,exist_ok=True)
cases=[
 ('unwrap-validity','unwrap_repro',[],['valid-value-checks'],1,'expected pattern type to have a scalar ABI'),
 ('unwrap-uninit','unwrap_repro',[],['uninit-checks'],1,'function pointers and vtable calls are not supported'),
 ('no-std-equality','assert_eq_no_std_repro',[],['valid-value-checks'],1,'expected pattern type to have a scalar ABI'),
 ('plain','assert_repro',[],['uninit-checks','valid-value-checks'],0,'VERIFICATION:- SUCCESSFUL'),
 ('raw-write','reference_write_repro',['--harness','raw_reference_write'],['valid-value-checks'],1,'Undefined Behavior: Invalid value of type'),
 ('spare-slice-validity','reference_write_repro',['--harness','initialized_spare_slice'],['valid-value-checks'],1,'expected pattern type to have a scalar ABI'),
 ('safe-write','reference_write_repro',['--harness','initialized_reference_write'],['valid-value-checks'],0,'VERIFICATION:- SUCCESSFUL'),
 ('invalid-bool','invalid_bool_repro',[],['valid-value-checks'],1,'Undefined Behavior: Invalid value of type `bool`'),
 ('uninit-read','uninitialized_read_repro',[],['uninit-checks'],1,'Reading from an uninitialized pointer'),
]
results=[]
for name,file,args,flags,expected,diagnostic in cases:
    command=['kani',str(root/'tools/memory-proof'/f'{file}.rs')]+args
    for flag in flags: command+=['-Z',flag]
    r=subprocess.run(command,capture_output=True,text=True,timeout=30)
    text=r.stdout+r.stderr
    (out/f'repro-{name}.log').write_text(text)
    assert r.returncode==expected and diagnostic in text,(name,r.returncode,text)
    results.append(dict(case=name,command=command,exit_code=r.returncode,expected=expected))
(out/'reproduction-status.json').write_text(json.dumps(results,indent=2))
print(f'{len(cases)} expected outcomes reproduced; these are diagnostic controls, not product proofs')
