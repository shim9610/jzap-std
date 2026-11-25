# Causal check with a debug build of CBMC 6.11.0: run unchanged, except that
# the array branch's "offset C + x" recursion is skipped, so the call falls
# through to "give up" and CBMC keeps its generic byte_extract encoding.
# Not a fix: it disables the rewrite for every caller.
set pagination off
set breakpoint pending on
# Skip only the "C + x" recursion of the array branch: continue at the
# multiplication check, which returns no subexpression for a sum offset.
break pointer_offset_size.cpp:785
commands
  silent
  printf "skipping C+x recursion into %s\n", format_to_string<exprt>(to_index_expr(*expr_at_offset_C).array()).c_str()
  jump 825
end
run
quit
