//! Matching a folder pattern against the path of a file (V0.2.179). The grammar is deliberately tiny and is never a
//! regular expression: `/` separates folders, `*` stands for any run of characters inside one name, and a name
//! that is `**` stands for any number of folders (none included). A pattern may start anywhere in the path, as a
//! `.gitignore` pattern does: `docs/adr/**` finds `docs/adr/0001.md` and `payments/docs/adr/0001.md`, and `README*`
//! finds the README of any folder. Case does not matter.

/// Whether `path` (folders and file name, separated by `/`) is matched by `pattern`.
pub fn path_matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<String> = pattern.split('/').map(str::to_lowercase).collect();
    let pattern: Vec<&str> = pattern.iter().map(String::as_str).collect();
    let path: Vec<String> = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect();
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    (0..=path.len()).any(|start| segments_match(&pattern, &path[start..]))
}

fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments_match(rest, &path[skip..])),
        Some((first, rest)) => match path.split_first() {
            Some((name, tail)) => name_matches(first, name) && segments_match(rest, tail),
            None => false,
        },
    }
}

/// One name against one pattern segment, where `*` is any run of characters.
fn name_matches(pattern: &str, name: &str) -> bool {
    let (pattern, name): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut p, mut n) = (0, 0);
    let (mut star, mut resume) = (None, 0);
    while n < name.len() {
        if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            resume = n;
            p += 1;
        } else if p < pattern.len() && pattern[p] == name[n] {
            p += 1;
            n += 1;
        } else if let Some(at) = star {
            p = at + 1;
            resume += 1;
            n = resume;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::path_matches;

    #[test]
    fn a_folder_pattern_finds_what_is_below_it_wherever_it_starts() {
        assert!(path_matches("docs/adr/**", "docs/adr/0001-use-rust.md"));
        assert!(path_matches("docs/adr/**", "payments/docs/adr/0001.md"));
        assert!(path_matches("docs/adr/**", "docs/adr/2026/10/0002.md"));
        assert!(!path_matches("docs/adr/**", "docs/guides/adr.md"));
        assert!(!path_matches("docs/adr/**", "src/docs.rs"));
        assert!(path_matches("runbooks/**", "runbooks/payments/restart.md"));
    }

    #[test]
    fn a_name_pattern_finds_the_file_of_any_folder() {
        assert!(path_matches("README*", "README.md"));
        assert!(path_matches("README*", "services/payments/readme.md"));
        assert!(path_matches("*.lock", "Cargo.lock"));
        assert!(path_matches("*.lock", "web/yarn.lock"));
        assert!(!path_matches("README*", "docs/README/index.md"));
        assert!(!path_matches("*.lock", "locksmith.md"));
        assert!(path_matches("a*b*c", "axxbyyc"));
        assert!(!path_matches("a*b*c", "axxbyy"));
    }

    #[test]
    fn a_pattern_with_folders_has_to_match_to_the_end_of_the_path() {
        assert!(path_matches("src/*.rs", "crates/api/src/lib.rs"));
        assert!(!path_matches("src/*.rs", "crates/api/src/deep/lib.rs"));
        assert!(path_matches("src/**/*.rs", "crates/api/src/deep/er/lib.rs"));
        assert!(path_matches("**/*.md", "a/b/c.md"));
        assert!(path_matches("**", "anything/at/all"));
    }

    #[test]
    fn case_and_the_kind_of_slash_do_not_matter_and_nothing_is_a_regular_expression() {
        assert!(path_matches("DOCS/ADR/**", "docs/adr/x.md"));
        assert!(path_matches("docs/adr/**", "docs\\adr\\x.md"));
        assert!(!path_matches("a.b", "aXb"), "a dot is a dot");
        assert!(!path_matches("(a|b).md", "a.md"));
        assert!(path_matches("a+b.md", "a+b.md"));
        assert!(!path_matches("docs/adr", ""), "an empty path matches no folder pattern");
        assert!(path_matches("**", ""));
    }
}
