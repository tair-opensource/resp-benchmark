import sys
from itertools import chain
from typing import List
import multiprocessing


def parse_range_list(rl):
    def parse_range(r):
        if len(r) == 0:
            return []
        parts = r.split("-")
        if len(parts) > 2:
            raise ValueError("Invalid range: {}".format(r))
        return range(int(parts[0]), int(parts[-1]) + 1)

    return sorted(set(chain.from_iterable(map(parse_range, rl.split(",")))))


def parse_cores_string(cores) -> List[int]:
    try:
        return parse_range_list(cores)
    except ValueError:
        print(f"Invalid cores range: {cores}.")
        sys.exit(1)
