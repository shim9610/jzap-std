"""Check weighted means and selection of valid completed attempts."""
import unittest
import csv
from pathlib import Path
import tempfile

from compare import VEC_LABELS, labels_for
from report import compact_records, operation_means, read_compact, validated_records, markdown_table
from scenario_report import apply_repeat_gate


class MeanReporting(unittest.TestCase):
    def test_deque_policy_labels_and_table(self):
        task="deque_fifo_8"
        self.assertEqual(labels_for(task),["std","std_control","unlimited","local","concurrent"])
        records=[dict(profile="mixed",task=task,variant="default",seed=1,label=label,
            operations=640,elements=1280,elapsed_seconds=1.2,phase="measurement")for label in labels_for(task)]
        rows=operation_means(records)
        table=markdown_table(rows,[task],family="VecDeque")
        self.assertIn("Local budget",table)
        self.assertIn("Concurrent budget",table)
        self.assertIn(task,table)

    def test_repeat_gate_keeps_variant_controls_separate(self):
        rows = [dict(task="arc_app_state8_t4",variant=v,label=l,
                     relative_percent=change,time_classification="within-15-percent")
                for v,change in [("cas",0),("headroom",20)]
                for l in ["std_control","owned"]]
        apply_repeat_gate(rows)
        self.assertTrue(all(r["repeat_required"]==(r["variant"]=="headroom") for r in rows))
        self.assertEqual(rows[0]["time_classification"],"within-15-percent")
        self.assertEqual(rows[-1]["time_classification"],"repeat-required-std-drift")

    def samples(self):
        return [dict(profile="mixed", task="clone", variant="default", label=label,
                     seed=seed, operations=count, elements=count * 8, elapsed_seconds=seconds,
                     phase="measurement")
                for seed, count, std, ours in [(1, 10, 1.0, 2.0), (2, 100, 4.0, 4.0)]
                for label, seconds in [("std", std), ("unlimited", ours)]]

    def test_mean_weights_executed_calls_instead_of_averaging_seed_rates(self):
        row = operation_means(self.samples())[0]
        self.assertAlmostEqual(row["std_mean_ns_per_operation"], 5e9 / 110)
        self.assertAlmostEqual(row["mean_ns_per_operation"], 6e9 / 110)
        self.assertAlmostEqual(row["seed_mean_ns_arithmetic"], (2e9 / 10 + 4e9 / 100) / 2)
        self.assertNotAlmostEqual(row["mean_ns_per_operation"], row["seed_mean_ns_arithmetic"])
        self.assertEqual(row["mean_recording_seconds"], 3)
        self.assertAlmostEqual(row["relative_percent"], 20)

    def test_outlier_is_preserved_and_unequal_calls_are_rejected(self):
        rows = self.samples()
        rows[-1]["elapsed_seconds"] = 10
        row = operation_means(rows)[0]
        self.assertAlmostEqual(row["mean_ns_per_operation"], 12e9 / 110)
        rows[-1]["operations"] = 99
        with self.assertRaisesRegex(AssertionError, "unequal call count"):
            operation_means(rows)

    def test_only_selected_complete_long_attempts_contribute(self):
        protocol = dict(minimum_seconds=1, plan=[["mixed", "clone", ["default"]]], seeds=[1])
        completed = [dict(profile="mixed", task="clone", seed=1, attempt=2)]
        rows = [dict(profile="mixed", task="clone", variant="default", label=label, seed=1,
                     phase="measurement", attempt=attempt, epochs=10, operations=640, elements=1280,
                     corpus_digest="same", cases=64, min_size=0, max_size=64, elapsed_seconds=seconds)
                for attempt, seconds in [(1, .999), (2, 1.2)] for label in VEC_LABELS]
        accepted = validated_records(protocol, completed, rows)
        self.assertEqual(len(accepted), len(VEC_LABELS))
        self.assertTrue(all(r["attempt"] == 2 for r in accepted))
        completed[0]["attempt"] = 1
        with self.assertRaisesRegex(AssertionError, "short interval"):
            validated_records(protocol, completed, rows)
        with self.assertRaisesRegex(AssertionError, "incomplete/duplicate"):
            validated_records(protocol, [], rows)


class CompactReporting(unittest.TestCase):
    def samples(self):
        return [dict(profile="mixed", task="binary_heap_push_8", variant="default",
                     seed=seed, epochs=epochs, cases=64, operations=epochs * 64,
                     elements=epochs * 64 * 8, min_size=0, max_size=32,
                     corpus_digest=18446744073709000000 + seed,
                     checksum=18446744073709551601, phase="measurement",
                     accepted_attempt=True, label=label, elapsed_seconds=seconds)
                for seed, epochs in [(617, 10), (6203, 30), (96011, 90)]
                for label, seconds in [("std", 1.2), ("std_control", 1.21),
                                       ("unlimited", 1.3), ("local", 1.4),
                                       ("concurrent", 1.5)]]

    def test_csv_round_trip_preserves_weighted_means_and_integer_digests(self):
        records = self.samples()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matched.csv"
            compact_records(records, path)
            restored = read_compact(path)
            self.assertEqual(operation_means(records), operation_means(restored))
            self.assertEqual(len(restored), 15)
            self.assertEqual({r["checksum"] for r in restored}, {18446744073709551601})

    def test_rejects_duplicate_groups_and_nonfinite_or_short_intervals(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matched.csv"
            compact_records(self.samples(), path)
            original = path.read_text()
            path.write_text(original + original.splitlines()[-1] + "\n")
            with self.assertRaisesRegex(AssertionError, "duplicate"):
                read_compact(path)
            for duration in ("nan", "inf", "0.999"):
                with self.subTest(duration=duration):
                    rows = list(csv.DictReader(original.splitlines()))
                    rows[0]["unlimited_seconds"] = duration
                    with path.open("w", newline="") as stream:
                        writer = csv.DictWriter(stream, fieldnames=rows[0])
                        writer.writeheader()
                        writer.writerows(rows)
                    with self.assertRaisesRegex(AssertionError, "invalid/short"):
                        read_compact(path)

    def test_export_rejects_unequal_work(self):
        records = self.samples()
        records[-1]["operations"] += 1
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(AssertionError, "unequal input or work"):
                compact_records(records, Path(directory) / "matched.csv")


if __name__ == "__main__":
    unittest.main()
