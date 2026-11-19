"""Negative controls for the long-interval comparison contract."""
import copy
import unittest
from compare import summarize, validate_equal_work


class ComparisonContract(unittest.TestCase):
    def records(self):
        return [dict(variant=v, label=l, epochs=1000, operations=64000,
                     elements=8192000, corpus_digest="0123456789abcdef", cases=64,
                     min_size=0, max_size=131072, elapsed_seconds=1.2)
                for v in ["default", "align64"] for l in ["std", "unlimited"]]

    def test_equal_work_and_long_intervals(self):
        validate_equal_work(self.records(), ["std", "unlimited"], ["default", "align64"], 1)

    def test_changed_input_count_short_missing_and_duplicate_are_rejected(self):
        for key, value in [("corpus_digest", "other"), ("epochs", 1001),
                           ("operations", 63999), ("elements", 1), ("elapsed_seconds", .999)]:
            rows = self.records()
            rows[0][key] = value
            with self.subTest(key=key), self.assertRaises(AssertionError):
                validate_equal_work(rows, ["std", "unlimited"], ["default", "align64"], 1)
        for rows in [self.records()[:-1], self.records() + [self.records()[0]]]:
            with self.assertRaises(AssertionError):
                validate_equal_work(rows, ["std", "unlimited"], ["default", "align64"], 1)

    def test_summary_compares_total_seconds_for_the_same_work(self):
        rows = []
        for seed, base, ours in [(1, 1.0, 2.0), (2, 4.0, 4.0)]:
            for label, elapsed in [("std", base), ("unlimited", ours)]:
                row = copy.copy(self.records()[0])
                row.update(profile="mixed", task="retain", variant="default", label=label,
                           seed=seed, elapsed_seconds=elapsed, phase="measurement", accepted_attempt=True)
                rows.append(row)
        result = summarize(rows)[0]
        self.assertEqual(result["std_total_seconds"], 5)
        self.assertEqual(result["total_seconds"], 6)
        self.assertEqual(result["ratio"], 1.2)

    def test_workflow_checksum_mismatch_and_missing_result_are_rejected(self):
        rows = self.records()
        for r in rows:
            r["checksum"] = 19
        validate_equal_work(rows, ["std", "unlimited"], ["default", "align64"], 1)
        rows[0]["checksum"] = 20
        with self.assertRaisesRegex(AssertionError, "unequal workload result"):
            validate_equal_work(rows, ["std", "unlimited"], ["default", "align64"], 1)
        del rows[0]["checksum"]
        with self.assertRaisesRegex(AssertionError, "unequal workload result"):
            validate_equal_work(rows, ["std", "unlimited"], ["default", "align64"], 1)


if __name__ == "__main__":
    unittest.main()
