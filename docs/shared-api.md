# Shared ownership API mapping

The reference is Rust 1.95.0 (`59807616e`) `alloc/src/rc.rs` and
`alloc/src/sync.rs`. Each file contains the same 24 distinct stable public
inherent method names across its strong and weak types. Overloads and duplicate
names are grouped below. `scripts/check_shared_api.py` compares the installed
pinned sources and rejects missing or newly introduced method names.

| Stable std name | Explicit jzap counterpart |
| --- | --- |
| `new` | `try_new / Weak::new` |
| `new_cyclic` | `try_new_cyclic` |
| `new_uninit` | `try_new_uninit` |
| `new_zeroed` | `try_new_zeroed` |
| `new_uninit_slice` | `try_new_uninit_slice` |
| `new_zeroed_slice` | `try_new_zeroed_slice` |
| `assume_init` | `assume_init` |
| `pin` | `try_pin` |
| `try_unwrap` | `try_unwrap` |
| `into_inner` | `into_inner` |
| `unwrap_or_clone` | `try_unwrap_or_clone` |
| `get_mut` | `get_mut` |
| `make_mut` | `try_make_mut` |
| `downgrade` | `try_downgrade` |
| `upgrade` | `try_upgrade` |
| `strong_count` | `strong_count` |
| `weak_count` | `weak_count` |
| `ptr_eq` | `ptr_eq` |
| `as_ptr` | `as_ptr` |
| `into_raw` | `into_raw` |
| `from_raw` | `from_raw` |
| `increment_strong_count` | `try_increment_strong_count` |
| `decrement_strong_count` | `decrement_strong_count` |
| `downcast` | `downcast` |

Names are shared by the Rc and Arc aliases. `_in` constructors select a policy;
`try_*` count operations return `Error::RefCount`. Unsafe initialization/raw
operations retain their explicit safety contracts. `Weak::as_ptr` uses an Option
for empty state; raw ownership points to the header, not the payload.

| Related standard capability | jzap interface |
| --- | --- |
| Clone / target-budget clone | `TryClone`, `TryCloneIn`; handle clones retain their original allocation policy |
| Default | `TryDefault` for values, slices, str, CStr and std OS/path shapes; empty Weak is allocation-free |
| From arrays and slices | `try_from_array[_in]`, `try_from_slice[_in]`, `try_copy_from_slice[_in]`, `into_slice`, `try_into_array` |
| From Box / Vec | `try_from_box_in`, `try_from_box_with`, `try_from_std_box[_in]`, `try_from_vec_in`, `try_from_std_vec[_in]` |
| From String / Cow / borrowed values | `try_from_string_in`, `try_from_std_string[_in]`, `try_from_cow[_in]`, `try_from_ref[_in]` |
| FromIterator | `try_from_iter[_in]`; `try_from_fallible_iter_in` uses the open fallible iterator contract |
| Shared str to bytes | `into_bytes` transfers the same shared allocation |
| Unsize / dispatch from dynamic | Compiler-checked strong/weak and pinned unsizing macros; no nightly language feature |
| Pin | `try_pin[_in]`, `try_pin_init_in`, `Pinned`; existing-owner safe conversion checks Unpin or uniqueness |
| Deref / AsRef / Borrow | Sized Deref; explicit slice/str borrows; generic AsRef and Borrow |
| Comparison / Hash / formatting / Error | Forwarding to the payload implementation, preserving its execution contract |
| Unpin / unwind markers | Handle markers with payload/policy bounds; pinned extraction retains its own restrictions |
| Send / Sync | Atomic owners with Send+Sync payload/policy; compiler rejection for Rc and local budgets |

Nightly allocator APIs and compiler-only trait syntax are outside the pinned
stable API inventory. Policies select budget accounting; explicit adapters
provide stable coercion behavior. No infallible standard Clone/FromIterator bridge is added.
The generic Box metadata move uses the pinned Rust wide-pointer data-first
application binary interface (ABI). Miri exercises arbitrary dynamically sized
types on 64- and 32-bit targets; native Kani covers an actual slice transfer.
This ABI boundary is distinct from the numeric proofs.
