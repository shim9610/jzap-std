"""Check the callers of retained guards in GNU objdump output."""

from collections import defaultdict
import re

SYMBOL = re.compile(r"^([0-9a-f]+) <(.+)>:$")
ADDRESS = re.compile(r"#\s*([0-9a-f]+)\s+<")
BRANCH = re.compile(r"\b(?:callq?|j[a-z]+)\s+(?:\*[^#]+#\s*([0-9a-f]+)\s|([0-9a-f]+)\s+<)")

PAIR_WINDOW_NEXT = "<core::slice::iter::ArrayWindows<T,_> as core::iter::traits::iterator::Iterator>::next"
PAIR_WINDOW_PATH = (
    "core::iter::traits::iterator::Iterator::try_fold",
    "core::slice::<impl [T]>::is_sorted_by",
    "jzap::collections::btree_map::view::bplus::Node<K,V,I>::new",
)
SLICE_INDEX_FAIL = "core::slice::index::slice_index_fail"


class LinkCallGraph:
    def __init__(self, assembly, relocations):
        self.names = {}
        self.callers = defaultdict(set)
        got = {
            int(slot, 16): int(target, 16)
            for slot, target in re.findall(
                r"^([0-9a-f]+)\s+R_X86_64_RELATIVE\s+\*ABS\*\+0x([0-9a-f]+)", relocations, re.M
            )
        }
        current = None
        for line in assembly.splitlines():
            if match := SYMBOL.match(line):
                current = int(match[1], 16)
                self.names[current] = match[2]
            elif current is not None:
                if match := BRANCH.search(line):
                    target = got.get(int(match[1], 16)) if match[1] else int(match[2], 16)
                elif match := ADDRESS.search(line):
                    address = int(match[1], 16)
                    target = got.get(address, address)
                else:
                    continue
                self.callers[target].add(current)

    def incoming(self, address):
        return self.callers[address] - {address}

    def verify_path(self, address, expected):
        if expected == {()}:
            return {()}
        callers = self.incoming(address)
        actual_names = {self.names[caller] for caller in callers}
        expected_names = {path[0] for path in expected if path}
        if not actual_names or not actual_names <= expected_names:
            raise ValueError(f"unexpected callers of {self.names[address]}: {sorted(actual_names)}")
        return {
            (self.names[caller], *tail)
            for caller in callers
            for tail in self.verify_path(caller, {path[1:] for path in expected if path[0] == self.names[caller]})
        }


def is_pair_window_edge(edge):
    symbol = SYMBOL.fullmatch(edge["function"])
    return (symbol is not None and symbol[2] == PAIR_WINDOW_NEXT
            and edge["target"] == SLICE_INDEX_FAIL
            and edge["location"].endswith("/core/src/slice/index.rs:569"))


def bind_pair_window_edges(edges, assembly, relocations):
    candidates = [edge for edge in edges if is_pair_window_edge(edge)]
    if candidates:
        graph = LinkCallGraph(assembly, relocations)
        for edge in candidates:
            address = int(SYMBOL.fullmatch(edge["function"])[1], 16)
            edge["caller_paths"] = [list(path) for path in sorted(graph.verify_path(address, {PAIR_WINDOW_PATH}))]
