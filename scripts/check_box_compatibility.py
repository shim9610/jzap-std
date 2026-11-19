#!/usr/bin/env python3
"""Prove compiler boundaries separately from incomplete product features."""
import hashlib,json,pathlib,subprocess
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'target/verification/box-compatibility';out.mkdir(exist_ok=True)
build=root/'target/box-compatibility';build.mkdir(parents=True,exist_ok=True)
rows=[]
for toolchain in ['1.95.0','nightly-2026-10-30']:
    version=subprocess.check_output(['rustc','+'+toolchain,'-Vv'],text=True)
    for name in ['std_control','move_out','coerce','fn_once','coerce_impl','fn_once_impl','explicit_control']:
        src=root/'tools/box-compatibility'/f'{name}.rs'
        command=['rustc','+'+toolchain,'--edition=2024',str(src),'-o',str(build/name)]
        if toolchain.startswith('nightly') and name.endswith('_impl'):command+=['--cfg','box_nightly']
        result=subprocess.run(command,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (out/f'{toolchain}-{name}.log').write_text(result.stdout)
        expected=(name in ['std_control','explicit_control'] or (toolchain.startswith('nightly') and name.endswith('_impl')))
        assert (result.returncode==0)==expected,(command,result.stdout)
        runtime=None
        if expected:
            executed=subprocess.run([str(build/name)],text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
            assert executed.returncode==0,executed.stdout
            runtime=executed.returncode
        else:
            code={'move_out':'E0507','coerce':'E0308','fn_once':'E0277','coerce_impl':'E0658','fn_once_impl':'E0658'}[name]
            assert code in result.stdout,result.stdout
        rows.append(dict(toolchain=toolchain,version=version,case=name,command=command,compile_exit=result.returncode,run_exit=runtime,source_sha256=hashlib.sha256(src.read_bytes()).hexdigest()))
(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('14 expected compiler outcomes confirmed; std and explicit stable controls ran successfully.')
