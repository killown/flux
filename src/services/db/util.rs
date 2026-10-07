pub fn rekey_path_prefix<V>(
    map: &mut std::collections::HashMap<String, V>,
    old: &str,
    new: &str,
) -> bool {
    let old = old.trim_end_matches('/');
    let new = new.trim_end_matches('/');
    if old.is_empty() || new.is_empty() || old == new {
        return false;
    }

    let affected: Vec<String> = map
        .keys()
        .filter(|k| k.as_str() == old || (k.starts_with(old) && k[old.len()..].starts_with('/')))
        .cloned()
        .collect();
    if affected.is_empty() {
        return false;
    }

    // Two phases so freshly inserted keys can never be matched a second time.
    let moved: Vec<(String, V)> = affected
        .into_iter()
        .filter_map(|k| {
            let v = map.remove(&k)?;
            Some((format!("{}{}", new, &k[old.len()..]), v))
        })
        .collect();
    for (k, v) in moved {
        map.insert(k, v);
    }
    true
}
