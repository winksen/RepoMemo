//! Matching a folder pattern against the path of a file (V0.2.179). The grammar is deliberately tiny and is never a
//! regular expression: `/` separates folders, `*` stands for any run of characters inside one name, and a name
//! that is `**` stands for any number of folders (none included). A pattern may start anywhere in the path, as a
//! `.gitignore` pattern does: `docs/adr/**` finds `docs/adr/0001.md` and `payments/docs/adr/0001.md`, and `README*`
//! finds the README of any folder. Case does not matter.

/// Whether `path` (folders and file name, separated by `/`) is matched by `pattern`.
///
/// The work is the number of folders in the pattern times the number of folders in the path, whatever the pattern says
/// (V0.2.192): a template is a file anyone may write, and a pattern of many `**` must not be able to stall a search.
pub fn path_matches(pattern: &str, path: &str) -> bool {
    let mut pattern: Vec<String> = pattern.split('/').map(str::to_lowercase).collect();
    // "May start anywhere in the path" is a `**` in front, and two `**` in a row are one.
    pattern.insert(0, "**".to_owned());
    pattern.dedup_by(|later, earlier| later == "**" && earlier == "**");
    let path: Vec<String> = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect();
    // `reach[j]`: the pattern's folders so far match exactly the first `j` folders of the path.
    let mut reach = vec![false; path.len() + 1];
    reach[0] = true;
    for segment in &pattern {
        let mut next = vec![false; path.len() + 1];
        if segment == "**" {
            let mut open = false;
            for (j, slot) in next.iter_mut().enumerate() {
                open |= reach[j];
                *slot = open;
            }
        } else {
            for j in 1..=path.len() {
                next[j] = reach[j - 1] && name_matches(segment, &path[j - 1]);
            }
        }
        reach = next;
    }
    reach[path.len()]
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

    /// V0.2.192: many `**` used to take time that grew with every one of them; now the work is the pattern times the path.
    #[test]
    fn a_pattern_of_many_double_stars_cannot_stall_a_search() {
        let long_path = (0..80).map(|n| format!("folder{n}")).collect::<Vec<_>>().join("/");
        let stacked = format!("{}x.md", "**/".repeat(11));
        assert!(!path_matches(&stacked, &format!("{long_path}/y.md")));
        assert!(path_matches(&stacked, &format!("{long_path}/x.md")));
        assert!(!path_matches("**/a/**/b/**/c/**/d/**/e/**/f/**/z", &long_path));
        assert!(path_matches("**/folder3/**/folder40/**/folder79", &long_path));
        // Several stars in a name are bounded by the length of the name.
        assert!(!path_matches("*a*a*a*a*a*a*a*a*a*b", &"a".repeat(200)));
        // Two `**` in a row mean one, and a leading one changes nothing.
        assert_eq!(
            path_matches("**/**/docs/**", "x/docs/y.md"),
            path_matches("docs/**", "x/docs/y.md")
        );
        assert!(path_matches("**/**", "a/b"));
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
