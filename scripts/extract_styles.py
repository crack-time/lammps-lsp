"""Extract all unique style names from commands.json for tree-sitter highlighting.

Outputs a tree-sitter query using #match? predicate to be added to highlights.scm.
"""
import json
import os
import re
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DATA_PATH = os.path.join(SCRIPT_DIR, "..", "data", "commands.json")

def extract_styles():
    with open(DATA_PATH, "r", encoding="utf-8") as f:
        commands = json.load(f)

    styles = set()

    for entry in commands:
        for cmd in entry.get("command", []):
            parts = cmd.split()
            if len(parts) >= 2:
                style = " ".join(parts[1:])
                if style and not re.match(r'^[A-Z_]+$', style) and style not in ("ID", "group-ID", "style", "style_name"):
                    styles.add(style)

    return sorted(styles)

def escape_regex(s):
    """Escape a style name for use in a regex alternation group."""
    return re.escape(s)

def main():
    styles = extract_styles()
    print(f"Found {len(styles)} unique style names", file=sys.stderr)

    # Sort by length descending to put longer matches first (avoids partial match issues)
    styles.sort(key=lambda s: (-len(s), s))

    escaped = [escape_regex(s) for s in styles]
    pattern = "^(?:" + "|".join(escaped) + ")$"

    query = f"""; LAMMPS command styles
((word) @keyword.style
 (#match? @keyword.style "{pattern}"))"""

    print(query)

if __name__ == "__main__":
    main()
