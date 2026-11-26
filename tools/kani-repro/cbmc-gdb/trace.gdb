# Print the inputs and call stacks of the CBMC code that builds the wrong
# dereference. Use with a debug build of CBMC 6.11.0 (see run.sh). Line
# numbers are those of the cbmc-6.11.0 tag.
set pagination off
set print pretty off
set breakpoint pending on
# value_set_dereferencet::build_reference_to: the member/index fast path.
break value_set_dereference.cpp:648
commands
  silent
  printf "== build_reference_to:648\n"
  printf "pointer_expr  : %s\n", format_to_string<exprt>(pointer_expr).c_str()
  printf "root_object   : %s\n", format_to_string<exprt>(root_object).c_str()
  printf "o.offset()    : %s\n", format_to_string<exprt>(o.offset()).c_str()
  continue
end
break value_set_dereference.cpp:653
commands
  silent
  if subexpr.has_value()
    printf "subexpr       : %s\n", format_to_string<exprt>(*subexpr).c_str()
  else
    printf "subexpr       : none\n"
  end
  continue
end
# get_subexpression_at_offset(expr, exprt offset, ...): the array branch's
# "offset C + x" recursion.
break pointer_offset_size.cpp:785
commands
  silent
  printf "== C+x recursion, pointer_offset_size.cpp:785\n"
  printf "expr          : %s\n", format_to_string<exprt>(expr).c_str()
  printf "offset        : %s\n", format_to_string<exprt>(offset).c_str()
  printf "x             : %s\n", format_to_string<exprt>(other_factor).c_str()
  printf "expr_at_C     : %s\n", format_to_string<exprt>(*expr_at_offset_C).c_str()
  printf "recurses into : %s\n", format_to_string<exprt>(to_index_expr(*expr_at_offset_C).array()).c_str()
  bt 12
  continue
end
# The struct branch's "offset C + x" recursion, for comparison.
break pointer_offset_size.cpp:914
commands
  silent
  printf "== C+x recursion (struct), pointer_offset_size.cpp:914\n"
  printf "expr          : %s\n", format_to_string<exprt>(expr).c_str()
  printf "offset        : %s\n", format_to_string<exprt>(offset).c_str()
  bt 12
  continue
end
run
quit
