//! `CODEOWNERS` as ownership hints (roadmap F16c, V0.2.25).
//!
//! A repository's `CODEOWNERS` file says who owns which paths. RepoMemo shows
//! the matching owners on each file as a hint next to its review owner: the
//! two are different things (an owner here is a handle on the host, not a
//! person of the workspace), and nothing is ever assigned from it.
//!
//! The matcher follows what GitHub and GitLab document, without regular
//! expressions: patterns are read top to bottom and the last one that matches
//! a path wins; `*` and `?` stay inside one path segment and `**` crosses
//! them; a pattern with a `/` in front or in the middle is anchored to the
//! root, one without matches at any depth; a pattern that ends in `/`, or in a
//! name with no wildcard, covers everything inside the directory it names. A
//! pattern with no owners clears ownership. GitLab's `[Section]` lines (and
//! their default owners) are understood; negation and escapes are not.

/// Where the file is looked for, first one found wins.
pub const CODEOWNERS_PATHS: &[&str] = &[".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS", ".gitlab/CODEOWNERS"];

/// The most owners kept for one file.
const MAX_OWNERS: usize = 16;
/// The largest `CODEOWNERS` read (GitHub's own limit is 3 MiB).
pub const MAX_CODEOWNERS_BYTES: u64 = 3 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    segments: Vec<String>,
    /// Matches at the root only, not at any depth.
    anchored: bool,
    /// Ends in `/`: only directories, with everything inside.
    directory: bool,
    owners: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeOwners {
    rules: Vec<Rule>,
}

impl CodeOwners {
    pub fn parse(text: &str) -> Self {
        let mut rules = Vec::new();
        let mut section_default: Vec<String> = Vec::new();
        for line in text.lines() {
            let line = line.split(" #").next().unwrap_or(line).trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // GitLab sections: `[Name]`, `^[Optional]`, `[Name][2] @default`.
            let header = line.strip_prefix('^').unwrap_or(line);
            if header.starts_with('[') {
                if let Some(end) = header.find(']') {
                    let mut rest = &header[end + 1..];
                    if rest.starts_with('[') {
                        rest = rest.find(']').map_or("", |at| &rest[at + 1..]);
                    }
                    section_default = owners_of(rest);
                    continue;
                }
            }
            let mut parts = line.split_whitespace();
            let Some(pattern) = parts.next() else {
                continue;
            };
            let mut owners = owners_of(&parts.collect::<Vec<_>>().join(" "));
            if owners.is_empty() {
                owners = section_default.clone();
            }
            if let Some(rule) = rule_of(pattern, owners) {
                rules.push(rule);
            }
        }
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The owners of a path (with `/` separators, no leading slash): those of
    /// the last rule that matches it.
    pub fn owners_of(&self, path: &str) -> Vec<String> {
        let segments = path.split('/').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
        self.rules
            .iter()
            .rev()
            .find(|rule| rule_matches(rule, &segments))
            .map(|rule| rule.owners.iter().take(MAX_OWNERS).cloned().collect())
            .unwrap_or_default()
    }
}

fn owners_of(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|owner| owner.starts_with('@') || owner.contains('@'))
        .map(str::to_owned)
        .collect()
}

fn rule_of(pattern: &str, owners: Vec<String>) -> Option<Rule> {
    let directory = pattern.ends_with('/');
    let trimmed = pattern.trim_matches('/');
    if trimmed.is_empty() {
        // "/" alone means everything.
        return Some(Rule {
            segments: vec!["**".to_owned()],
            anchored: true,
            directory: false,
            owners,
        });
    }
    let anchored = pattern.starts_with('/') || trimmed.contains('/');
    Some(Rule {
        segments: trimmed.split('/').map(str::to_owned).collect(),
        anchored,
        directory,
        owners,
    })
}

fn has_wildcard(segment: &str) -> bool {
    segment.contains('*') || segment.contains('?')
}

fn rule_matches(rule: &Rule, path: &[&str]) -> bool {
    if path.is_empty() {
        return false;
    }
    // Where a match may start: the root for an anchored rule, any directory
    // level otherwise.
    let starts: Vec<usize> = if rule.anchored { vec![0] } else { (0..path.len()).collect() };
    let last_is_name = rule.segments.last().is_some_and(|segment| !has_wildcard(segment));
    // Whether what a pattern names may be a directory whose content counts.
    let covers_directory = rule.directory || last_is_name || rule.segments.last().is_some_and(|s| s == "**");
    for start in starts {
        let tail = &path[start..];
        if let Some(consumed) = match_segments(&rule.segments, tail) {
            if consumed == tail.len() {
                // The whole of what is left was named; a directory-only
                // pattern does not name a file.
                if !rule.directory {
                    return true;
                }
                continue;
            }
            if covers_directory {
                return true;
            }
        }
    }
    false
}

/// How many leading path segments the pattern segments consume, if they match
/// from the start (`**` takes any number, including none).
fn match_segments(pattern: &[String], path: &[&str]) -> Option<usize> {
    match pattern.split_first() {
        None => Some(0),
        Some((first, rest)) if first == "**" => {
            // The shortest consumption that lets the rest match, so a
            // trailing `**` still takes the remainder.
            if rest.is_empty() {
                return Some(path.len());
            }
            (0..=path.len()).find_map(|skip| match_segments(rest, &path[skip..]).map(|consumed| skip + consumed))
        }
        Some((first, rest)) => {
            let (name, remaining) = path.split_first()?;
            if !wildcard_match(first.as_bytes(), name.as_bytes()) {
                return None;
            }
            match_segments(rest, remaining).map(|consumed| consumed + 1)
        }
    }
}

/// `*` (any run of characters) and `?` (one character) inside one segment.
fn wildcard_match(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(star_at) = star {
            p = star_at + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners(file: &str, path: &str) -> Vec<String> {
        CodeOwners::parse(file).owners_of(path)
    }

    #[test]
    fn the_last_matching_rule_wins_and_patterns_follow_the_documented_forms() {
        let file = "\
# everything\n\
*       @everyone\n\
*.js    @js-owner\n\
/build/logs/  @ops\n\
docs/*  docs@example.com\n\
/src/**/generated/ @robots\n\
apps/   @app-team @lead\n\
/README.md @writer # trailing comment\n";
        assert_eq!(owners(file, "main.py"), ["@everyone"]);
        assert_eq!(owners(file, "web/app.js"), ["@js-owner"]);
        assert_eq!(owners(file, "build/logs/2026/run.txt"), ["@ops"]);
        assert_eq!(owners(file, "other/build/logs/run.txt"), ["@everyone"], "anchored to the root");
        assert_eq!(owners(file, "docs/intro.md"), ["docs@example.com"]);
        assert_eq!(owners(file, "docs/deep/intro.md"), ["@everyone"], "`docs/*` does not recurse");
        assert_eq!(owners(file, "src/a/b/generated/x.rs"), ["@robots"]);
        assert_eq!(owners(file, "src/generated/x.rs"), ["@robots"], "`**` takes no segment too");
        assert_eq!(owners(file, "apps/web/x.ts"), ["@app-team", "@lead"]);
        assert_eq!(owners(file, "more/apps/x.ts"), ["@app-team", "@lead"], "a bare directory name matches at any depth");
        assert_eq!(owners(file, "README.md"), ["@writer"]);
        assert_eq!(owners(file, "sub/README.md"), ["@everyone"]);
    }

    #[test]
    fn a_rule_without_owners_clears_and_sections_bring_defaults() {
        let file = "*  @all\n/vendor/  \n[Docs] @docs-team\n*.md\n[Security]\n/auth/ @sec\n";
        assert_eq!(owners(file, "vendor/lib.c"), Vec::<String>::new());
        assert_eq!(owners(file, "guide.md"), ["@docs-team"]);
        assert_eq!(owners(file, "auth/login.rs"), ["@sec"]);
        assert_eq!(owners(file, "lib.rs"), ["@all"]);
        assert_eq!(owners("# only comments\n", "a.rs"), Vec::<String>::new());
        assert!(CodeOwners::parse("").is_empty());
    }

    #[test]
    fn wildcards_stay_inside_a_segment() {
        assert!(wildcard_match(b"*.rs", b"main.rs"));
        assert!(wildcard_match(b"te?t*", b"test.txt"));
        assert!(!wildcard_match(b"*.rs", b"main.rs.bak"));
        assert!(wildcard_match(b"*", b"anything"));
        assert!(!wildcard_match(b"a?", b"a"));
    }
}
