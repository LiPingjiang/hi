fn main() {
    // Rerun if the knowledge data changes
    println!("cargo:rerun-if-changed=data/leetcode_knowledge/problems_v2.json.gz");
}
