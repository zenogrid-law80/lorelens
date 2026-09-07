use super::*;

pub(super) struct IgnoreRule {
    matcher: globset::GlobMatcher,
    include: bool,
    directory: bool,
    filename: bool,
}

pub(super) fn load_loreignore(root: &Path) -> Result<Vec<IgnoreRule>, String> {
    let text = match fs::read_to_string(root.join(".loreignore")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!(".loreignore: {error}")),
    };
    let mut rules = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let mut pattern = line.trim();
        if pattern.is_empty() || pattern.starts_with('#') {
            continue;
        }
        let mut include = false;
        while let Some(rest) = pattern.strip_prefix('!') {
            include = !include;
            pattern = rest;
        }
        if let Some(rest) = pattern.strip_prefix("\\!") {
            pattern = rest;
        }
        let anchored = pattern.starts_with('/');
        let directory = pattern.ends_with('/');
        pattern = pattern.trim_matches('/');
        if !anchored
            && let Some(rest) = pattern.strip_prefix("**/")
            && !rest.contains('/')
        {
            pattern = rest;
        }
        let filename = !anchored && !pattern.contains('/') && pattern != "**";
        let mut patterns = vec![pattern.to_owned()];
        if include && !filename && !pattern.ends_with('*') {
            patterns.push(format!("{pattern}/**"));
        }
        for (companion, pattern) in patterns.into_iter().enumerate() {
            let matcher = globset::GlobBuilder::new(&pattern)
                .case_insensitive(true)
                .literal_separator(true)
                .backslash_escape(true)
                .build()
                .map_err(|e| format!(".loreignore line {}: {e}", index + 1))?
                .compile_matcher();
            rules.push(IgnoreRule {
                matcher,
                include,
                directory: directory && companion == 0,
                filename,
            });
        }
    }
    Ok(rules)
}

pub(super) fn loreignored(rules: &[IgnoreRule], path: &str, directory: bool) -> bool {
    let mut excluded = false;
    let mut decided_at = 0;
    let mut ends: Vec<_> = path.match_indices('/').map(|(index, _)| index).collect();
    ends.push(path.len());
    for end in ends {
        let prefix = &path[..end];
        let is_dir = end < path.len() || directory;
        let parent_excluded = excluded;
        for (index, rule) in rules.iter().enumerate().skip(decided_at) {
            if rule.directory && !is_dir {
                continue;
            }
            if parent_excluded && rule.include && rule.filename {
                continue;
            }
            let candidate = if rule.filename {
                prefix.rsplit('/').next().unwrap_or(prefix)
            } else {
                prefix
            };
            if rule.matcher.is_match(candidate) {
                excluded = !rule.include;
                decided_at = index;
            }
        }
    }
    excluded
}
