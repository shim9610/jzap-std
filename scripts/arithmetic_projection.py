"""Project proof-bounded wrapping calls to mathematical arithmetic.

The generated verifier must check overflow/underflow on the resulting primitive
operations. Actual bounded helpers additionally assert the checked-operation
precondition under Kani. This projection must never license modular overflow in
collection metadata.
"""
import re

def bounded_projection(text):
    pattern = re.compile(r'(?:(?:crate\s*::\s*)?arithmetic\s*::\s*bounded\s*::\s*)?wrapping_(add|sub|mul)\s*\(')
    while match := pattern.search(text):
        opening=match.end()-1;end=opening+1;depth=1;comma=None
        while depth:
            char=text[end]
            if char=='(': depth+=1
            elif char==')': depth-=1
            elif char==',' and depth==1:
                if comma is not None:
                    if not text[end+1:].lstrip().startswith(')'): raise ValueError('bounded arithmetic requires two operands')
                else: comma=end
            end+=1
        if comma is None: raise ValueError('bounded arithmetic requires two operands')
        left=bounded_projection(text[opening+1:comma]);right=bounded_projection(text[comma+1:end-1].strip().removesuffix(','));op={'add':'+','sub':'-','mul':'*'}[match[1]]
        replacement=f'({left.strip()} {op} {right.strip()})'
        text=text[:match.start()]+replacement+text[end:]
    return text

def unparenthesized(text):
    text=text.strip()
    while text.startswith('(') and text.endswith(')'):
        depth=0
        for i,char in enumerate(text):
            depth+=(char=='(')-(char==')')
            if depth==0:break
        if i != len(text)-1:break
        text=text[1:-1].strip()
    return text


def replace_once(source, old, new):
    pattern = r'\s*'.join((r',?\s*\)' if char==')' else re.escape(char)) for char in re.sub(r'\s+', '', old))
    matches=list(re.finditer(pattern, source))
    if len(matches)!=1: raise ValueError(('source mutation anchor',old,len(matches)))
    match=matches[0]
    return source[:match.start()]+new+source[match.end():]


def validate_bounded(source_root):
    """Verify the actual helpers before other, still-projected caller models.

    This no longer extracts or pattern-matches their Rust function bodies.
    The caller projections retain their explicitly documented translation gap.
    """
    from verify_direct_verus import verify
    from pathlib import Path
    out=Path(__file__).resolve().parents[1]/'target/verification/direct-verus/prerequisites'
    verify(root=source_root,out=out,targets=['bounded'])
