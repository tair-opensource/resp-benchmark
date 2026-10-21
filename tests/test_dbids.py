import unittest

from resp_benchmark.dbids import parse_dbids_string


class ParseDbidsTests(unittest.TestCase):
    def test_single_list_and_ranges(self):
        for text, expected in [
            ("0", [0]),
            ("255", [255]),
            ("0,1,2,255", [0, 1, 2, 255]),
            ("0-2,255", [0, 1, 2, 255]),
            (" 255, 0 - 2, 2, 1-3 ", [255, 0, 1, 2, 3]),
            ("2-2", [2]),
            ("9223372036854775807", [9223372036854775807]),
        ]:
            with self.subTest(text=text):
                self.assertEqual(parse_dbids_string(text), expected)

    def test_invalid_input(self):
        for text in ["", " ", ",", "0,", ",0", "0,,1", "-1", "2-0", "0--2", "0-1-2", "1.5", "a", "1_0", "9223372036854775808"]:
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    parse_dbids_string(text)


if __name__ == "__main__":
    unittest.main()
