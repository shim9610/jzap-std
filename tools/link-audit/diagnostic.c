/* Used only after a failed strict link. Resolving the deliberate sink lets us
 * inspect retained callers and run fault cases; it does NOT turn the gate green. */
#include <stdio.h>
#include <stdlib.h>
void jzap_link_audit_panic_reachable(void) {
    fputs("link-audit diagnostic: panic reached\n", stderr);
    abort();
}
/* Prebuilt core may retain an unwind-table personality reference in debug.
 * panic=abort must never execute it; a call is a diagnostic failure too. */
void rust_eh_personality(void) {
    fputs("link-audit diagnostic: unwind personality called\n", stderr);
    abort();
}
