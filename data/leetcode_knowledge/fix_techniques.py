#!/usr/bin/env python3
"""
Fix the '暴力枚举' misclassification in problems_v2.json.gz.

Many problems were tagged as '暴力枚举' (brute force) simply because the
restructure script couldn't infer a technique from category/tags. This script
re-analyzes the approach text and code to assign proper techniques.
"""
import json
import gzip
import re
from collections import Counter

# Keywords in approach/code that indicate specific techniques
APPROACH_PATTERNS = [
    # (regex_pattern, technique, priority)
    (r'双指针|two.?pointer|对撞|快慢指针|左右指针', '双指针', 10),
    (r'滑动窗口|sliding.?window|窗口', '滑动窗口', 10),
    (r'二分查找|二分搜索|binary.?search|二分答案|二分法', '二分查找', 10),
    (r'动态规划|dp\[|dp\(|状态转移|子问题|最优子结构|记忆化', '动态规划', 10),
    (r'贪心|greedy|局部最优|贪心选择', '贪心', 8),
    (r'回溯|backtrack|剪枝|全排列|组合|子集生成', '回溯', 10),
    (r'BFS|广度优先|层序遍历|队列.*遍历|逐层', 'BFS', 10),
    (r'DFS|深度优先|递归遍历|先序|中序|后序', 'DFS', 10),
    (r'分治|divide.?and.?conquer|归并|merge.?sort', '分治', 10),
    (r'单调栈|monotonic.?stack|单调递[增减]栈', '单调栈', 10),
    (r'单调队列|monotonic.?queue|滑动窗口最[大小]', '单调队列', 10),
    (r'并查集|union.?find|连通分量|合并集合', '并查集', 10),
    (r'前缀和|prefix.?sum|累加和|区间和', '前缀和', 8),
    (r'差分|difference.?array', '差分', 8),
    (r'拓扑排序|topological|入度|出度.*排序', '拓扑排序', 10),
    (r'KMP|kmp|字符串匹配算法', 'KMP', 10),
    (r'Trie|trie|字典树|前缀树', '字典树', 10),
    (r'堆|heap|优先队列|priority.?queue|最[大小]堆', '堆', 8),
    (r'哈希|hash|散列|映射.*查找', '哈希表', 6),
    (r'栈|stack|后进先出|括号匹配', '栈', 6),
    (r'排序|sort|归并|快排|插入排序', '排序', 5),
    (r'模拟|simulate|按题意|逐步执行|直接模拟', '模拟', 4),
    (r'位运算|bit|异或|与运算|或运算|移位', '位运算', 7),
    (r'数学推导|数学公式|数论|质数|因数|GCD|取模', '数学推导', 6),
    (r'线段树|segment.?tree', '线段树', 10),
    (r'树状数组|binary.?indexed|fenwick', '树状数组', 10),
]

# Code patterns that indicate techniques
CODE_PATTERNS = [
    (r'while\s+left\s*<\s*right|while\s+l\s*<\s*r|left,\s*right\s*=', '双指针', 8),
    (r'while\s+.*end.*-.*start|window|while.*right.*<.*len', '滑动窗口', 7),
    (r'lo,\s*hi|left,\s*right.*mid|while\s+lo\s*<\s*hi|bisect', '二分查找', 8),
    (r'dp\s*=|dp\[|@lru_cache|@cache|memo\[', '动态规划', 9),
    (r'def\s+backtrack|def\s+dfs.*path|result\.append\(path', '回溯', 8),
    (r'queue|deque|bfs|level', 'BFS', 6),
    (r'def\s+dfs|stack.*while|递归', 'DFS', 6),
    (r'heapq|heappush|heappop|PriorityQueue', '堆', 9),
    (r'parent\[|find\(|union\(|rank\[', '并查集', 9),
    (r'prefix|presum', '前缀和', 7),
    (r'sorted\(|\.sort\(', '排序', 3),
]

# Title patterns
TITLE_PATTERNS = [
    (r'Two Sum|Three Sum|3Sum|4Sum', '双指针', 6),
    (r'Binary Search|Search.*Sorted|Find.*Peak', '二分查找', 7),
    (r'Longest.*Substring|Minimum.*Window|Sliding', '滑动窗口', 7),
    (r'Permutation|Combination|Subset|N-Queens', '回溯', 8),
    (r'Shortest Path|BFS|Level Order', 'BFS', 7),
    (r'DFS|Depth|Inorder|Preorder|Postorder', 'DFS', 7),
    (r'Maximum.*Subarray|Longest.*Subsequence|Knapsack|House Robber|Coin Change', '动态规划', 8),
    (r'Merge.*Sort|Sort.*Array|Kth.*Largest', '排序', 6),
    (r'Valid.*Parentheses|Calculator|Decode', '栈', 6),
    (r'LRU|LFU|Design.*Cache|Implement.*Queue|Implement.*Stack', '设计', 5),
    (r'Topological|Course Schedule', '拓扑排序', 9),
    (r'Union|Connected|Number.*Islands', '并查集', 6),
    (r'Trie|Prefix Tree|Word Search II', '字典树', 8),
]

# Specific well-known problem technique mappings (by approach keywords)
APPROACH_SPECIFIC = {
    '转置': '模拟',
    '翻转': '模拟',
    '头插法': '链表操作',
    '虚拟头节点': '链表操作',
    'dummy': '链表操作',
    '连环': '链表操作',
    '状态机': '模拟',
    '状态转移': '动态规划',
    '线性扫描': '模拟',
    '原地标记': '模拟',
    '递归': 'DFS',
    '分治递归': '分治',
    '归并': '分治',
    '三步法': '模拟',
    '反向遍历': '模拟',
    '逐字符': '模拟',
    '迭代': '模拟',
}


def reclassify_problem(problem):
    """Re-classify a problem that was tagged as brute-force-only."""
    techniques = set()
    approach = problem.get('approach', '')
    code = problem.get('code_python3', '')
    title = problem.get('title', '')
    topics = problem.get('topics', [])

    # Check approach text
    combined_text = approach + ' ' + code
    for pattern, tech, priority in APPROACH_PATTERNS:
        if re.search(pattern, combined_text, re.IGNORECASE):
            techniques.add(tech)

    # Check code patterns
    for pattern, tech, priority in CODE_PATTERNS:
        if re.search(pattern, code, re.IGNORECASE):
            techniques.add(tech)

    # Check title patterns
    for pattern, tech, priority in TITLE_PATTERNS:
        if re.search(pattern, title, re.IGNORECASE):
            techniques.add(tech)

    # Check specific approach keywords
    for keyword, tech in APPROACH_SPECIFIC.items():
        if keyword in approach:
            techniques.add(tech)

    # Topic-based inference for linked list problems
    if '链表' in topics and not techniques:
        techniques.add('链表操作')

    # If still nothing, check if it's truly simple/brute-force
    if not techniques:
        # Check if it's a simulation/implementation problem
        if any(kw in approach for kw in ['遍历', '扫描', '逐个', '依次', '按顺序']):
            techniques.add('模拟')
        elif '数组' in topics or '字符串' in topics:
            # Simple array/string problems are often simulation
            techniques.add('模拟')
        else:
            # Truly can't determine - keep as 暴力枚举 but this should be rare
            techniques.add('暴力枚举')

    # Remove '链表操作' and map to more standard categories
    if '链表操作' in techniques:
        techniques.discard('链表操作')
        techniques.add('模拟')  # Linked list manipulation is essentially simulation

    # Remove low-value duplicates
    if '排序' in techniques and len(techniques) > 1:
        # Sorting is often just a preprocessing step
        if any(t in techniques for t in ['双指针', '二分查找', '贪心', '动态规划']):
            techniques.discard('排序')

    if '哈希表' in techniques and len(techniques) > 1:
        # Hash table is often just a helper data structure
        if any(t in techniques for t in ['双指针', '滑动窗口', 'BFS', 'DFS', '动态规划']):
            techniques.discard('哈希表')

    return sorted(techniques)


def main():
    # Load data
    with gzip.open('problems_v2.json.gz', 'rt', encoding='utf-8') as f:
        problems = json.load(f)

    print(f"Total problems: {len(problems)}")

    # Find brute-force-only problems
    brute_only = [p for p in problems if p['techniques'] == ['暴力枚举']]
    print(f"Brute force only (before fix): {len(brute_only)}")

    # Reclassify
    changes = 0
    for p in problems:
        if p['techniques'] == ['暴力枚举']:
            new_techniques = reclassify_problem(p)
            if new_techniques != ['暴力枚举']:
                p['techniques'] = new_techniques
                changes += 1

    # Count remaining brute force
    remaining_brute = sum(1 for p in problems if '暴力枚举' in p['techniques'])
    print(f"Reclassified: {changes}")
    print(f"Remaining with brute force: {remaining_brute}")

    # Show new technique distribution
    print(f"\n--- New Technique Distribution ---")
    tech_counter = Counter()
    for p in problems:
        tech_counter.update(p['techniques'])
    for t, c in tech_counter.most_common():
        print(f"  {t}: {c}")

    # Write back
    with gzip.open('problems_v2.json.gz', 'wt', encoding='utf-8') as f:
        json.dump(problems, f, ensure_ascii=False)

    # Also write uncompressed for inspection
    with open('problems_v2.json', 'w', encoding='utf-8') as f:
        json.dump(problems, f, ensure_ascii=False, indent=2)

    print(f"\nDone! Updated problems_v2.json.gz")


if __name__ == '__main__':
    main()
