#!/usr/bin/env python3
"""
Mass LeetCode Knowledge Generator
Generates problem entries for IDs 1-3000 in large batches.
Uses a template-based approach with problem metadata.

Usage:
  python mass_generator.py <start_id> <end_id>
  python mass_generator.py merge
"""
import json
import os
import sys

BASE_DIR = '/Users/lipingjiang/Codes/hi/data/leetcode_knowledge'
BATCH_DIR = os.path.join(BASE_DIR, 'batches')
ALL_FILE = os.path.join(BASE_DIR, 'problems_all.json')

os.makedirs(BATCH_DIR, exist_ok=True)


def load_existing_ids():
    """Load IDs already in problems_all.json."""
    if os.path.exists(ALL_FILE):
        with open(ALL_FILE) as f:
            return set(p['id'] for p in json.load(f))
    return set()


def merge_all():
    """Merge all batch files into problems_all.json."""
    # Load existing
    if os.path.exists(ALL_FILE):
        with open(ALL_FILE) as f:
            all_problems = json.load(f)
    else:
        all_problems = []
    
    seen_ids = set(p['id'] for p in all_problems)
    
    # Load all batch files
    batch_files = sorted(f for f in os.listdir(BATCH_DIR) if f.endswith('.json'))
    new_count = 0
    for bf in batch_files:
        with open(os.path.join(BATCH_DIR, bf)) as f:
            batch = json.load(f)
        for p in batch:
            if p['id'] not in seen_ids:
                all_problems.append(p)
                seen_ids.add(p['id'])
                new_count += 1
    
    all_problems.sort(key=lambda x: x['id'])
    
    with open(ALL_FILE, 'w', encoding='utf-8') as f:
        json.dump(all_problems, f, ensure_ascii=False, indent=2)
    
    # Stats
    categories = {}
    difficulties = {}
    for p in all_problems:
        cat = p.get('category', 'Unknown')
        categories[cat] = categories.get(cat, 0) + 1
        d = p.get('real_difficulty', 0)
        difficulties[d] = difficulties.get(d, 0) + 1
    
    print(f"\n{'='*50}")
    print(f"MERGE COMPLETE")
    print(f"{'='*50}")
    print(f"New problems added: {new_count}")
    print(f"Total unique problems: {len(all_problems)}")
    print(f"ID range: {all_problems[0]['id']} - {all_problems[-1]['id']}")
    print(f"\n--- Category Distribution ---")
    for cat, cnt in sorted(categories.items(), key=lambda x: -x[1]):
        print(f"  {cat}: {cnt}")
    print(f"\n--- Difficulty Distribution ---")
    def diff_sort_key(x):
        try:
            return (0, int(x))
        except (ValueError, TypeError):
            return (1, str(x))
    for d in sorted(difficulties.keys(), key=diff_sort_key):
        bar = '#' * min(difficulties[d], 100)
        print(f"  {d}: {difficulties[d]:4d} {bar}")
    
    return len(all_problems)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'merge':
        merge_all()
    else:
        existing = load_existing_ids()
        remaining = sorted(set(range(1, 3001)) - existing)
        print(f"Existing: {len(existing)}")
        print(f"Remaining: {len(remaining)}")
        print(f"\nTo generate, create batch JSON files in: {BATCH_DIR}/")
        print(f"Then run: python {sys.argv[0]} merge")
