#!/usr/bin/env python3
"""
Restructure problems_all.json into v2 format:
- Split category into topics (data structure/scenario) and techniques (algorithm/strategy)
- Normalize tags into consistent Chinese naming
- Add patterns field for specific code patterns
"""
import json
import re
from collections import Counter

# ═══════════════════════════════════════════════════════════════════════════════
# Taxonomy definitions
# ═══════════════════════════════════════════════════════════════════════════════

TOPICS = [
    "数组", "字符串", "链表", "树", "图",
    "矩阵", "区间", "数学", "设计", "数据库",
    "位操作", "几何"
]

TECHNIQUES = [
    "暴力枚举", "双指针", "滑动窗口", "二分查找",
    "排序", "贪心", "动态规划", "回溯",
    "BFS", "DFS", "分治", "单调栈",
    "单调队列", "并查集", "前缀和", "差分",
    "拓扑排序", "数学推导", "哈希表", "堆",
    "栈", "模拟", "位运算"
]

# Mapping from old category/tags to new topics
TOPIC_MAP = {
    # Direct category mappings
    "数组": "数组", "链表": "链表", "树": "树", "图": "图",
    "数学": "数学", "设计": "设计", "数据库": "数据库",
    "字符串": "字符串",
    # Tag-based mappings
    "array": "数组", "Array": "数组",
    "linked-list": "链表", "链表": "链表",
    "tree": "树", "Tree": "树", "binary-tree": "树",
    "graph": "图", "Graph": "图",
    "matrix": "矩阵", "矩阵": "矩阵",
    "string": "字符串", "String": "字符串",
    "math": "数学", "Math": "数学",
    "design": "设计", "Design": "设计",
    "SQL": "数据库", "sql": "数据库", "database": "数据库",
    "geometry": "几何", "几何": "几何",
    "bit": "位操作", "位运算": "位操作", "bit-manipulation": "位操作",
    "interval": "区间", "区间": "区间",
}

# Mapping from old category/tags to new techniques
TECHNIQUE_MAP = {
    # Direct category mappings
    "动态规划": "动态规划", "贪心": "贪心", "二分查找": "二分查找",
    "双指针": "双指针", "回溯": "回溯", "滑动窗口": "滑动窗口",
    "排序": "排序", "分治": "分治", "前缀和": "前缀和",
    "单调栈": "单调栈", "单调队列": "单调队列", "堆": "堆",
    "栈": "栈", "哈希表": "哈希表", "位运算": "位运算",
    # Tag-based mappings
    "dynamic-programming": "动态规划", "dp": "动态规划", "DP": "动态规划",
    "greedy": "贪心", "Greedy": "贪心",
    "binary-search": "二分查找", "Binary Search": "二分查找",
    "two-pointers": "双指针", "two-pointer": "双指针",
    "backtracking": "回溯", "Backtracking": "回溯",
    "sliding-window": "滑动窗口",
    "sorting": "排序", "Sorting": "排序", "sort": "排序",
    "divide-and-conquer": "分治",
    "prefix-sum": "前缀和", "前缀和": "前缀和",
    "monotonic-stack": "单调栈", "monotone-stack": "单调栈",
    "monotonic-queue": "单调队列",
    "union-find": "并查集", "并查集": "并查集", "UnionFind": "并查集",
    "topological-sort": "拓扑排序", "拓扑排序": "拓扑排序",
    "bfs": "BFS", "BFS": "BFS", "breadth-first-search": "BFS",
    "dfs": "DFS", "DFS": "DFS", "depth-first-search": "DFS",
    "heap": "堆", "priority-queue": "堆", "Heap": "堆",
    "stack": "栈", "Stack": "栈",
    "hash-table": "哈希表", "hash-map": "哈希表", "HashMap": "哈希表",
    "simulation": "模拟", "模拟": "模拟",
    "bit-manipulation": "位运算", "Bit Manipulation": "位运算",
    "math": "数学推导", "数学": "数学推导",
}

# Subcategory to pattern mapping (common patterns)
PATTERN_KEYWORDS = {
    "快慢指针": "快慢指针", "左右指针": "左右指针", "对撞指针": "左右指针",
    "背包": "背包DP", "01背包": "背包DP", "完全背包": "背包DP",
    "区间DP": "区间DP", "区间dp": "区间DP",
    "状态压缩": "状态压缩DP", "状压": "状态压缩DP",
    "树形DP": "树形DP", "树形dp": "树形DP",
    "数位DP": "数位DP", "数位dp": "数位DP",
    "二分答案": "二分答案",
    "单调栈": "单调栈", "单调队列": "单调队列",
    "拓扑排序": "拓扑排序",
    "最短路": "最短路径", "Dijkstra": "最短路径", "dijkstra": "最短路径",
    "最小生成树": "最小生成树", "Kruskal": "最小生成树", "Prim": "最小生成树",
    "KMP": "KMP匹配", "kmp": "KMP匹配",
    "Trie": "字典树", "trie": "字典树", "字典树": "字典树",
    "线段树": "线段树", "segment-tree": "线段树",
    "树状数组": "树状数组", "BIT": "树状数组",
    "滚动数组": "滚动数组优化",
    "记忆化搜索": "记忆化搜索", "记忆化": "记忆化搜索",
    "贪心": "贪心选择",
    "窗口函数": "SQL窗口函数",
    "自连接": "SQL自连接",
    "子查询": "SQL子查询",
}


def infer_topics(problem):
    """Infer topics from category and tags."""
    topics = set()
    cat = problem.get("category", "")
    tags = problem.get("tags", [])

    # From category
    if cat in TOPIC_MAP:
        topics.add(TOPIC_MAP[cat])

    # From tags
    for tag in tags:
        if tag in TOPIC_MAP:
            topics.add(TOPIC_MAP[tag])

    # Fallback: if category is actually a technique, infer topic from context
    if not topics:
        # Default based on common patterns
        code = problem.get("code_python3", "")
        title = problem.get("title", "").lower()
        if "tree" in title or "TreeNode" in code:
            topics.add("树")
        elif "list" in title or "ListNode" in code:
            topics.add("链表")
        elif "graph" in title:
            topics.add("图")
        elif "SELECT" in code or "FROM" in code:
            topics.add("数据库")
        else:
            topics.add("数组")  # Most common default

    return sorted(topics)


def infer_techniques(problem):
    """Infer techniques from category, subcategory, and tags."""
    techniques = set()
    cat = problem.get("category", "")
    sub = problem.get("subcategory", "")
    tags = problem.get("tags", [])
    approach = problem.get("approach", "")

    # From category (many categories ARE techniques)
    if cat in TECHNIQUE_MAP:
        techniques.add(TECHNIQUE_MAP[cat])

    # From tags
    for tag in tags:
        if tag in TECHNIQUE_MAP:
            techniques.add(TECHNIQUE_MAP[tag])

    # From subcategory keywords
    sub_lower = sub.lower()
    if "dp" in sub_lower or "动态规划" in sub or "动规" in sub:
        techniques.add("动态规划")
    if "贪心" in sub:
        techniques.add("贪心")
    if "二分" in sub:
        techniques.add("二分查找")
    if "双指针" in sub or "对撞" in sub:
        techniques.add("双指针")
    if "滑动窗口" in sub:
        techniques.add("滑动窗口")
    if "回溯" in sub or "dfs" in sub_lower:
        techniques.add("回溯")
    if "bfs" in sub_lower or "广度" in sub:
        techniques.add("BFS")
    if "排序" in sub and "拓扑" not in sub:
        techniques.add("排序")
    if "前缀和" in sub or "差分" in sub:
        techniques.add("前缀和")
    if "并查集" in sub or "union" in sub_lower:
        techniques.add("并查集")
    if "拓扑" in sub:
        techniques.add("拓扑排序")
    if "单调栈" in sub:
        techniques.add("单调栈")
    if "单调队列" in sub:
        techniques.add("单调队列")
    if "模拟" in sub:
        techniques.add("模拟")
    if "哈希" in sub:
        techniques.add("哈希表")
    if "堆" in sub or "优先队列" in sub:
        techniques.add("堆")

    # From approach text
    if "动态规划" in approach or "dp[" in approach.lower():
        techniques.add("动态规划")
    if "二分" in approach:
        techniques.add("二分查找")

    # Ensure at least one technique
    if not techniques:
        techniques.add("暴力枚举")

    # Remove "数学推导" if it came from math tag but problem is clearly algorithmic
    if "数学推导" in techniques and len(techniques) > 1:
        # Keep it only if it's the primary technique
        pass

    return sorted(techniques)


def infer_patterns(problem):
    """Infer specific patterns from subcategory and approach."""
    patterns = set()
    sub = problem.get("subcategory", "")
    approach = problem.get("approach", "")
    combined = sub + " " + approach

    for keyword, pattern in PATTERN_KEYWORDS.items():
        if keyword in combined:
            patterns.add(pattern)

    return sorted(patterns)


def restructure():
    with open("problems_all.json", "r", encoding="utf-8") as f:
        problems = json.load(f)

    v2_problems = []
    for p in problems:
        topics = infer_topics(p)
        techniques = infer_techniques(p)
        patterns = infer_patterns(p)

        # Ensure real_difficulty is int 1-10
        rd = p.get("real_difficulty", 5)
        if isinstance(rd, str):
            rd = {"Easy": 2, "easy": 2, "Medium": 5, "medium": 5, "Hard": 7, "hard": 7}.get(rd, 5)
        rd = max(1, min(10, int(rd)))

        v2 = {
            "id": p["id"],
            "title": p["title"],
            "slug": p["slug"],
            "topics": topics,
            "techniques": techniques,
            "patterns": patterns,
            "difficulty": rd,
            "official_difficulty": p.get("official_difficulty", "Medium"),
            "key_insight": p.get("key_insight", ""),
            "approach": p.get("approach", ""),
            "time_complexity": p.get("time_complexity", ""),
            "space_complexity": p.get("space_complexity", ""),
            "code_python3": p.get("code_python3", ""),
        }
        v2_problems.append(v2)

    # Sort by id
    v2_problems.sort(key=lambda x: x["id"])

    # Write v2
    with open("problems_v2.json", "w", encoding="utf-8") as f:
        json.dump(v2_problems, f, ensure_ascii=False, indent=2)

    # Stats
    print(f"Total problems: {len(v2_problems)}")
    print(f"\n--- Topics Distribution ---")
    topic_counter = Counter()
    for p in v2_problems:
        topic_counter.update(p["topics"])
    for t, c in topic_counter.most_common():
        print(f"  {t}: {c}")

    print(f"\n--- Techniques Distribution ---")
    tech_counter = Counter()
    for p in v2_problems:
        tech_counter.update(p["techniques"])
    for t, c in tech_counter.most_common():
        print(f"  {t}: {c}")

    print(f"\n--- Multi-technique problems ---")
    multi = sum(1 for p in v2_problems if len(p["techniques"]) > 1)
    print(f"  {multi}/{len(v2_problems)} ({100*multi//len(v2_problems)}%) have multiple techniques")

    print(f"\n--- Patterns (top 20) ---")
    pat_counter = Counter()
    for p in v2_problems:
        pat_counter.update(p["patterns"])
    for t, c in pat_counter.most_common(20):
        print(f"  {t}: {c}")


if __name__ == "__main__":
    restructure()
