import re
from typing import List


def parse_dbids_string(dbid: str) -> List[int]:
    """Parse database IDs and inclusive ranges, preserving order without duplicates."""
    dbids = {}
    for part in dbid.split(","):
        match = re.fullmatch(r"\s*([0-9]+)\s*(?:-\s*([0-9]+)\s*)?", part)
        if match is None:
            raise ValueError("Invalid dbid: use non-negative IDs or ranges, e.g. 0,1,2,255 or 0-2,255")
        start = int(match.group(1))
        end = int(match.group(2)) if match.group(2) is not None else start
        if start > end:
            raise ValueError(f"Invalid dbid range: {part.strip()} (start must not exceed end)")
        if end > 2**63 - 1:
            raise ValueError("dbid must not exceed 9223372036854775807")
        dbids.update(dict.fromkeys(range(start, end + 1)))
    return list(dbids)
