#!/usr/bin/env python3
"""Checks the translations in po/: everything except the app name must be translated, with no fuzzy entries, and placeholders ({}, {0}, {1}) must match the source.

Usage: check-translations.py <po directory> (run by Meson's check-translations test).
"""

import re
import sys
from pathlib import Path

# Strings intentionally left untranslated.
UNTRANSLATED = {"Anemobit"}
PLACEHOLDER = re.compile(r"\{\d*\}")


def unquote(lines):
    return "".join(re.match(r'^"(.*)"$', line).group(1) for line in lines).encode().decode("unicode_escape").encode("latin-1").decode("utf-8")


def entries(path):
    """Returns (msgid, msgid_plural, [msgstr...], fuzzy)."""
    blocks = path.read_text(encoding="utf-8").split("\n\n")
    for block in blocks:
        lines = [line for line in block.splitlines() if line and not line.startswith("#~")]
        fuzzy = any(line.startswith("#,") and "fuzzy" in line for line in lines)
        fields = {}
        current = None
        for line in lines:
            if line.startswith("#"):
                continue
            match = re.match(r'^(msgid|msgid_plural|msgstr(?:\[\d+\])?) (".*")$', line)
            if match:
                current = match.group(1)
                fields[current] = [match.group(2)]
            elif line.startswith('"') and current:
                fields[current].append(line)
        if "msgid" not in fields:
            continue
        msgid = unquote(fields["msgid"])
        if not msgid:
            continue
        plural = unquote(fields["msgid_plural"]) if "msgid_plural" in fields else None
        strings = [unquote(v) for k, v in sorted(fields.items()) if k.startswith("msgstr")]
        yield msgid, plural, strings, fuzzy


def main():
    directory = Path(sys.argv[1])
    problems = []
    languages = (directory / "LINGUAS").read_text(encoding="utf-8").split()
    for language in languages:
        path = directory / f"{language}.po"
        for msgid, plural, strings, fuzzy in entries(path):
            where = f"{path.name}: {msgid!r}"
            if fuzzy:
                problems.append(f"{where}: fuzzy")
            if msgid in UNTRANSLATED:
                continue
            if not any(strings):
                problems.append(f"{where}: missing translation")
                continue
            expected = sorted(PLACEHOLDER.findall(plural or msgid))
            for string in strings:
                if sorted(PLACEHOLDER.findall(string)) != expected:
                    problems.append(f"{where}: placeholders differ in {string!r}")
            if msgid.endswith(";") and not all(s.endswith(";") for s in strings):
                problems.append(f"{where}: keyword list must end with ';'")
    for problem in problems:
        print(problem)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
