"""Shared checks for maintained callers composed with the product crate.

A maintained caller is a Verus function that calls product kernels with the
arguments of one product body. This module checks that (a) each caller still
contains its product calls, (b) each followed product body has the recorded
text, and runs the two kinds of controls: weakened callers that Verus must
reject and compiled product changes that the text comparison must refuse.
The text comparison is a review trigger, not a solver proof.
"""
import re,shutil,subprocess,tempfile
from pathlib import Path
from verify_edit_loops import block
from verus_composition import run as compose
compact=lambda s:re.sub(r'\s+','',re.sub(r'//[^\n]*','',s))
def maintained(text,name):
    start=text.index('\nfn '+name+'(');return compact(text[start:text.index('\n}\n',start)+3])
def correspondence(root,model,bodies,callers,sites,label,restated=(),shared=''):
    # `shared` is maintained text in front of the model (the storage model),
    # searched for callers the area binds but does not define itself.
    root=Path(root);text=shared+Path(model).read_text()
    for name,calls in callers.items():
        body=maintained(text,name)
        for call in calls:assert call in body,('maintained '+label+' caller lost a product call',name,call)
    for (file,anchor,head),expected in bodies.items():
        source=(root/'src'/file).read_text();assert source.count(anchor or head)==1,(label+' caller anchor is ambiguous',file,anchor,head)
        if anchor:source=source[source.index(anchor):]
        assert compact(block(source,head))==expected,(label+' caller body changed',file,anchor,head)
    for key,name,suffix in restated:
        assert bodies[key]+'}'==suffix,(label+' restated product body changed',key)
        assert maintained(text,name).endswith('{'+suffix),(label+' restated maintained body changed',name)
    assert set(sites.values())<=set(callers),label+' site names an unknown maintained caller'
    return dict(callers=sorted(callers),bodies=sorted(' '.join(k) for k in bodies),restated=[r[1] for r in restated])
def verify(verus,root,out,crate,name,text,negative=False):
    file=Path(out)/(name+'.generated.rs');file.write_text(text);command,r=compose(verus,root,file,crate)
    log=r.stdout+r.stderr;(Path(out)/(name+'.verus.log')).write_text(log);counts=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
    assert counts and 'error[E' not in log and ((r.returncode!=0 and any(int(e) for _,e in counts)) if negative else (r.returncode==0 and all(int(e)==0 for _,e in counts))),(name,log[-4000:])
    return dict(name=name,exit_code=r.returncode,verification=counts,negative_control=negative)
def controls(verus,root,out,crate,model,solver,text_controls,check,label,prefix=''):
    """Run every control; `check(root)` is the area's correspondence function.

    `prefix` is text placed in front of the model in every solver run (the
    storage kernel); control patterns are matched in the model only."""
    text=Path(model).read_text();results=[];detected=[]
    for name,old,new in solver:assert text.count(old)==1,(label+' solver control pattern is not unique',name,text.count(old))
    for name,old,new in solver:
        results.append(verify(verus,root,out,crate,'caller-control-'+name,prefix+text.replace(old,new),True))
        detected.append(dict(name=name,detected_by='Verus rejects the weakened maintained caller'))
    for name,file,old,new in text_controls:
        with tempfile.TemporaryDirectory(prefix='jzap-caller-text-') as tmp:
            work=Path(tmp);shutil.copytree(Path(root)/'src',work/'src');path=work/'src'/file;source=path.read_text()
            assert source.count(old)==1,(name,source.count(old));path.write_text(source.replace(old,new))
            built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(work/'src/lib.rs'),'-o',str(work/'probe.rlib')],capture_output=True,text=True)
            assert built.returncode==0,(name,built.stderr[-2000:])
            try:check(work)
            except AssertionError as error:assert label+' caller body changed' in str(error),str(error)
            else:raise AssertionError(('changed '+label+' caller body accepted',name))
            detected.append(dict(name=name,compiled=True,detected_by='source correspondence comparison'))
    return results,detected
def prevalidate(root,text_controls,check,label):
    """Cheap standalone check that each text control compiles and is refused."""
    bad=[]
    for name,file,old,new in text_controls:
        with tempfile.TemporaryDirectory(prefix='jzap-caller-pre-') as tmp:
            work=Path(tmp);shutil.copytree(Path(root)/'src',work/'src');path=work/'src'/file;source=path.read_text()
            if source.count(old)!=1:bad.append((name,'anchor',source.count(old)));continue
            path.write_text(source.replace(old,new))
            built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(work/'src/lib.rs'),'-o',str(work/'probe.rlib')],capture_output=True,text=True)
            if built.returncode:bad.append((name,'compile',built.stderr[:300]));continue
            try:check(work);bad.append((name,'accepted'))
            except AssertionError as error:
                if label+' caller body changed' not in str(error):bad.append((name,str(error)))
    return bad
