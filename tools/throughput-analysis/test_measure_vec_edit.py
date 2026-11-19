import copy
import unittest

from measure_vec_edit import paired_bootstrap, validate


class PairedTimingTests(unittest.TestCase):
    def rows(self):
        return [dict(block='0', seed='1', case='0', attempt='1', phase='measurement',
            label=label, position=str(i), epochs='100', operations='6400',
            wall_seconds='1.2', cpu_seconds='1.1', checksum='7', corpus_digest='abc')
            for i, label in enumerate(('std', 'old', 'new', 'old_control'))]

    def test_incomplete_duplicate_short_or_unequal_work_is_rejected(self):
        plan = [dict(block=0, seed=1)]
        validate(self.rows(), plan, cases=1)
        for field, value in [('wall_seconds', '.999'), ('operations', '6401'),
                             ('checksum', '8'), ('corpus_digest', 'different'), ('attempt', '2')]:
            rows = self.rows(); rows[0][field] = value
            with self.subTest(field=field), self.assertRaises(AssertionError):
                validate(rows, plan, cases=1)
        for rows in (self.rows()[:-1], self.rows() + [copy.copy(self.rows()[0])]):
            with self.assertRaises(AssertionError):
                validate(rows, plan, cases=1)

    def test_bootstrap_keeps_pairing_and_uses_weighted_totals(self):
        # Arbitrarily large shared block noise cancels for a constant paired effect.
        pairs = [(seed, 2 * t, t) for seed in (1, 2, 3) for t in (1, 100, 10000, 1000000)]
        result = paired_bootstrap(pairs, resamples=2000)
        self.assertTrue(all(v == 2 for v in result.values()))

    def test_wide_control_noise_is_visible(self):
        pairs = [(seed, a, 1) for seed in (1, 2, 3) for a in (.5, .8, 1.5, 2)]
        result = paired_bootstrap(pairs, resamples=2000)
        self.assertLess(result['low95'], .95)
        self.assertGreater(result['high95'], 1.05)


if __name__ == '__main__':
    unittest.main()
