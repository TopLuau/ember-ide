#!/usr/bin/env python3
"""Write manifest.json for a downloads folder and enforce the per-file size limit.

    python scripts/make-manifest.py dist 0.1.0

Every release file must stay under LIMIT_MB (Vercel's per-file upload limit is 100 MB).
Exits with an error if any file is larger, so CI fails instead of shipping a broken site.
"""
import hashlib
import json
import os
import sys

LIMIT_MB = 95

folder = sys.argv[1] if len(sys.argv) > 1 else "dist"
version = (sys.argv[2] if len(sys.argv) > 2 else "").lstrip("v")

files, too_big = [], []
for name in sorted(os.listdir(folder)):
    path = os.path.join(folder, name)
    if not os.path.isfile(path) or name in ("manifest.json", "SHA256SUMS.txt", ".gitkeep"):
        continue
    size = os.path.getsize(path)
    if size > LIMIT_MB * 1024 * 1024:
        too_big.append((name, size))
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    files.append({"name": name, "size": size, "sha256": h.hexdigest()})

if too_big:
    for n, s in too_big:
        print(f"TOO BIG: {n} is {s / 1048576:.1f} MB (limit {LIMIT_MB} MB)")
    sys.exit(1)

if not version:
    for f in files:
        parts = f["name"].split("-")
        for p in parts:
            if p[:1].isdigit() and p.count(".") >= 1:
                version = p
                break
        if version:
            break

with open(os.path.join(folder, "manifest.json"), "w", encoding="utf-8") as f:
    json.dump({"version": version, "files": files}, f, indent=2)

with open(os.path.join(folder, "SHA256SUMS.txt"), "w", encoding="utf-8") as f:
    for x in files:
        f.write(f"{x['sha256']}  {x['name']}\n")

total = sum(x["size"] for x in files)
print(f"{len(files)} files, {total / 1048576:.1f} MB total, largest {max([x['size'] for x in files] or [0]) / 1048576:.1f} MB")
