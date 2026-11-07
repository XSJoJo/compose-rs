"""Resolve docs/decisions.md after a merge: keep both sides' rows, recount the Files table, check anchors.

Run from the repo root with `python3 -I`. Exits non-zero if the index and the anchors disagree.
"""
import collections
import glob
import re
import sys

p = 'docs/decisions.md'
t = open(p).read()
t = re.sub(r'^(<<<<<<<|=======|>>>>>>>)[^\n]*\n', '', t, flags=re.M)

lines = t.split('\n')
seen = {}
out = []
for line in lines:
    m = re.match(r'^\| D(\d+) \|', line)
    if m:
        n = int(m.group(1))
        if n in seen:
            if len(line) > len(out[seen[n]]):
                out[seen[n]] = line
            continue
        seen[n] = len(out)
    out.append(line)

files_seen = set()
deduped = []
for line in out:
    m = re.match(r'^\| \[([\w-]+)\]\(decisions/[\w-]+\.md\) \|', line)
    if m:
        if m.group(1) in files_seen:
            continue
        files_seen.add(m.group(1))
    deduped.append(line)
out = deduped
idx = [i for i, l in enumerate(out) if re.match(r'^\| D\d+ \|', l)]
if idx:
    rows = sorted((out[i] for i in idx), key=lambda l: int(re.match(r'^\| D(\d+)', l).group(1)))
    for i, r in zip(idx, rows):
        out[i] = r
t = '\n'.join(out)

counts = {f[len('docs/decisions/'):-3]: len(re.findall(r'<a id="d\d+"', open(f).read()))
          for f in glob.glob('docs/decisions/*.md')}
t = re.sub(r'^(\| \[([\w-]+)\]\(decisions/[\w-]+\.md\) \| .* \| )\d+( \|)$',
           lambda m: f'{m.group(1)}{counts[m.group(2)]}{m.group(3)}', t, flags=re.M)
open(p, 'w').write(t)

ids = collections.Counter()
for f in glob.glob('docs/decisions/*.md'):
    for x in re.findall(r'<a id="d(\d+)"', open(f).read()):
        ids[int(x)] += 1
rows = collections.Counter(int(x) for x in re.findall(r'^\| D(\d+) \|', t, re.M))
dups = [k for k, v in ids.items() if v > 1]
missing = sorted(set(ids) - set(rows))
orphans = sorted(set(rows) - set(ids))
print('anchors', sum(ids.values()), 'index', sum(rows.values()), 'dups', dups, 'missing', missing, 'orphans', orphans)
sys.exit(1 if dups or missing or orphans else 0)
