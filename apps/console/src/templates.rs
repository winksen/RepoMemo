//! `templates validate <file>`: checks a workspace template file on this machine with the rules the server applies
//! to the files of `REPOMEMO_TEMPLATES_DIR` (V0.2.190).
//!
//! It is the one console command that does not go to the server: the file is on the machine the console runs on,
//! and the answer should not need a server, a sign-in or a restart. The check is the server's own: the same
//! `repomemo-templates` code reads the file, so the messages are the ones the server would put in System › Overview.

use std::path::Path;

use repomemo_templates::{read_template_file, Catalogue, SourceProblem, TemplateDefinition};

/// The path of a `templates validate <file>` line, or `None` for any other line. The rest of the line is the path, so a
/// path with spaces works with or without quotes.
pub fn validate_request(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line
        .get(.."templates".len())
        .filter(|word| word.eq_ignore_ascii_case("templates"))?;
    let rest = line[rest.len()..].trim_start();
    let after = rest
        .get(.."validate".len())
        .filter(|word| word.eq_ignore_ascii_case("validate"))?;
    let path = &rest[after.len()..];
    if !path.is_empty() && !path.starts_with(char::is_whitespace) {
        return None;
    }
    let path = path.trim();
    let unquoted = ['"', '\'']
        .iter()
        .find_map(|quote| path.strip_prefix(*quote).and_then(|inner| inner.strip_suffix(*quote)))
        .unwrap_or(path);
    Some(unquoted.to_owned())
}

fn lines(problems: &[SourceProblem]) -> Vec<String> {
    problems
        .iter()
        .flat_map(|problem| {
            problem
                .issues
                .iter()
                .map(|issue| format!("{}: {issue}", problem.source))
        })
        .collect()
}

/// What the server would make of a file called `name` with this `text`: a line saying so when it is fine, or each
/// problem, with the place in the template and in the file. A name that starts with `_` is a file the server leaves
/// alone, so only its content is checked.
pub fn check(name: &str, text: &str) -> Result<String, Vec<String>> {
    if name.starts_with('_') {
        return match TemplateDefinition::parse(text) {
            Ok(template) => Ok(format!(
                "{name}: a valid template ({}). The server leaves files whose name starts with _ alone, so it would not be loaded until it is renamed {}.json.",
                template.id(),
                template.id()
            )),
            Err(issues) => Err(issues.iter().map(|issue| format!("{name}: {issue}")).collect()),
        };
    }
    let builtin = Catalogue::builtin();
    let (extended, problems) = builtin.extended([(name, text)]);
    if !problems.is_empty() {
        return Err(lines(&problems));
    }
    let added = extended
        .list()
        .iter()
        .find(|template| extended.is_custom(template.id()))
        .ok_or_else(|| vec![format!("{name}: the file added no template")])?;
    Ok(format!(
        "{name}: valid. It would be added as the custom template {} \"{}\" (version {}).",
        added.id(),
        added.meta.name,
        added.meta.version
    ))
}

/// Reads the file at `path` the way the server reads its folder (a link is not followed, at most 64 KiB, UTF-8) and
/// checks it.
pub fn validate_file(path: &str) -> Result<String, Vec<String>> {
    if path.is_empty() {
        return Err(vec![
            "Usage: templates validate <file>. Give the template file to check.".to_owned(),
        ]);
    }
    let read = read_template_file(Path::new(path));
    if !read.problems.is_empty() {
        return Err(lines(&read.problems));
    }
    let (name, text) = read
        .sources
        .first()
        .ok_or_else(|| vec![format!("{path}: nothing was read")])?;
    check(name, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
  "format": 1,
  "meta": { "id": "support", "version": 1, "name": "Customer support", "summary": "Playbooks and macros.", "icon": "messages" },
  "sections": { "order": ["overview", "review"] }
}"#;

    #[test]
    fn only_a_templates_validate_line_is_taken() {
        assert_eq!(
            validate_request("templates validate support.json"),
            Some("support.json".to_owned())
        );
        assert_eq!(
            validate_request("  Templates   VALIDATE  my templates/a b.json "),
            Some("my templates/a b.json".to_owned())
        );
        assert_eq!(
            validate_request(r#"templates validate "a b.json""#),
            Some("a b.json".to_owned())
        );
        assert_eq!(
            validate_request("templates validate 'a b.json'"),
            Some("a b.json".to_owned())
        );
        assert_eq!(validate_request("templates validate"), Some(String::new()));
        assert_eq!(validate_request("templates list"), None);
        assert_eq!(validate_request("templates validated x"), None);
        assert_eq!(validate_request("template validate x"), None);
        assert_eq!(validate_request("status"), None);
        assert_eq!(validate_request(""), None);
    }

    #[test]
    fn a_valid_file_would_be_added_as_a_custom_template() {
        let said = check("support.json", VALID).unwrap();
        assert!(
            said.contains("custom template support \"Customer support\" (version 1)"),
            "{said}"
        );
    }

    #[test]
    fn a_file_is_refused_with_the_messages_the_server_gives_it() {
        let misspelled = VALID.replace("\"review\"", "\"revue\"");
        let said = check("support.json", &misspelled).unwrap_err();
        assert_eq!(said.len(), 1);
        assert!(
            said[0].starts_with("support.json: sections.order[1]: `revue` is not a workspace section"),
            "{said:?}"
        );
        // The same file is refused for the reasons the server names: another name, a built-in id, an unknown key.
        let wrong_name = check("other.json", VALID).unwrap_err();
        assert!(wrong_name[0].contains("named after its id"), "{wrong_name:?}");
        let built_in = VALID.replace("support", "operations");
        let shadow = check("operations.json", &built_in).unwrap_err();
        assert!(shadow[0].contains("built into this server"), "{shadow:?}");
        let odd = VALID.replace("\"format\": 1,", "\"format\": 1, \"colour\": \"red\",");
        let unknown = check("support.json", &odd).unwrap_err();
        assert!(unknown[0].contains("colour"), "{unknown:?}");
        let broken = check("support.json", "{ \"format\": ").unwrap_err();
        assert!(broken[0].contains("line"), "{broken:?}");
    }

    #[test]
    fn a_file_the_server_leaves_alone_is_checked_for_its_content_and_said_so() {
        let said = check("_support.json", VALID).unwrap();
        assert!(said.contains("leaves files whose name starts with _ alone"), "{said}");
        assert!(check("_support.json", "{}").is_err());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_a_problem_named_after_it() {
        let missing = std::env::temp_dir().join("repomemo-console-no-such-template.json");
        let _ = std::fs::remove_file(&missing);
        let said = validate_file(&missing.to_string_lossy()).unwrap_err();
        assert_eq!(said.len(), 1);
        assert!(said[0].contains("cannot be read"), "{said:?}");
        assert_eq!(validate_file("").unwrap_err().len(), 1);
    }
}
