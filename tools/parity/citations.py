"""How much of each ledger row's Compose test file has a port, from the `// Compose:` citations.

Ports cite the Kotlin test they rewrite in a comment: `// Compose: compose/.../FooTest.kt, testA, testB`
(continued on following `//` lines after a trailing comma). This script reads every such citation under
`crates/`, reads each row's Kotlin file for its `@Test` names, and prints, for the rows of a ledger that
aren't `ported` or `n/a`, how many of the file's tests are cited, biggest rows first. A row with all its
tests cited is likely `ported`; a `port` or `feature:` row with citations is likely `partial`.

Usage (from the repo root): python3 -I tools/parity/citations.py docs/parity/<ledger>.md [<androidx-main>]
The androidx checkout defaults to ../androidx-main; paths are relative to its frameworks/support.
"""
import collections
import os
import re
import sys

AX = os.path.join(sys.argv[2] if len(sys.argv) > 2 else '../androidx-main', 'frameworks', 'support')

cites = collections.defaultdict(set)
PATTERN = re.compile(r'Compose:\s*(compose/\S+?\.kt)\s*,?\s*(.*)')
for dirpath, _, files in os.walk('crates'):
    if '/target' in dirpath:
        continue
    for name in files:
        if not name.endswith('.rs'):
            continue
        lines = open(os.path.join(dirpath, name), errors='ignore').read().split('\n')
        for i, line in enumerate(lines):
            m = PATTERN.search(line)
            if not m:
                continue
            path, rest = m.group(1), m.group(2)
            j = i + 1
            while rest.rstrip().endswith(',') and j < len(lines) and re.match(r'\s*//', lines[j]) \
                    and 'Compose:' not in lines[j]:
                rest += ' ' + re.sub(r'^\s*//+\s*', '', lines[j])
                j += 1
            cites[path] |= set(re.findall(r'[A-Za-z_][A-Za-z_0-9]*', rest))


def kotlin_tests(path):
    full = os.path.join(AX, path)
    if not os.path.exists(full):
        return None
    src = open(full, errors='ignore').read().split('\n')
    names = []
    for i, line in enumerate(src):
        if re.match(r'\s*@(org\.junit\.)?Test\b', line):
            for j in range(i, min(i + 6, len(src))):
                m = re.search(r'fun\s+(?:`([^`]+)`|([A-Za-z_0-9]+))', src[j])
                if m:
                    names.append(m.group(1) or m.group(2))
                    break
    return names


def rows(ledger):
    for line in open(ledger):
        if not line.startswith('| `compose/'):
            continue
        cols = [c.strip() for c in line.split('|')]
        yield cols[1].strip('`'), int(cols[2].replace(',', '')), cols[4].replace('**', '').replace('`', '').strip()


results = []
for path, count, status in rows(sys.argv[1]):
    if status.split()[0].split(':')[0] in ('ported', 'n/a'):
        continue
    names = kotlin_tests(path)
    if names is None:
        results.append((count, 'MISSING', 0, status, path))
        continue
    cited = cites.get(path, set())
    results.append((count, len(names), sum(1 for n in names if n in cited), status, path))
for count, total, hit, status, path in sorted(results, key=lambda r: -r[0]):
    print(f'{count:4} {total!s:>7} cited={hit:3} {status[:34]:34} {path.split("/src/")[-1]}')
