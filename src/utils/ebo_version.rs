pub fn ebo_version_ge(a: &str, b: &str) -> bool {
    use std::cmp::Ordering;

    let mut av: Vec<u64> = a.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect();
    let mut bv: Vec<u64> = b.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect();

    // Pad to equal length
    let len = av.len().max(bv.len());
    av.resize(len, 0);
    bv.resize(len, 0);

    for (x, y) in av.iter().zip(bv.iter()) {
        match x.cmp(y) {
            Ordering::Equal => continue,
            ord => return ord == Ordering::Greater,
        }
    }

    true // equal versions → true
}
