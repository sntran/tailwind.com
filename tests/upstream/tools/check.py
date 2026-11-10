#!/usr/bin/env python3
# Usage: python3 check.py <fixture directory>
# This script reads each fixture file and prints the number of records.
import collections, gzip, json, os, sys

d = sys.argv[1] if len(sys.argv) > 1 else '.'

suite = [json.loads(l) for l in gzip.open(os.path.join(d, 'suite.jsonl.gz'), 'rt', encoding='utf-8')]
builds = sum(len(r['builds']) for r in suite)
print(f'suite.jsonl.gz: {len(suite)} compile records, {builds} builds, {sum("error" in r for r in suite)} compile errors')

classes = [json.loads(l) for l in gzip.open(os.path.join(d, 'classes.jsonl.gz'), 'rt', encoding='utf-8')]
kinds = collections.Counter(r['kind'] for r in classes)
empty = collections.Counter(r['css'] for r in classes).most_common(1)[0]
print(f'classes.jsonl.gz: {len(classes)} records, {dict(kinds)}, {empty[1]} records with the most common (empty) output')

all_ = json.load(gzip.open(os.path.join(d, 'classes-all.json.gz'), 'rt', encoding='utf-8'))
assert all_['candidates'] == [r['candidate'] for r in classes]
print(f'classes-all.json.gz: {len(all_["candidates"])} candidates, css {len(all_["css"])}, optimized {len(all_["optimized"])}, minified {len(all_["minified"])} characters')

full = json.load(gzip.open(os.path.join(d, 'full.json.gz'), 'rt', encoding='utf-8'))
for e in full:
    print(f'full.json.gz: entry with {len(e["candidates"])} candidates, css {len(e["css"])}, optimized {len(e["optimized"])}, minified {len(e["minified"])} characters')

for group in ('cli', 'cli-map', 'cli-js'):
    cli = os.path.join(d, group)
    for name in sorted(os.listdir(cli)):
        c = os.path.join(cli, name)
        if not os.path.isdir(c):
            continue
        code = open(os.path.join(c, 'exit.txt')).read().strip()
        size = os.path.getsize(os.path.join(c, 'stdout.css'))
        args = ' '.join(open(os.path.join(c, 'args.txt')).read().split())
        files = sum(len(f) for _, _, f in os.walk(os.path.join(c, 'files')))
        print(f'{group}/{name}: exit {code}, stdout {size} bytes, {files} files, args: {args}')

maps = [json.loads(l) for l in gzip.open(os.path.join(d, 'source-maps.jsonl.gz'), 'rt', encoding='utf-8')]
builds = [b for r in maps for b in r['builds']]
print(f'source-maps.jsonl.gz: {len(maps)} records, {len(builds)} builds, '
      f'{sum(len(b["map"]["mappings"]) for b in builds)} mappings, {sum("error" in r for r in maps)} errors')
