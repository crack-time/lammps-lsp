#!/usr/bin/env python3
"""
Extract nested keyword lists from 'parameters' text and add them
as additional type=3 choices in the corresponding args slot.
"""

import json
import re
import sys
from pathlib import Path


def extract_sub_keywords(parameters: str):
    """Parse parameters text for 'keyword = ... or ... or ...' patterns."""
    lines = parameters.split("\n")
    for line in lines:
        if "=" not in line:
            continue
        eq = line.find("=")
        left = line[:eq].replace("*", "").replace("&#160;", "").strip()
        right = line[eq + 1 :].strip()
        if left == "keyword" and " or " in right:
            keywords = [
                k.strip()
                for k in right.split(" or ")
                if k.strip() and "=" not in k and " " not in k
            ]
            if len(keywords) > 1:
                return keywords
    return []


def main():
    data_path = Path(__file__).resolve().parent.parent.parent / "data" / "commands.json"
    if not data_path.exists():
        print(f"Error: {data_path} not found")
        sys.exit(1)

    with open(data_path) as f:
        data = json.load(f)

    total_added = 0
    modified = 0

    for entry in data:
        params = entry.get("parameters", "")
        sub_keywords = extract_sub_keywords(params)
        if not sub_keywords:
            continue

        # Find the first type=3 slot to add keywords to
        for variant in entry.get("args", []):
            for slot in variant:
                if slot.get("type") == 3:
                    existing = set(slot.get("choices", []))
                    new_kw = [kw for kw in sub_keywords if kw not in existing]
                    if new_kw:
                        slot.setdefault("choices", []).extend(new_kw)
                        total_added += len(new_kw)
                        modified += 1
                    break  # Only modify the first type=3 slot
            break  # Only process the first variant

    with open(data_path, "w") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

    print(
        f"Modified {modified} entries, added {total_added} sub-keywords to {data_path}"
    )


if __name__ == "__main__":
    main()
