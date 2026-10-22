use crate::catalogue::BUILTIN_SOURCES;
use crate::*;

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

/// The valid file with a `starter_folders` block written as JSON.
fn with_folders(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"starter_folders\": {block},"),
        1,
    )
}

#[test]
fn a_template_lists_the_folders_a_new_workspace_starts_with() {
    let template = parse(&with_folders(
        r#"[ { "name": "Runbooks" }, { "name": "Incidents", "children": ["2026", "Postmortems"] } ]"#,
    ))
    .unwrap();
    assert_eq!(template.starter_folders.len(), 2);
    assert_eq!(template.starter_folders[0].name, "Runbooks");
    assert!(template.starter_folders[0].children.is_empty());
    assert_eq!(template.starter_folders[1].children, ["2026", "Postmortems"]);
    // Written out and read back it is the same, and a template without the block writes none.
    let text = serde_json::to_string(&template).unwrap();
    assert_eq!(parse(&text).unwrap(), template);
    assert!(!serde_json::to_string(&general()).unwrap().contains("starter_folders"));
    assert!(general().starter_folders.is_empty());
    // An empty list is a valid way to say none.
    assert!(parse(&with_folders("[]")).unwrap().starter_folders.is_empty());
}

#[test]
fn the_folders_are_bounded_in_number_and_in_levels() {
    let twelve: Vec<String> = (0..limits::STARTER_FOLDERS_MAX)
        .map(|n| format!("{{ \"name\": \"Folder {n}\" }}"))
        .collect();
    assert!(parse(&with_folders(&format!("[{}]", twelve.join(",")))).is_ok());
    let thirteen: Vec<String> = (0..=limits::STARTER_FOLDERS_MAX)
        .map(|n| format!("{{ \"name\": \"Folder {n}\" }}"))
        .collect();
    assert_eq!(
        paths(parse(&with_folders(&format!("[{}]", thirteen.join(","))))),
        ["starter_folders"]
    );
    // Children count with their parents: 4 folders with 2 children each is 12, one more is 13.
    let family = |children: usize| {
        let kids: Vec<String> = (0..children).map(|n| format!("\"Kid {n}\"")).collect();
        format!("{{ \"name\": \"Parent\", \"children\": [{}] }}", kids.join(","))
    };
    assert!(parse(&with_folders(&format!("[{}]", family(limits::STARTER_FOLDERS_MAX - 1)))).is_ok());
    assert!(parse(&with_folders(&format!("[{}]", family(limits::STARTER_FOLDERS_MAX)))).is_err());
    // Only two levels: a child is a name, and a folder inside a child is not part of the format.
    let deeper = r#"[ { "name": "A", "children": [ { "name": "B" } ] } ]"#;
    assert!(parse(&with_folders(deeper)).is_err());
    let unknown = r#"[ { "name": "A", "colour": "red" } ]"#;
    let issues = parse(&with_folders(unknown)).unwrap_err();
    assert!(issues[0].message.contains("unknown field `colour`"), "{}", issues[0]);
}

#[test]
fn a_folder_name_follows_the_rules_of_a_workspace_folder() {
    let longest = "f".repeat(limits::FOLDER_NAME_MAX);
    assert!(parse(&with_folders(&format!(r#"[ {{ "name": "{longest}" }} ]"#))).is_ok());
    let too_long = "f".repeat(limits::FOLDER_NAME_MAX + 1);
    for bad in [
        "",
        "   ",
        too_long.as_str(),
        " Padded",
        "Padded ",
        "a/b",
        "a\\\\b",
        "two\\nlines",
    ] {
        let issues = parse(&with_folders(&format!(r#"[ {{ "name": "{bad}" }} ]"#))).unwrap_err();
        assert_eq!(issues[0].path, "starter_folders[0].name", "{bad:?} should be refused");
    }
    let issues = parse(&with_folders(r#"[ { "name": "Ok", "children": ["fine", "a/b"] } ]"#)).unwrap_err();
    assert_eq!(issues[0].path, "starter_folders[0].children[1]");
}

#[test]
fn two_folders_beside_each_other_cannot_share_a_name() {
    let top = parse(&with_folders(r#"[ { "name": "Runbooks" }, { "name": "runbooks" } ]"#)).unwrap_err();
    assert_eq!(top[0].path, "starter_folders[1].name");
    let below = parse(&with_folders(r#"[ { "name": "A", "children": ["x", "X"] } ]"#)).unwrap_err();
    assert_eq!(below[0].path, "starter_folders[0].children[1]");
    // The same name at another level, or under another parent, is fine.
    let apart =
        r#"[ { "name": "A", "children": ["Notes"] }, { "name": "B", "children": ["Notes"] }, { "name": "Notes" } ]"#;
    assert!(parse(&with_folders(apart)).is_ok());
}

#[test]
fn the_first_four_templates_ship_as_data_with_the_folders_a_new_workspace_starts_with() {
    let catalogue = Catalogue::builtin();
    assert!(
        Catalogue::builtin_problems().is_empty(),
        "{:?}",
        Catalogue::builtin_problems()
    );
    // General first, then the four by name.
    assert_eq!(
        ids(catalogue),
        [
            "general",
            "knowledge-base",
            "operations",
            "product-decisions",
            "software-engineering"
        ]
    );
    let expected: [(&str, &str, &[&str]); 4] = [
        (
            "software-engineering",
            "Software engineering",
            &["Architecture", "Decisions", "Runbooks"],
        ),
        (
            "operations",
            "Operations and incidents",
            &["Runbooks", "Incidents", "Postmortems", "On-call"],
        ),
        (
            "product-decisions",
            "Product and decisions",
            &["Decisions", "Specs", "Research", "Meeting notes"],
        ),
        (
            "knowledge-base",
            "Knowledge base",
            &["Policies", "Contracts", "Reports", "Archive"],
        ),
    ];
    for (id, name, folders) in expected {
        let template = catalogue.get(id).unwrap_or_else(|| panic!("{id} is built in"));
        assert_eq!(template.meta.name, name);
        assert!(template.validate().is_empty(), "{id}: {:?}", template.validate());
        let made: Vec<&str> = template
            .starter_folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect();
        assert_eq!(made, folders, "{id}");
        assert!(
            template.starter_folders.iter().all(|folder| folder.children.is_empty()),
            "{id}"
        );
    }
    // General stays defined in code, with none.
    assert!(catalogue.get(GENERAL_ID).unwrap().starter_folders.is_empty());
}

/// The valid file with a `sections` block written as JSON.
fn with_sections(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"sections\": {block},"),
        1,
    )
}

fn names(sections: &[String]) -> Vec<&str> {
    sections.iter().map(String::as_str).collect()
}

#[test]
fn a_template_without_a_sections_block_arranges_the_twelve_as_they_always_were() {
    let usual = general().sections();
    assert_eq!(names(&usual.primary), WORKSPACE_SECTIONS);
    assert!(usual.more.is_empty());
    assert!(parse(VALID).unwrap().sections().more.is_empty());
    assert_eq!(names(&parse(VALID).unwrap().sections().primary), WORKSPACE_SECTIONS);
    // An empty block says the same.
    let empty = parse(&with_sections("{}")).unwrap().sections();
    assert_eq!(names(&empty.primary), WORKSPACE_SECTIONS);
    assert!(empty.more.is_empty());
}

#[test]
fn the_order_leads_and_a_section_named_nowhere_follows_in_the_usual_order() {
    let sections = parse(&with_sections(
        r#"{ "order": ["review", "overview"], "more": ["map", "documents"] }"#,
    ))
    .unwrap()
    .sections();
    assert_eq!(
        names(&sections.primary),
        [
            "review",
            "overview",
            "evidence",
            "repositories",
            "retrieval",
            "health",
            "memory",
            "tasks",
            "people",
            "settings"
        ]
    );
    assert_eq!(names(&sections.more), ["map", "documents"]);
    // Every section is in exactly one list.
    let mut all: Vec<&str> = names(&sections.primary)
        .into_iter()
        .chain(names(&sections.more))
        .collect();
    all.sort();
    let mut every = WORKSPACE_SECTIONS.to_vec();
    every.sort();
    assert_eq!(all, every);
    // Only `more`: the rest keep their usual order.
    let only_more = parse(&with_sections(r#"{ "more": ["overview"] }"#)).unwrap().sections();
    assert_eq!(names(&only_more.primary)[0], "evidence");
    assert_eq!(names(&only_more.more), ["overview"]);
}

#[test]
fn a_section_must_exist_and_be_named_once() {
    let unknown = parse(&with_sections(r#"{ "order": ["overview", "dashboard"] }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "sections.order[1]");
    assert!(unknown[0].message.contains("dashboard"));
    let in_more = parse(&with_sections(r#"{ "more": ["Map"] }"#)).unwrap_err();
    assert_eq!(in_more[0].path, "sections.more[0]");
    let twice = parse(&with_sections(r#"{ "order": ["map", "map"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "sections.order[1]");
    let both = parse(&with_sections(r#"{ "order": ["map"], "more": ["overview", "map"] }"#)).unwrap_err();
    assert_eq!(both[0].path, "sections.more[1]");
    let unknown_key = parse(&with_sections(r#"{ "order": ["map"], "colour": "map" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
    // The block is written back as it was read.
    let template = parse(&with_sections(r#"{ "order": ["review"], "more": ["map"] }"#)).unwrap();
    assert_eq!(parse(&serde_json::to_string(&template).unwrap()).unwrap(), template);
    assert!(!serde_json::to_string(&general()).unwrap().contains("sections"));
}

#[test]
fn a_workspace_opens_on_overview_unless_the_template_names_a_landing_section() {
    assert_eq!(general().sections().landing, "overview");
    assert_eq!(parse(&with_sections("{}")).unwrap().sections().landing, "overview");
    for landing in ["review", "documents", "map", "overview"] {
        let sections = parse(&with_sections(&format!(
            r#"{{ "more": ["map"], "landing": "{landing}" }}"#
        )))
        .unwrap()
        .sections();
        assert_eq!(sections.landing, landing);
    }
    // A landing section can be one kept under More, and the open section is then shown as a tab by the client.
    let unknown = parse(&with_sections(r#"{ "landing": "dashboard" }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "sections.landing");
    for admin_only in ADMIN_ONLY_SECTIONS {
        let refused = parse(&with_sections(&format!(r#"{{ "landing": "{admin_only}" }}"#))).unwrap_err();
        assert_eq!(refused[0].path, "sections.landing", "{admin_only}");
        assert!(refused[0].message.contains("administrators"), "{}", refused[0]);
    }
    assert_eq!(ADMIN_ONLY_SECTIONS, ["people", "settings"]);
    let template = parse(&with_sections(r#"{ "landing": "review" }"#)).unwrap();
    assert_eq!(parse(&serde_json::to_string(&template).unwrap()).unwrap(), template);
}

/// The valid file with a `labels` block written as JSON.
fn with_labels(block: &str) -> String {
    VALID.replacen("\"format\": 1,", &format!("\"format\": 1,\n  \"labels\": {block},"), 1)
}

#[test]
fn a_template_can_rename_sections_and_kinds_of_evidence() {
    let template = parse(&with_labels(
        r#"{ "sections": { "tasks": "Actions", "memory": "Lessons" }, "artifact_types": { "pr": "Pull request" } }"#,
    ))
    .unwrap();
    let labels = template.labels();
    assert_eq!(labels.sections["tasks"], "Actions");
    assert_eq!(labels.sections.len(), 2);
    assert_eq!(labels.artifact_types["pr"], "Pull request");
    // Without the block there are no new names, and General has none.
    assert!(general().labels().sections.is_empty() && general().labels().artifact_types.is_empty());
    assert!(parse(VALID).unwrap().labels().sections.is_empty());
    assert!(parse(&with_labels("{}")).unwrap().labels().artifact_types.is_empty());
    // Written back as it was read.
    assert_eq!(parse(&serde_json::to_string(&template).unwrap()).unwrap(), template);
    assert!(!serde_json::to_string(&general()).unwrap().contains("labels"));
}

#[test]
fn a_name_must_be_for_a_known_section_or_kind_and_follow_the_rules_of_a_name() {
    let unknown_section = parse(&with_labels(r#"{ "sections": { "dashboard": "Home" } }"#)).unwrap_err();
    assert_eq!(unknown_section[0].path, "labels.sections.dashboard");
    let unknown_kind = parse(&with_labels(r#"{ "artifact_types": { "spreadsheet": "Sheet" } }"#)).unwrap_err();
    assert_eq!(unknown_kind[0].path, "labels.artifact_types.spreadsheet");
    let longest = "l".repeat(limits::LABEL_MAX);
    assert!(parse(&with_labels(&format!(
        r#"{{ "sections": {{ "tasks": "{longest}" }} }}"#
    )))
    .is_ok());
    let too_long = "l".repeat(limits::LABEL_MAX + 1);
    for bad in ["", "   ", too_long.as_str(), " Padded", "Padded ", "two\\nlines"] {
        let issues = parse(&with_labels(&format!(r#"{{ "sections": {{ "tasks": "{bad}" }} }}"#))).unwrap_err();
        assert_eq!(issues[0].path, "labels.sections.tasks", "{bad:?} should be refused");
    }
    let unknown_key = parse(&with_labels(r#"{ "colours": {} }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colours`"),
        "{}",
        unknown_key[0]
    );
    // The kinds of evidence a template can name: twelve, with no repeat.
    let mut kinds = ARTIFACT_TYPES.to_vec();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds.len(), 12);
}

#[test]
fn the_four_first_templates_name_what_their_teams_call_things() {
    let catalogue = Catalogue::builtin();
    let section = |id: &str, key: &str| catalogue.get(id).unwrap().labels().sections.get(key).cloned();
    assert_eq!(section("operations", "tasks").as_deref(), Some("Actions"));
    assert_eq!(
        section("operations", "evidence").as_deref(),
        Some("Runbooks and incidents")
    );
    assert_eq!(section("operations", "memory").as_deref(), Some("Lessons"));
    assert_eq!(
        section("product-decisions", "memory").as_deref(),
        Some("Decisions and memory")
    );
    assert_eq!(section("knowledge-base", "evidence").as_deref(), Some("Library"));
    assert_eq!(section("software-engineering", "tasks"), None);
    let kinds = catalogue.get("software-engineering").unwrap().labels().artifact_types;
    assert_eq!(
        (kinds["api_spec"].as_str(), kinds["pr"].as_str()),
        ("API spec", "Pull request")
    );
}

/// The valid file with an `overview` block written as JSON.
fn with_overview(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"overview\": {block},"),
        1,
    )
}

#[test]
fn the_overview_shows_the_usual_nine_cards_unless_a_template_chooses() {
    assert_eq!(general().overview_cards(), DEFAULT_CARDS);
    assert_eq!(parse(VALID).unwrap().overview_cards(), DEFAULT_CARDS);
    for card in DEFAULT_CARDS {
        assert!(PULSE_CARDS.contains(&card), "{card} is a pulse card");
    }
    let chosen = parse(&with_overview(
        r#"{ "cards": ["memory_cards", "at_risk", "fresh_evidence"] }"#,
    ))
    .unwrap();
    assert_eq!(chosen.overview_cards(), ["memory_cards", "at_risk", "fresh_evidence"]);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("overview"));
}

#[test]
fn a_card_must_exist_be_listed_once_and_there_are_at_most_eight() {
    let unknown = parse(&with_overview(r#"{ "cards": ["at_risk", "revenue"] }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "overview.cards[1]");
    let twice = parse(&with_overview(r#"{ "cards": ["at_risk", "at_risk"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "overview.cards[1]");
    let none = parse(&with_overview("{}")).unwrap_err();
    assert_eq!(none[0].path, "overview.cards");
    let eight: Vec<String> = PULSE_CARDS
        .iter()
        .take(CARDS_MAX)
        .map(|card| format!("\"{card}\""))
        .collect();
    assert!(parse(&with_overview(&format!(r#"{{ "cards": [{}] }}"#, eight.join(",")))).is_ok());
    let nine: Vec<String> = PULSE_CARDS
        .iter()
        .take(CARDS_MAX + 1)
        .map(|card| format!("\"{card}\""))
        .collect();
    let too_many = parse(&with_overview(&format!(r#"{{ "cards": [{}] }}"#, nine.join(",")))).unwrap_err();
    assert_eq!(too_many[0].path, "overview.cards");
    let unknown_key = parse(&with_overview(r#"{ "cards": ["at_risk"], "colour": "red" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn the_card_list_is_the_one_the_web_client_draws_and_the_four_templates_choose_theirs() {
    let client = include_str!("../../../apps/desktop/src/web/workspace/pulseCards.tsx");
    let line = client
        .lines()
        .find(|line| line.starts_with("export const PULSE_CARD_IDS"))
        .expect("the client lists the cards");
    let listed: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
    assert_eq!(
        listed, PULSE_CARDS,
        "pulseCards.tsx and repomemo-templates list different cards"
    );

    let catalogue = Catalogue::builtin();
    let cards = |id: &str| catalogue.get(id).unwrap().overview_cards();
    assert_eq!(
        cards("software-engineering"),
        ["code_symbols", "sources", "index_coverage", "at_risk", "fresh_evidence"]
    );
    assert_eq!(
        cards("operations"),
        ["fresh_evidence", "active_tasks", "at_risk", "memory_cards"]
    );
    assert_eq!(
        cards("product-decisions"),
        ["memory_cards", "active_tasks", "fresh_evidence", "index_coverage"]
    );
    assert_eq!(
        cards("knowledge-base"),
        [
            "index_coverage",
            "indexed_storage",
            "fresh_evidence",
            "knowledge_density"
        ]
    );
}

/// The valid file with an `evidence` block written as JSON.
fn with_evidence(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"evidence\": {block},"),
        1,
    )
}

#[test]
fn the_ledger_opens_as_a_grid_with_no_preferred_types_unless_a_template_chooses() {
    let usual = general().evidence();
    assert_eq!(
        (usual.default_view.as_str(), usual.types_first.is_empty()),
        ("grid", true)
    );
    assert_eq!(parse(VALID).unwrap().evidence().default_view, "grid");
    assert_eq!(parse(&with_evidence("{}")).unwrap().evidence().default_view, "grid");
    let chosen = parse(&with_evidence(
        r#"{ "default_view": "list", "types_first": ["pdf", "word"] }"#,
    ))
    .unwrap();
    assert_eq!(chosen.evidence().default_view, "list");
    assert_eq!(chosen.evidence().types_first, ["pdf", "word"]);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("evidence"));
}

#[test]
fn a_view_and_the_file_types_must_be_known_and_a_type_listed_once() {
    let view = parse(&with_evidence(r#"{ "default_view": "documents" }"#)).unwrap_err();
    assert_eq!(view[0].path, "evidence.default_view");
    let unknown = parse(&with_evidence(r#"{ "types_first": ["pdf", "spreadsheet"] }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "evidence.types_first[1]");
    let twice = parse(&with_evidence(r#"{ "types_first": ["pdf", "pdf"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "evidence.types_first[1]");
    let unknown_key = parse(&with_evidence(r#"{ "colour": "red" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
    assert_eq!(EVIDENCE_VIEWS, ["grid", "list"]);
    assert_eq!(FILE_TYPES.len(), 12);
}

#[test]
fn the_file_types_are_the_ones_the_ledger_filters_by_and_the_four_templates_choose_theirs() {
    let client = include_str!("../../../apps/desktop/src/web/constants.ts");
    let start = client
        .find("export const FILE_CATEGORIES")
        .expect("the client lists the file types");
    let block = &client[start..start + client[start..].find("];").unwrap()];
    let listed: Vec<&str> = block
        .split("value: \"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert_eq!(
        listed, FILE_TYPES,
        "constants.ts and repomemo-templates list different file types"
    );

    let catalogue = Catalogue::builtin();
    let evidence = |id: &str| catalogue.get(id).unwrap().evidence();
    assert_eq!(evidence("knowledge-base").default_view, "list");
    assert_eq!(
        evidence("knowledge-base").types_first,
        ["word", "pdf", "excel", "powerpoint"]
    );
    assert_eq!(evidence("operations").default_view, "list");
    assert_eq!(evidence("software-engineering").default_view, "grid");
    assert_eq!(
        evidence("software-engineering").types_first,
        ["code", "markdown", "repository"]
    );
    assert_eq!(evidence("product-decisions").types_first, ["markdown", "note", "word"]);
}

/// The valid file with an `assistant` block written as JSON.
fn with_assistant(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"assistant\": {block},"),
        1,
    )
}

/// The valid file with a `retrieval` block written as JSON.
fn with_retrieval(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"retrieval\": {block},"),
        1,
    )
}

#[test]
fn the_assistant_keeps_its_usual_buttons_and_suggests_nothing_unless_a_template_chooses() {
    assert!(general().assistant().order.is_empty() && general().assistant().suggestions.is_empty());
    assert!(general().retrieval().type_chips.is_empty());
    assert!(parse(VALID).unwrap().assistant().suggestions.is_empty());
    let chosen = parse(&with_assistant(r#"{ "order": ["ask_question", "find_files"], "suggestions": ["What is the runbook for…", "Who is on call?"] }"#)).unwrap();
    assert_eq!(chosen.assistant().order, ["ask_question", "find_files"]);
    assert_eq!(chosen.assistant().suggestions[0], "What is the runbook for…");
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("assistant"));
    assert!(!serde_json::to_string(&general()).unwrap().contains("retrieval"));
    assert_eq!(CAPABILITIES.len(), 6);
}

#[test]
fn the_assistants_order_and_suggestions_follow_the_rules_of_a_name() {
    let unknown = parse(&with_assistant(r#"{ "order": ["ask_question", "book_flights"] }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "assistant.order[1]");
    let twice = parse(&with_assistant(r#"{ "order": ["find_files", "find_files"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "assistant.order[1]");
    let longest = "q".repeat(SUGGESTION_MAX);
    assert!(parse(&with_assistant(&format!(r#"{{ "suggestions": ["{longest}"] }}"#))).is_ok());
    let too_long = "q".repeat(SUGGESTION_MAX + 1);
    for bad in ["", "  ", too_long.as_str(), " padded", "two\\nlines"] {
        let issues = parse(&with_assistant(&format!(r#"{{ "suggestions": ["{bad}"] }}"#))).unwrap_err();
        assert_eq!(issues[0].path, "assistant.suggestions[0]", "{bad:?} should be refused");
    }
    let repeated = parse(&with_assistant(
        r#"{ "suggestions": ["Who is on call?", "who is on call?"] }"#,
    ))
    .unwrap_err();
    assert_eq!(repeated[0].path, "assistant.suggestions[1]");
    let seven: Vec<String> = (0..=SUGGESTIONS_MAX).map(|n| format!("\"Question {n}\"")).collect();
    let too_many = parse(&with_assistant(&format!(
        r#"{{ "suggestions": [{}] }}"#,
        seven.join(",")
    )))
    .unwrap_err();
    assert_eq!(too_many[0].path, "assistant.suggestions");
    let unknown_key = parse(&with_assistant(r#"{ "phrases": [] }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `phrases`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn retrieval_chips_are_kinds_of_evidence_listed_once_and_at_most_six() {
    let chosen = parse(&with_retrieval(r#"{ "type_chips": ["runbook", "incident"] }"#)).unwrap();
    assert_eq!(chosen.retrieval().type_chips, ["runbook", "incident"]);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(parse(&with_retrieval("{}")).unwrap().retrieval().type_chips.is_empty());
    let unknown = parse(&with_retrieval(r#"{ "type_chips": ["runbook", "spreadsheet"] }"#)).unwrap_err();
    assert_eq!(unknown[0].path, "retrieval.type_chips[1]");
    let twice = parse(&with_retrieval(r#"{ "type_chips": ["note", "note"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "retrieval.type_chips[1]");
    let seven: Vec<String> = ARTIFACT_TYPES
        .iter()
        .take(TYPE_CHIPS_MAX + 1)
        .map(|kind| format!("\"{kind}\""))
        .collect();
    let too_many = parse(&with_retrieval(&format!(
        r#"{{ "type_chips": [{}] }}"#,
        seven.join(",")
    )))
    .unwrap_err();
    assert_eq!(too_many[0].path, "retrieval.type_chips");
}

#[test]
fn the_four_first_templates_suggest_what_their_teams_ask() {
    let catalogue = Catalogue::builtin();
    let operations = catalogue.get("operations").unwrap();
    assert_eq!(
        operations.assistant().order,
        ["ask_question", "search_content", "find_files"]
    );
    assert!(operations
        .assistant()
        .suggestions
        .iter()
        .any(|suggestion| suggestion.starts_with("What is the runbook for")));
    assert_eq!(operations.retrieval().type_chips, ["runbook", "incident", "note"]);
    assert_eq!(
        catalogue.get("software-engineering").unwrap().assistant().order[0],
        "find_symbol"
    );
    assert_eq!(
        catalogue.get("knowledge-base").unwrap().assistant().order[0],
        "find_files"
    );
    for id in [
        "software-engineering",
        "operations",
        "product-decisions",
        "knowledge-base",
    ] {
        let template = catalogue.get(id).unwrap();
        assert!(
            !template.assistant().suggestions.is_empty() && template.assistant().suggestions.len() <= SUGGESTIONS_MAX,
            "{id}"
        );
        assert!(!template.retrieval().type_chips.is_empty(), "{id}");
    }
}

#[test]
fn the_section_list_is_the_one_the_web_client_has_pages_for() {
    let client = include_str!("../../../apps/desktop/src/web/constants.ts");
    let line = client
        .lines()
        .find(|line| line.starts_with("export const WORKSPACE_SECTIONS"))
        .expect("the client lists the sections");
    let listed: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
    assert_eq!(
        listed, WORKSPACE_SECTIONS,
        "constants.ts and repomemo-templates list different sections"
    );
}

#[test]
fn the_four_first_templates_put_their_lesser_sections_under_more() {
    let catalogue = Catalogue::builtin();
    let expected: [(&str, &[&str], &[&str]); 4] = [
        (
            "software-engineering",
            &[
                "overview",
                "repositories",
                "retrieval",
                "evidence",
                "map",
                "health",
                "review",
                "memory",
                "tasks",
                "people",
                "settings",
            ],
            &["documents"],
        ),
        (
            "operations",
            &[
                "overview",
                "evidence",
                "review",
                "health",
                "retrieval",
                "tasks",
                "memory",
                "people",
                "settings",
            ],
            &["map", "repositories", "documents"],
        ),
        (
            "product-decisions",
            &[
                "overview",
                "memory",
                "evidence",
                "retrieval",
                "tasks",
                "review",
                "documents",
                "people",
                "settings",
            ],
            &["health", "map", "repositories"],
        ),
        (
            "knowledge-base",
            &[
                "overview",
                "documents",
                "retrieval",
                "evidence",
                "review",
                "health",
                "memory",
                "tasks",
                "people",
                "settings",
            ],
            &["map", "repositories"],
        ),
    ];
    for (id, primary, more) in expected {
        let sections = catalogue.get(id).unwrap().sections();
        assert_eq!(names(&sections.primary), primary, "{id}");
        assert_eq!(names(&sections.more), more, "{id}");
    }
    // Where each opens (V0.2.172): Operations on Review, Product and decisions on Memory, Knowledge base on Documents.
    let lands_on = |id: &str| catalogue.get(id).unwrap().sections().landing;
    assert_eq!(lands_on("software-engineering"), "overview");
    assert_eq!(lands_on("operations"), "review");
    assert_eq!(lands_on("product-decisions"), "memory");
    assert_eq!(lands_on("knowledge-base"), "documents");
}

fn catalogue_with_operations(version: u32) -> Catalogue {
    let file = VALID.replace("\"version\": 2", &format!("\"version\": {version}"));
    let (catalogue, problems) = Catalogue::load([("operations.json", file.as_str())]);
    assert!(problems.is_empty(), "{problems:?}");
    catalogue
}

#[test]
fn no_choice_resolves_to_general() {
    let resolution = catalogue_with_operations(2).resolve(None);
    assert_eq!(resolution.template, general());
    assert_eq!(resolution.origin, Origin::Default);
}

#[test]
fn a_choice_the_catalogue_has_is_used_at_its_current_file() {
    let catalogue = catalogue_with_operations(2);
    for saved in [1, 2] {
        let resolution = catalogue.resolve(Some(("operations", saved)));
        assert_eq!(resolution.template.id(), "operations", "saved at version {saved}");
        assert_eq!(resolution.template.meta.version, 2);
        assert_eq!(resolution.origin, Origin::Workspace);
    }
    let general_chosen = catalogue.resolve(Some((GENERAL_ID, 1)));
    assert_eq!(general_chosen.template, general());
    assert_eq!(general_chosen.origin, Origin::Workspace);
}

#[test]
fn a_missing_template_resolves_to_general_with_a_reason() {
    let resolution = catalogue_with_operations(2).resolve(Some(("no-such-template", 1)));
    assert_eq!(resolution.template, general());
    let Origin::Fallback { reason } = resolution.origin else {
        panic!("a fallback was expected")
    };
    assert!(
        reason.contains("no-such-template") && reason.contains("not available"),
        "{reason}"
    );
}

#[test]
fn a_choice_saved_by_a_newer_server_resolves_to_general_with_a_reason() {
    let resolution = catalogue_with_operations(2).resolve(Some(("operations", 3)));
    assert_eq!(resolution.template, general());
    let Origin::Fallback { reason } = resolution.origin else {
        panic!("a fallback was expected")
    };
    assert!(
        reason.contains("operations") && reason.contains("newer server"),
        "{reason}"
    );
}
