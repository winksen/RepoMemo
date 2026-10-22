use crate::{general, limits, TemplateDefinition, TemplateIssue, FORMAT, GENERAL_ID, WORKSPACE_ICONS};

const VALID: &str = r#"{
  "format": 1,
  "meta": {
    "id": "operations",
    "version": 2,
    "name": "Operations and incidents",
    "summary": "Runbooks, incidents and on-call knowledge.",
    "icon": "heartbeat"
  }
}"#;

fn parse(text: &str) -> Result<TemplateDefinition, Vec<TemplateIssue>> {
    TemplateDefinition::parse(text)
}

/// The valid file with one value of `meta` replaced; `value` is written as JSON.
fn with_meta(key: &str, value: &str) -> String {
    let mut fields = [
        ("id", "\"operations\""),
        ("version", "2"),
        ("name", "\"Operations and incidents\""),
        ("summary", "\"Runbooks, incidents and on-call knowledge.\""),
        ("icon", "\"heartbeat\""),
    ];
    for field in &mut fields {
        if field.0 == key {
            field.1 = value;
        }
    }
    let body = fields
        .iter()
        .map(|(key, value)| format!("    \"{key}\": {value}"))
        .collect::<Vec<_>>()
        .join(
            ",
",
        );
    format!(
        "{{
  \"format\": 1,
  \"meta\": {{
{body}
  }}
}}"
    )
}

fn paths(result: Result<TemplateDefinition, Vec<TemplateIssue>>) -> Vec<String> {
    result
        .expect_err("the file should be refused")
        .into_iter()
        .map(|issue| issue.path)
        .collect()
}

#[test]
fn a_valid_file_is_read() {
    let template = parse(VALID).unwrap();
    assert_eq!(template.id(), "operations");
    assert_eq!(template.meta.version, 2);
    assert_eq!(template.meta.icon, "heartbeat");
    assert!(template.validate().is_empty());
}

#[test]
fn general_is_valid_and_defined_without_a_file() {
    let template = general();
    assert_eq!(template.id(), GENERAL_ID);
    assert_eq!(template.format, FORMAT);
    assert!(template.validate().is_empty(), "{:?}", template.validate());
    // Written out and read back, it is the same template.
    let text = serde_json::to_string_pretty(&template).unwrap();
    assert_eq!(parse(&text).unwrap(), template);
}

#[test]
fn an_unknown_key_is_refused_with_its_line() {
    let top = VALID.replacen("\"format\": 1,", "\"format\": 1,\n  \"colour\": \"blue\",", 1);
    let issues = parse(&top).unwrap_err();
    assert!(issues[0].message.contains("unknown field `colour`"), "{}", issues[0]);
    assert_eq!(issues[0].line, Some(3));

    let inner = VALID.replacen("\"version\": 2,", "\"version\": 2,\n    \"author\": \"me\",", 1);
    let issues = parse(&inner).unwrap_err();
    assert!(issues[0].message.contains("unknown field `author`"), "{}", issues[0]);
    assert!(issues[0].line.is_some());
}

#[test]
fn a_missing_block_or_key_is_refused() {
    let issues = parse(r#"{ "format": 1 }"#).unwrap_err();
    assert!(issues[0].message.contains("missing field `meta`"), "{}", issues[0]);
    let issues = parse(&VALID.replace("\"icon\": \"heartbeat\"", "\"colour\": \"red\"")).unwrap_err();
    assert!(!issues.is_empty());
}

#[test]
fn text_that_is_not_json_names_where_it_stopped() {
    let issues = parse("{ \"format\": 1,\n  meta }").unwrap_err();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].line, Some(2));
    assert!(issues[0].to_string().contains("line 2"));
    assert!(parse("").is_err());
    assert!(parse("[]").is_err());
}

#[test]
fn the_format_must_be_the_one_this_server_reads() {
    let newer = parse(&VALID.replace("\"format\": 1", "\"format\": 2")).unwrap_err();
    assert_eq!(newer[0].path, "format");
    assert!(newer[0].message.contains("newer"), "{}", newer[0]);
    let older = parse(&VALID.replace("\"format\": 1", "\"format\": 0")).unwrap_err();
    assert!(older[0].message.contains("older"), "{}", older[0]);
}

#[test]
fn an_id_has_its_length_and_its_characters() {
    let longest = "x".repeat(limits::ID_MAX);
    for good in ["ab", "operations", "knowledge-base", "a1-b2", longest.as_str()] {
        assert!(
            parse(&with_meta("id", &format!("\"{good}\""))).is_ok(),
            "{good} should be valid"
        );
    }
    let too_long = "x".repeat(limits::ID_MAX + 1);
    for bad in [
        "",
        "a",
        "Operations",
        "ops team",
        "ops_team",
        "-ops",
        "ops-",
        "ops--team",
        "opérations",
        "ops/team",
        too_long.as_str(),
    ] {
        assert_eq!(
            paths(parse(&with_meta("id", &format!("\"{bad}\"")))),
            ["meta.id"],
            "{bad:?} should be refused"
        );
    }
}

#[test]
fn a_version_is_a_whole_number_from_one() {
    assert!(parse(&with_meta("version", "1")).is_ok());
    assert!(parse(&with_meta("version", &limits::VERSION_MAX.to_string())).is_ok());
    assert_eq!(paths(parse(&with_meta("version", "0"))), ["meta.version"]);
    assert_eq!(
        paths(parse(&with_meta("version", &(limits::VERSION_MAX + 1).to_string()))),
        ["meta.version"]
    );
    assert!(parse(&with_meta("version", "-1")).is_err());
    assert!(parse(&with_meta("version", "1.5")).is_err());
    assert!(parse(&with_meta("version", "\"2\"")).is_err());
}

#[test]
fn a_name_and_a_summary_are_one_line_of_text_within_their_limits() {
    let longest_name = "n".repeat(limits::NAME_MAX);
    assert!(parse(&with_meta("name", &format!("\"{longest_name}\""))).is_ok());
    let too_long = "n".repeat(limits::NAME_MAX + 1);
    for bad in [
        "",
        "   ",
        too_long.as_str(),
        " padded",
        "padded ",
        "two\\nlines",
        "tab\\there",
    ] {
        assert_eq!(
            paths(parse(&with_meta("name", &format!("\"{bad}\"")))),
            ["meta.name"],
            "{bad:?} should be refused"
        );
    }
    // Limits count characters, not bytes: forty accented letters are forty.
    let accented = "é".repeat(limits::NAME_MAX);
    assert!(parse(&with_meta("name", &format!("\"{accented}\""))).is_ok());

    let longest_summary = "s".repeat(limits::SUMMARY_MAX);
    assert!(parse(&with_meta("summary", &format!("\"{longest_summary}\""))).is_ok());
    let too_long = "s".repeat(limits::SUMMARY_MAX + 1);
    for bad in ["", too_long.as_str(), "line\\none"] {
        assert_eq!(
            paths(parse(&with_meta("summary", &format!("\"{bad}\"")))),
            ["meta.summary"],
            "{bad:?} should be refused"
        );
    }
}

#[test]
fn an_icon_must_be_one_the_client_can_draw() {
    for icon in WORKSPACE_ICONS {
        assert!(
            parse(&with_meta("icon", &format!("\"{icon}\""))).is_ok(),
            "{icon} should be valid"
        );
    }
    for bad in ["", "Heartbeat", "dragon", "stack ", "../stack"] {
        assert_eq!(
            paths(parse(&with_meta("icon", &format!("\"{bad}\"")))),
            ["meta.icon"],
            "{bad:?} should be refused"
        );
    }
}

#[test]
fn every_problem_is_reported_at_once() {
    let text = VALID
        .replace("\"operations\"", "\"Ops\"")
        .replace("\"version\": 2", "\"version\": 0")
        .replace("\"heartbeat\"", "\"dragon\"")
        .replace("\"format\": 1", "\"format\": 9");
    assert_eq!(paths(parse(&text)), ["format", "meta.id", "meta.version", "meta.icon"]);
}

#[test]
fn a_file_is_bounded_in_size_and_in_depth() {
    let padding = " ".repeat(limits::FILE_BYTES);
    let issues = parse(&format!("{VALID}{padding}")).unwrap_err();
    assert!(issues[0].message.contains("bytes"), "{}", issues[0]);

    let deep = format!(
        "{}1{}",
        "[".repeat(limits::NESTING + 1),
        "]".repeat(limits::NESTING + 1)
    );
    let issues = parse(&deep).unwrap_err();
    assert!(issues[0].message.contains("nested"), "{}", issues[0]);
    // The valid file nests two levels, and brackets inside a string do not count.
    assert_eq!(super::nesting_depth(VALID), 2);
    assert_eq!(super::nesting_depth(r#"{"a": "[[[[[[[[[[ \" ]]]]"}"#), 1);
}

#[test]
fn the_icon_list_is_the_one_the_web_client_draws() {
    let client = include_str!("../../../apps/desktop/src/web/workspaceIcons.tsx");
    let keys: Vec<&str> = client
        .lines()
        .filter_map(|line| line.trim().strip_prefix("{ key: \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert_eq!(
        keys, WORKSPACE_ICONS,
        "workspaceIcons.tsx and repomemo-templates list different icons"
    );
}
