use crate::catalogue::BUILTIN_SOURCES;
use crate::{general, limits, Catalogue, TemplateDefinition, TemplateIssue, FORMAT, GENERAL_ID, WORKSPACE_ICONS};

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

/// A valid template file with the given id and name.
fn template_file(id: &str, name: &str) -> String {
    VALID
        .replace("\"operations\"", &format!("\"{id}\""))
        .replace("Operations and incidents", name)
}

fn ids(catalogue: &Catalogue) -> Vec<&str> {
    catalogue.list().iter().map(|template| template.id()).collect()
}

#[test]
fn the_built_in_catalogue_has_general_first_and_every_file_of_the_folder() {
    let catalogue = Catalogue::builtin();
    assert!(
        Catalogue::builtin_problems().is_empty(),
        "{:?}",
        Catalogue::builtin_problems()
    );
    assert_eq!(catalogue.list()[0], general());
    assert_eq!(catalogue.get(GENERAL_ID), Some(&general()));
    assert!(catalogue.get("no-such-template").is_none());

    // Each file of the folder is registered, with no Rust edited to say so.
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("definitions");
    let mut files: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json") && !name.starts_with('_'))
        .collect();
    files.sort();
    let registered: Vec<&str> = BUILTIN_SOURCES.iter().map(|(name, _)| *name).collect();
    assert_eq!(registered, files);
    assert_eq!(catalogue.list().len(), 1 + files.len());

    // Every id is unique and every list is in the same order every time.
    let mut seen = ids(catalogue);
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), catalogue.list().len());
    assert_eq!(ids(catalogue), ids(Catalogue::builtin()));
}

#[test]
fn a_valid_file_added_to_the_folder_appears_in_the_catalogue() {
    // The folder's own files plus one more, as a copy would hold them.
    let mut sources: Vec<(String, String)> = BUILTIN_SOURCES
        .iter()
        .map(|(name, text)| (name.to_string(), text.to_string()))
        .collect();
    sources.push(("support.json".to_owned(), template_file("support", "Support desk")));
    let (catalogue, problems) = Catalogue::load(sources.iter().map(|(name, text)| (name.as_str(), text.as_str())));
    assert!(problems.is_empty(), "{problems:?}");
    assert!(ids(&catalogue).contains(&"support"));
    assert_eq!(catalogue.get("support").unwrap().meta.name, "Support desk");
    assert_eq!(catalogue.list().len(), BUILTIN_SOURCES.len() + 2);
    assert_eq!(catalogue.list()[0].id(), GENERAL_ID);
}

#[test]
fn templates_follow_general_by_name_whatever_the_order_they_arrive_in() {
    let zebra = template_file("zebra", "Zebra");
    let apple = template_file("apple", "apple pie");
    let mango = template_file("mango", "Mango");
    let forward = [
        ("apple.json", apple.as_str()),
        ("mango.json", mango.as_str()),
        ("zebra.json", zebra.as_str()),
    ];
    let backward = [
        ("zebra.json", zebra.as_str()),
        ("mango.json", mango.as_str()),
        ("apple.json", apple.as_str()),
    ];
    let (first, _) = Catalogue::load(forward);
    let (second, _) = Catalogue::load(backward);
    assert_eq!(ids(&first), ["general", "apple", "mango", "zebra"]);
    assert_eq!(first, second);
}

#[test]
fn a_file_that_cannot_be_used_is_left_out_and_named_and_the_others_stay() {
    let good = template_file("good", "Good");
    let bad = VALID.replace("heartbeat", "dragon");
    let misnamed = template_file("other", "Other");
    let first = template_file("twin", "First twin");
    let second = template_file("twin", "Second twin");
    let shadow = template_file(GENERAL_ID, "Not general");
    let (catalogue, problems) = Catalogue::load([
        ("good.json", good.as_str()),
        ("bad.json", bad.as_str()),
        ("misnamed.json", misnamed.as_str()),
        ("twin.json", first.as_str()),
        ("twin.json", second.as_str()),
        ("general.json", shadow.as_str()),
    ]);
    assert_eq!(ids(&catalogue), ["general", "good", "twin"]);
    assert_eq!(catalogue.get("twin").unwrap().meta.name, "First twin");
    assert_eq!(catalogue.get(GENERAL_ID), Some(&general()));

    let named: Vec<&str> = problems.iter().map(|problem| problem.source.as_str()).collect();
    assert_eq!(named, ["bad.json", "general.json", "misnamed.json", "twin.json"]);
    assert_eq!(problems[0].issues[0].path, "meta.icon");
    assert!(problems[1].issues[0].message.contains("cannot be replaced"));
    assert!(problems[2].issues[0].message.contains("named after its id"));
    assert!(problems[3].issues[0].message.contains("already has the id"));
}

#[test]
fn a_catalogue_of_nothing_is_general_alone() {
    let (catalogue, problems) = Catalogue::load([]);
    assert!(problems.is_empty());
    assert_eq!(catalogue, Catalogue::general_only());
    assert_eq!(ids(&catalogue), ["general"]);
}
