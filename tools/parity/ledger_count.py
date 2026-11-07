import re, sys, collections
tot = collections.defaultdict(lambda: [0, 0]); feat = collections.defaultdict(lambda: [0, 0])
for line in open(sys.argv[1]):
    if not line.startswith('| `compose/'): continue
    cols = [c.strip() for c in line.split('|')]
    n = int(cols[2].replace(',', '')); st = cols[4].replace("**", "`").replace("``", "`")
    st = st if st.startswith("`") else "`" + st.split()[0] + "`"
    m = re.match(r'`([a-z/]+)(?::\s*([^`]*))?`', st)
    if not m: print('??', cols[1][:80], st[:40]); continue
    k = m.group(1)
    key = 'feature:<name>' if k == 'feature' else k
    tot[key][0] += 1; tot[key][1] += n
    if k == 'feature': f = feat[m.group(2).strip()]; f[0] += 1; f[1] += n
for k, v in tot.items(): print(k, v)
print('; '.join(f'`{k}` {v[0]}, {v[1]}' for k, v in sorted(feat.items(), key=lambda kv: (-kv[1][1], kv[0]))))
