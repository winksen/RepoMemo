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
    let unknown_key = parse(&with_assistant(r#"{ "colour": "red" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn a_template_may_add_phrases_to_the_router_and_one_line_about_what_the_workspace_is_for() {
    assert!(general().assistant().phrases.is_empty() && general().assistant().focus.is_none());
    let chosen = parse(&with_assistant(
        r#"{ "phrases": [ { "phrase": "runbook for", "capability": "find_files" }, { "phrase": "on-call", "capability": "search_content" } ],
             "focus": "Runbooks, incidents and on-call knowledge." }"#,
    ))
    .unwrap();
    let assistant = chosen.assistant();
    assert_eq!(assistant.phrases.len(), 2);
    assert_eq!(
        (
            assistant.phrases[0].phrase.as_str(),
            assistant.phrases[0].capability.as_str()
        ),
        ("runbook for", "find_files")
    );
    assert_eq!(
        assistant.focus.as_deref(),
        Some("Runbooks, incidents and on-call knowledge.")
    );
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("focus"));
}

#[test]
fn a_phrase_is_a_few_words_for_a_job_the_assistant_has_and_the_focus_is_one_short_line() {
    let phrase = |text: &str, capability: &str| {
        with_assistant(&format!(
            r#"{{ "phrases": [ {{ "phrase": "{text}", "capability": "{capability}" }} ] }}"#
        ))
    };
    assert!(parse(&phrase("meeting notes", "find_files")).is_ok());
    assert!(parse(&phrase("what's the", "ask_question")).is_ok());
    let longest = "p".repeat(PHRASE_MAX);
    assert!(parse(&phrase(&longest, "find_files")).is_ok());
    let too_long = "p".repeat(PHRASE_MAX + 1);
    for bad in [
        "",
        "  ",
        " padded",
        too_long.as_str(),
        "a.*b",
        "(a|b)",
        "a/b",
        "two\\nlines",
        "[x]",
    ] {
        assert_eq!(
            parse(&phrase(bad, "find_files")).unwrap_err()[0].path,
            "assistant.phrases[0].phrase",
            "{bad:?}"
        );
    }
    assert_eq!(
        parse(&phrase("runbook", "book_flights")).unwrap_err()[0].path,
        "assistant.phrases[0].capability"
    );
    let twice = parse(&with_assistant(
        r#"{ "phrases": [ { "phrase": "Runbook", "capability": "find_files" }, { "phrase": "runbook", "capability": "search_content" } ] }"#,
    ))
    .unwrap_err();
    assert_eq!(twice[0].path, "assistant.phrases[1].phrase");
    let many: Vec<String> = (0..=PHRASES_MAX)
        .map(|n| format!(r#"{{ "phrase": "word{n}", "capability": "find_files" }}"#))
        .collect();
    assert_eq!(
        parse(&with_assistant(&format!(r#"{{ "phrases": [{}] }}"#, many.join(",")))).unwrap_err()[0].path,
        "assistant.phrases"
    );
    let focus = |text: &str| with_assistant(&format!(r#"{{ "focus": "{text}" }}"#));
    assert!(parse(&focus(&"f".repeat(FOCUS_MAX))).is_ok());
    for bad in ["", "  ", " padded", "two\\nlines"] {
        assert_eq!(parse(&focus(bad)).unwrap_err()[0].path, "assistant.focus", "{bad:?}");
    }
    assert_eq!(
        parse(&focus(&"f".repeat(FOCUS_MAX + 1))).unwrap_err()[0].path,
        "assistant.focus"
    );
}

#[test]
fn the_four_first_templates_know_what_their_workspaces_are_for() {
    let catalogue = Catalogue::builtin();
    for id in [
        "software-engineering",
        "operations",
        "product-decisions",
        "knowledge-base",
    ] {
        let assistant = catalogue.get(id).unwrap().assistant();
        assert!(
            !assistant.phrases.is_empty() && assistant.phrases.len() <= PHRASES_MAX,
            "{id}"
        );
        assert!(
            assistant
                .focus
                .as_deref()
                .is_some_and(|focus| focus.starts_with("This workspace holds")),
            "{id}"
        );
    }
    let operations = catalogue.get("operations").unwrap().assistant();
    assert!(operations
        .phrases
        .iter()
        .any(|rule| rule.phrase == "runbook" && rule.capability == "find_files"));
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

fn overrides(text: &str) -> Result<Overrides, Vec<TemplateIssue>> {
    Overrides::parse(&serde_json::from_str(text).unwrap())
}

#[test]
fn an_administrator_may_adjust_the_landing_section_what_is_under_more_and_the_suggestions() {
    let adjusted =
        overrides(r#"{ "landing": "review", "more": ["map", "documents"], "suggestions": ["Who is on call?"] }"#)
            .unwrap();
    assert!(!adjusted.is_empty());
    assert!(Overrides::default().is_empty() && overrides("{}").unwrap().is_empty());
    assert_eq!(
        serde_json::from_str::<Overrides>(&serde_json::to_string(&adjusted).unwrap()).unwrap(),
        adjusted
    );

    // Laid over General: the landing section changes, the two sections leave the tabs and the questions are given.
    let template = general().with_overrides(&adjusted);
    let sections = template.sections();
    assert_eq!(sections.landing, "review");
    assert_eq!(names(&sections.more), ["map", "documents"]);
    assert!(!names(&sections.primary).contains(&"map") && !names(&sections.primary).contains(&"documents"));
    assert_eq!(sections.primary.len() + sections.more.len(), 12);
    assert_eq!(template.assistant().suggestions, ["Who is on call?"]);
    // Nothing else of the template changes.
    assert_eq!(template.meta, general().meta);
    assert_eq!(template.overview_cards(), general().overview_cards());
    assert_eq!(general().with_overrides(&Overrides::default()), general());
}

#[test]
fn adjustments_replace_what_the_template_says_for_that_key_only() {
    let catalogue = Catalogue::builtin();
    let operations = catalogue.get("operations").unwrap();
    // Landing alone leaves the template's tabs and questions.
    let landing = operations.with_overrides(&overrides(r#"{ "landing": "memory" }"#).unwrap());
    assert_eq!(landing.sections().landing, "memory");
    assert_eq!(landing.sections().primary, operations.sections().primary);
    assert_eq!(landing.assistant().suggestions, operations.assistant().suggestions);
    // Under More is the exact list: a section the template kept there that is not named comes back as a tab.
    let more = operations.with_overrides(&overrides(r#"{ "more": ["tasks"] }"#).unwrap());
    assert_eq!(names(&more.sections().more), ["tasks"]);
    assert!(names(&more.sections().primary).contains(&"map"), "Map is a tab again");
    assert!(!names(&more.sections().primary).contains(&"tasks"));
    assert_eq!(more.sections().landing, "review", "the template's landing stays");
    // An empty list under More brings every section back.
    let none = operations.with_overrides(&overrides(r#"{ "more": [] }"#).unwrap());
    assert!(none.sections().more.is_empty());
    assert_eq!(none.sections().primary.len(), 12);
}

#[test]
fn adjustments_follow_the_rules_of_the_blocks_they_adjust() {
    for (text, path) in [
        (r#"{ "landing": "dashboard" }"#, "overrides.landing"),
        (r#"{ "landing": "settings" }"#, "overrides.landing"),
        (r#"{ "more": ["map", "nowhere"] }"#, "overrides.more[1]"),
        (r#"{ "more": ["map", "map"] }"#, "overrides.more[1]"),
        (r#"{ "suggestions": [""] }"#, "overrides.suggestions[0]"),
        (r#"{ "suggestions": ["Same", "same"] }"#, "overrides.suggestions[1]"),
    ] {
        let issues = overrides(text).unwrap_err();
        assert_eq!(issues[0].path, path, "{text}");
    }
    let seven: Vec<String> = (0..=SUGGESTIONS_MAX).map(|n| format!("\"Question {n}\"")).collect();
    let too_many = overrides(&format!(r#"{{ "suggestions": [{}] }}"#, seven.join(","))).unwrap_err();
    assert_eq!(too_many[0].path, "overrides.suggestions");
    let unknown = overrides(r#"{ "ranking": {} }"#).unwrap_err();
    assert!(unknown[0].message.contains("unknown field `ranking`"), "{}", unknown[0]);
    assert!(Overrides::parse(&serde_json::json!("landing")).is_err());
}

#[test]
fn a_stored_adjustment_that_is_no_longer_valid_is_dropped_and_the_others_are_kept() {
    let stored = serde_json::json!({ "landing": "review", "more": ["map", "nowhere"], "suggestions": ["Who is on call?"], "colour": "red" });
    let kept = Overrides::from_stored(&stored);
    assert_eq!(kept.landing.as_deref(), Some("review"));
    assert_eq!(
        kept.more, None,
        "a list with a section this server lacks is dropped whole"
    );
    assert_eq!(kept.suggestions.as_deref(), Some(&["Who is on call?".to_owned()][..]));
    assert!(Overrides::from_stored(&serde_json::json!(null)).is_empty());
    assert!(Overrides::from_stored(&serde_json::json!("landing")).is_empty());
    assert!(Overrides::from_stored(&serde_json::json!({})).is_empty());
}

/// The valid file with a `ranking` block written as JSON.
fn with_ranking(block: &str) -> String {
    VALID.replacen("\"format\": 1,", &format!("\"format\": 1,\n  \"ranking\": {block},"), 1)
}

#[test]
fn search_ranks_as_it_always_has_unless_a_template_chooses() {
    assert_eq!(general().ranking().verified, DEFAULT_VERIFIED);
    assert_eq!(DEFAULT_VERIFIED, 1.1);
    assert_eq!(parse(VALID).unwrap().ranking().verified, 1.1);
    assert_eq!(parse(&with_ranking("{}")).unwrap().ranking().verified, 1.1);
    let chosen = parse(&with_ranking(r#"{ "verified": 1.15 }"#)).unwrap();
    assert_eq!(chosen.ranking().verified, 1.15);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("ranking"));
}

#[test]
fn a_factor_stays_between_eight_tenths_and_one_and_three_tenths() {
    for good in ["0.8", "1.0", "1.3", "0.95"] {
        assert!(
            parse(&with_ranking(&format!(r#"{{ "verified": {good} }}"#))).is_ok(),
            "{good} should be valid"
        );
    }
    for bad in ["0.79", "1.31", "0", "-1", "1.6", "100"] {
        let issues = parse(&with_ranking(&format!(r#"{{ "verified": {bad} }}"#))).unwrap_err();
        assert_eq!(issues[0].path, "ranking.verified", "{bad} should be refused");
    }
    assert!(parse(&with_ranking(r#"{ "verified": "high" }"#)).is_err());
    let unknown = parse(&with_ranking(r#"{ "colour": 1 }"#)).unwrap_err();
    assert!(unknown[0].message.contains("unknown field `colour`"), "{}", unknown[0]);
    assert_eq!((FACTOR_MIN, FACTOR_MAX), (0.8, 1.3));
}

fn boosts(list: &str) -> String {
    with_ranking(&format!(r#"{{ "boosts": {list} }}"#))
}

#[test]
fn a_template_lists_boosts_by_kind_of_evidence_kind_of_document_and_folder() {
    let template = parse(&boosts(
        r#"[ { "artifact_type": "runbook", "factor": 1.2 }, { "document_kind": "pdf", "factor": 1.15 }, { "path": "docs/adr/**", "factor": 1.15 } ]"#,
    ))
    .unwrap();
    let ranking = template.ranking();
    assert_eq!(ranking.boosts.len(), 3);
    assert_eq!(ranking.boosts[0].artifact_type.as_deref(), Some("runbook"));
    assert_eq!(ranking.boosts[1].document_kind.as_deref(), Some("pdf"));
    assert_eq!(ranking.boosts[2].path.as_deref(), Some("docs/adr/**"));
    assert_eq!(ranking.verified, DEFAULT_VERIFIED);
    assert_eq!(parse(&serde_json::to_string(&template).unwrap()).unwrap(), template);
    assert!(general().ranking().boosts.is_empty());
}

#[test]
fn a_boost_names_exactly_one_selector_that_exists_with_a_factor_in_bounds() {
    for (text, path) in [
        (r#"[ { "factor": 1.2 } ]"#, "ranking.boosts[0]"),
        (
            r#"[ { "artifact_type": "runbook", "path": "docs/**", "factor": 1.2 } ]"#,
            "ranking.boosts[0]",
        ),
        (
            r#"[ { "artifact_type": "spreadsheet", "factor": 1.2 } ]"#,
            "ranking.boosts[0].artifact_type",
        ),
        (
            r#"[ { "document_kind": "spreadsheet", "factor": 1.2 } ]"#,
            "ranking.boosts[0].document_kind",
        ),
        (
            r#"[ { "artifact_type": "runbook", "factor": 1.6 } ]"#,
            "ranking.boosts[0].factor",
        ),
        (
            r#"[ { "artifact_type": "runbook", "factor": 0.5 } ]"#,
            "ranking.boosts[0].factor",
        ),
        (r#"[ { "path": "(a|b).md", "factor": 1.2 } ]"#, "ranking.boosts[0].path"),
    ] {
        let issues = parse(&boosts(text)).unwrap_err();
        assert_eq!(issues[0].path, path, "{text}");
    }
    assert!(
        parse(&boosts(r#"[ { "artifact_type": "runbook" } ]"#)).is_err(),
        "a factor is required"
    );
    let unknown = parse(&boosts(
        r#"[ { "artifact_type": "runbook", "factor": 1.2, "colour": "red" } ]"#,
    ))
    .unwrap_err();
    assert!(unknown[0].message.contains("unknown field `colour`"), "{}", unknown[0]);
}

#[test]
fn at_most_sixteen_boosts_and_a_pattern_is_never_a_regular_expression() {
    let boost = r#"{ "artifact_type": "note", "factor": 1.1 }"#;
    let sixteen = vec![boost; BOOSTS_MAX].join(",");
    assert!(parse(&boosts(&format!("[{sixteen}]"))).is_ok());
    let seventeen = vec![boost; BOOSTS_MAX + 1].join(",");
    assert_eq!(
        parse(&boosts(&format!("[{seventeen}]"))).unwrap_err()[0].path,
        "ranking.boosts"
    );
    for good in [
        "docs/adr/**",
        "README*",
        "*.lock",
        "src/**/*.rs",
        "Meeting notes/*",
        "a-b_c.d/e@f+g",
    ] {
        assert_eq!(pattern_problem(good), None, "{good} should be valid");
    }
    let long = "x".repeat(121);
    for bad in [
        "",
        "/docs",
        "docs/",
        "docs//adr",
        "../secrets",
        "./x",
        "a**b",
        "(a|b)",
        "a?b",
        "[ab]",
        "{a,b}",
        "a^b",
        "a$",
        "back\\slash",
        "tab\there",
        long.as_str(),
        "a/b/c/d/e/f/g/h/i/j/k/l/m",
    ] {
        assert!(pattern_problem(bad).is_some(), "{bad:?} should be refused");
    }
    assert_eq!(PRODUCT_MAX, 1.5);
}

#[test]
fn the_four_first_templates_boost_what_their_teams_look_for() {
    let catalogue = Catalogue::builtin();
    let boosts = |id: &str| catalogue.get(id).unwrap().ranking().boosts;
    let operations = boosts("operations");
    assert_eq!(
        operations
            .iter()
            .filter_map(|boost| boost.artifact_type.as_deref())
            .collect::<Vec<_>>(),
        ["runbook", "incident"]
    );
    assert!(operations.iter().all(|boost| boost.factor == 1.2));
    let software = boosts("software-engineering");
    assert!(software
        .iter()
        .any(|boost| boost.path.as_deref() == Some("docs/adr/**") && boost.factor == 1.15));
    assert!(software.iter().any(|boost| boost.path.as_deref() == Some("README*")));
    assert!(software
        .iter()
        .any(|boost| boost.artifact_type.as_deref() == Some("api_spec")));
    let documents = boosts("knowledge-base");
    assert_eq!(
        documents
            .iter()
            .filter_map(|boost| boost.document_kind.as_deref())
            .collect::<Vec<_>>(),
        ["word", "pdf", "excel", "powerpoint"]
    );
    assert!(boosts("product-decisions")
        .iter()
        .any(|boost| boost.document_kind.as_deref() == Some("markdown")));
}

#[test]
fn a_template_may_prefer_recent_evidence_and_it_is_off_without_the_block() {
    assert!(general().ranking().recency.is_none());
    assert!(parse(&with_ranking("{}")).unwrap().ranking().recency.is_none());
    let template = parse(&with_ranking(
        r#"{ "recency": { "half_life_days": 90, "max_factor": 1.2 } }"#,
    ))
    .unwrap();
    let recency = template.ranking().recency.expect("the block was given");
    assert_eq!((recency.half_life_days, recency.max_factor), (90, 1.2));
    assert_eq!(parse(&serde_json::to_string(&template).unwrap()).unwrap(), template);
    assert!(!serde_json::to_string(&general()).unwrap().contains("recency"));
}

#[test]
fn the_half_life_and_the_most_a_file_may_be_favoured_by_stay_within_bounds() {
    for good in [(HALF_LIFE_MIN, 1.2), (HALF_LIFE_MAX, 1.01), (90, 1.2)] {
        let block = format!(
            r#"{{ "recency": {{ "half_life_days": {}, "max_factor": {} }} }}"#,
            good.0, good.1
        );
        assert!(parse(&with_ranking(&block)).is_ok(), "{good:?} should be valid");
    }
    for (days, factor, path) in [
        (HALF_LIFE_MIN - 1, "1.2", "ranking.recency.half_life_days"),
        (HALF_LIFE_MAX + 1, "1.2", "ranking.recency.half_life_days"),
        (90, "1.0", "ranking.recency.max_factor"),
        (90, "0.9", "ranking.recency.max_factor"),
        (90, "1.21", "ranking.recency.max_factor"),
    ] {
        let block = format!(r#"{{ "recency": {{ "half_life_days": {days}, "max_factor": {factor} }} }}"#);
        assert_eq!(
            parse(&with_ranking(&block)).unwrap_err()[0].path,
            path,
            "{days} {factor}"
        );
    }
    assert!(
        parse(&with_ranking(r#"{ "recency": { "half_life_days": 90 } }"#)).is_err(),
        "both keys are required"
    );
    assert!(
        parse(&with_ranking(
            r#"{ "recency": { "half_life_days": 90.5, "max_factor": 1.1 } }"#
        ))
        .is_err(),
        "whole days"
    );
    assert_eq!(RECENCY_MAX, 1.2);
}

/// The valid file with a `health` block written as JSON.
fn with_health(block: &str) -> String {
    VALID.replacen("\"format\": 1,", &format!("\"format\": 1,\n  \"health\": {block},"), 1)
}

#[test]
fn every_health_check_runs_in_the_usual_order_unless_a_template_chooses() {
    // The seven checks Health has always had; the check for stale evidence runs only where a template names it.
    assert_eq!(general().health_detectors(), DEFAULT_DETECTORS);
    assert_eq!(parse(VALID).unwrap().health_detectors(), DEFAULT_DETECTORS);
    assert!(!DEFAULT_DETECTORS.contains(&"stale_evidence") && HEALTH_DETECTORS.contains(&"stale_evidence"));
    assert_eq!(&HEALTH_DETECTORS[..7], DEFAULT_DETECTORS.as_slice());
    assert_eq!(general().stale_after_days(), None);
    let chosen = parse(&with_health(
        r#"{ "detectors": ["outdated_evidence_referenced", "stale_memory_citation"] }"#,
    ))
    .unwrap();
    assert_eq!(
        chosen.health_detectors(),
        ["outdated_evidence_referenced", "stale_memory_citation"]
    );
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("health"));
    assert_eq!(HEALTH_DETECTORS.len(), 8);
}

#[test]
fn a_health_check_must_exist_be_listed_once_and_at_least_one_must_run() {
    let unknown = parse(&with_health(
        r#"{ "detectors": ["outdated_evidence_referenced", "mind_reading"] }"#,
    ))
    .unwrap_err();
    assert_eq!(unknown[0].path, "health.detectors[1]");
    let twice = parse(&with_health(r#"{ "detectors": ["unconnected", "unconnected"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "health.detectors[1]");
    assert_eq!(
        parse(&with_health(r#"{ "detectors": [] }"#)).unwrap_err()[0].path,
        "health.detectors"
    );
    assert_eq!(parse(&with_health("{}")).unwrap_err()[0].path, "health.detectors");
    let unknown_key = parse(&with_health(r#"{ "detectors": ["unconnected"], "colour": "red" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn the_check_for_stale_evidence_needs_its_days_and_the_days_need_the_check() {
    let ok = parse(&with_health(
        r#"{ "detectors": ["stale_evidence", "index_failed"], "stale_after_days": 90 }"#,
    ))
    .unwrap();
    assert_eq!(ok.stale_after_days(), Some(90));
    assert_eq!(parse(&serde_json::to_string(&ok).unwrap()).unwrap(), ok);
    for (block, path) in [
        (r#"{ "detectors": ["stale_evidence"] }"#, "health.stale_after_days"),
        (
            r#"{ "detectors": ["index_failed"], "stale_after_days": 90 }"#,
            "health.stale_after_days",
        ),
        (
            r#"{ "detectors": ["stale_evidence"], "stale_after_days": 6 }"#,
            "health.stale_after_days",
        ),
        (
            r#"{ "detectors": ["stale_evidence"], "stale_after_days": 3651 }"#,
            "health.stale_after_days",
        ),
    ] {
        assert_eq!(parse(&with_health(block)).unwrap_err()[0].path, path, "{block}");
    }
    for days in [STALE_DAYS_MIN, STALE_DAYS_MAX] {
        let block = format!(r#"{{ "detectors": ["stale_evidence"], "stale_after_days": {days} }}"#);
        assert!(parse(&with_health(&block)).is_ok(), "{days}");
    }
    assert!(
        parse(&with_health(
            r#"{ "detectors": ["stale_evidence"], "stale_after_days": 90.5 }"#
        ))
        .is_err(),
        "whole days"
    );
}

/// The valid file with a `review` block written as JSON.
fn with_review(block: &str) -> String {
    VALID.replacen("\"format\": 1,", &format!("\"format\": 1,\n  \"review\": {block},"), 1)
}

/// The valid file with a `map` block written as JSON.
fn with_map(block: &str) -> String {
    VALID.replacen("\"format\": 1,", &format!("\"format\": 1,\n  \"map\": {block},"), 1)
}

#[test]
fn the_map_puts_nothing_first_unless_a_template_chooses() {
    assert_eq!(general().map_emphasis(), MapBlock::default());
    assert_eq!(parse(VALID).unwrap().map_emphasis(), MapBlock::default());
    assert_eq!(parse(&with_map("{}")).unwrap().map_emphasis(), MapBlock::default());
    let chosen = parse(&with_map(
        r#"{ "coverage_first": ["repository", "code"], "relations_first": ["cites"] }"#,
    ))
    .unwrap();
    assert_eq!(chosen.map_emphasis().coverage_first, ["repository", "code"]);
    assert_eq!(chosen.map_emphasis().relations_first, ["cites"]);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("\"map\""));
    assert_eq!(MAP_RELATIONS, ["similar", "cites"]);
}

#[test]
fn a_map_emphasis_names_known_file_types_and_links_once_each() {
    let unknown_type = parse(&with_map(r#"{ "coverage_first": ["code", "spreadsheet"] }"#)).unwrap_err();
    assert_eq!(unknown_type[0].path, "map.coverage_first[1]");
    let repeated = parse(&with_map(r#"{ "coverage_first": ["code", "code"] }"#)).unwrap_err();
    assert_eq!(repeated[0].path, "map.coverage_first[1]");
    let unknown_link = parse(&with_map(r#"{ "relations_first": ["similar", "mentions"] }"#)).unwrap_err();
    assert_eq!(unknown_link[0].path, "map.relations_first[1]");
    let repeated_link = parse(&with_map(r#"{ "relations_first": ["cites", "cites"] }"#)).unwrap_err();
    assert_eq!(repeated_link[0].path, "map.relations_first[1]");
    let unknown_key = parse(&with_map(r#"{ "colour": "red" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `colour`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn software_engineering_puts_repositories_and_code_first_on_its_map() {
    let catalogue = Catalogue::builtin();
    let software = catalogue.get("software-engineering").unwrap().map_emphasis();
    assert_eq!(software.coverage_first, ["repository", "code"]);
    assert_eq!(
        catalogue.get("knowledge-base").unwrap().map_emphasis().coverage_first[0],
        "word"
    );
    for id in [
        "software-engineering",
        "operations",
        "product-decisions",
        "knowledge-base",
    ] {
        let map = catalogue.get(id).unwrap().map_emphasis();
        assert!(
            !map.coverage_first.is_empty() && !map.relations_first.is_empty(),
            "{id}"
        );
    }
}

/// The valid file written for another template: its id and name replaced.
fn custom(id: &str, name: &str) -> String {
    VALID
        .replace("\"operations\"", &format!("\"{id}\""))
        .replace("Operations and incidents", name)
}

#[test]
fn files_added_to_the_built_in_templates_are_custom_and_sorted_among_them() {
    let builtin = Catalogue::builtin();
    let support = custom("support", "Customer support");
    let zebra = custom("zebra", "Zebra herding");
    let (extended, problems) = builtin.extended([("zebra.json", zebra.as_str()), ("support.json", support.as_str())]);
    assert!(problems.is_empty(), "{problems:?}");
    let ids: Vec<&str> = extended.list().iter().map(TemplateDefinition::id).collect();
    assert_eq!(ids[0], GENERAL_ID);
    assert!(ids.contains(&"operations") && ids.contains(&"support") && ids.contains(&"zebra"));
    // Built-in and custom templates are listed together, by name.
    let names: Vec<String> = extended.list()[1..]
        .iter()
        .map(|template| template.meta.name.to_lowercase())
        .collect();
    assert!(names.windows(2).all(|pair| pair[0] <= pair[1]), "{names:?}");
    assert!(extended.is_custom("support") && extended.is_custom("zebra"));
    assert!(!extended.is_custom("operations") && !extended.is_custom(GENERAL_ID));
    assert!(
        !builtin.is_custom("support") && builtin.get("support").is_none(),
        "the built-in catalogue is not changed"
    );
    // The same files in another order give the same catalogue.
    let (other_order, _) = builtin.extended([("support.json", support.as_str()), ("zebra.json", zebra.as_str())]);
    assert_eq!(extended, other_order);
    // Adding nothing leaves the catalogue as it was.
    assert_eq!(builtin.extended([]).0, *builtin);
}

#[test]
fn a_file_cannot_take_the_id_of_a_built_in_template_or_general_and_is_named_when_refused() {
    let builtin = Catalogue::builtin();
    let shadow = custom("operations", "Operations, my way");
    let general_text = custom(GENERAL_ID, "General, my way");
    let renamed = custom("support", "Customer support");
    let broken = VALID
        .replace("\"format\": 1,", "\"format\": 1, \"colour\": \"red\",")
        .replace("operations", "other");
    let (extended, problems) = builtin.extended([
        ("operations.json", shadow.as_str()),
        ("general.json", general_text.as_str()),
        ("some-other-name.json", renamed.as_str()),
        ("other.json", broken.as_str()),
    ]);
    let by_file = |file: &str| problems.iter().find(|problem| problem.source == file).unwrap();
    assert!(by_file("operations.json").issues[0]
        .message
        .contains("built into this server"));
    assert!(by_file("general.json").issues[0].message.contains("cannot be replaced"));
    assert!(by_file("some-other-name.json").issues[0]
        .message
        .contains("named after its id"));
    assert!(by_file("other.json").issues[0].message.contains("colour"));
    assert_eq!(problems.len(), 4);
    // Nothing was added and the built-in templates are as they were.
    assert_eq!(extended, *builtin);
    assert_eq!(extended.get("operations"), builtin.get("operations"));
    // Two files cannot share an id; the first by name is kept.
    let (twice, repeated) = builtin.extended([("support.json", renamed.as_str()), ("support.json", renamed.as_str())]);
    assert_eq!(repeated.len(), 1);
    assert!(twice.is_custom("support"));
}

/// A folder of its own under the temporary folder.
fn scratch_folder(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let folder = std::env::temp_dir().join(format!(
        "repomemo-templates-{label}-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn a_folder_gives_its_json_files_in_name_order_and_leaves_the_rest_alone() {
    let folder = scratch_folder("read");
    std::fs::write(folder.join("b-team.json"), custom("b-team", "B team")).unwrap();
    std::fs::write(folder.join("a-team.json"), custom("a-team", "A team")).unwrap();
    std::fs::write(folder.join("_example.json"), custom("example", "Example")).unwrap();
    std::fs::write(folder.join("notes.txt"), "not a template").unwrap();
    std::fs::create_dir(folder.join("nested.json")).unwrap();
    std::fs::write(folder.join("nested.json").join("deep.json"), custom("deep", "Deep")).unwrap();
    let read = read_folder(&folder);
    assert!(read.problems.is_empty(), "{:?}", read.problems);
    let names: Vec<&str> = read.sources.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["a-team.json", "b-team.json"]);
    assert!(read.sources[0].1.contains("a-team"));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_folder_that_cannot_be_read_is_a_problem_and_never_an_error() {
    let missing = std::env::temp_dir().join(format!("repomemo-templates-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    let read = read_folder(&missing);
    assert!(read.sources.is_empty());
    assert_eq!(read.problems.len(), 1);
    assert!(read.problems[0].issues[0].message.contains("cannot be read"));
    // A file where a folder should be is the same.
    let folder = scratch_folder("file");
    let file = folder.join("not-a-folder");
    std::fs::write(&file, "x").unwrap();
    assert_eq!(read_folder(&file).problems.len(), 1);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_file_over_the_size_limit_is_left_out_and_named_and_the_others_are_read() {
    let folder = scratch_folder("size");
    std::fs::write(folder.join("big.json"), vec![b' '; limits::FILE_BYTES + 1]).unwrap();
    std::fs::write(folder.join("just-fits.json"), vec![b' '; limits::FILE_BYTES]).unwrap();
    std::fs::write(folder.join("fine.json"), custom("fine", "Fine")).unwrap();
    std::fs::write(folder.join("latin.json"), [0xff, 0xfe, 0x00]).unwrap();
    let read = read_folder(&folder);
    let names: Vec<&str> = read.sources.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["fine.json", "just-fits.json"]);
    let problems: Vec<(&str, &str)> = read
        .problems
        .iter()
        .map(|problem| (problem.source.as_str(), problem.issues[0].message.as_str()))
        .collect();
    assert_eq!(problems.len(), 2);
    assert!(problems
        .iter()
        .any(|(file, message)| *file == "big.json" && message.contains("at most")));
    assert!(problems
        .iter()
        .any(|(file, message)| *file == "latin.json" && message.contains("UTF-8")));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_folder_gives_at_most_sixty_four_files() {
    let folder = scratch_folder("count");
    for number in 0..FOLDER_FILES_MAX + 3 {
        std::fs::write(folder.join(format!("t{number:03}.json")), "{}").unwrap();
    }
    let read = read_folder(&folder);
    assert_eq!(read.sources.len(), FOLDER_FILES_MAX);
    assert_eq!(read.problems.len(), 3);
    assert!(read
        .problems
        .iter()
        .all(|problem| problem.issues[0].message.contains("more than 64")));
    assert_eq!(read.problems[0].source, "t064.json");
    let _ = std::fs::remove_dir_all(&folder);
}

/// V0.2.190: the console checks one file with the rules the folder's files are read by.
#[test]
fn a_single_file_is_read_the_way_the_folders_files_are() {
    let folder = scratch_folder("single");
    let file = folder.join("support.json");
    std::fs::write(&file, custom("support", "Customer support")).unwrap();
    let read = read_template_file(&file);
    assert!(read.problems.is_empty(), "{:?}", read.problems);
    assert_eq!(read.sources.len(), 1);
    assert_eq!(read.sources[0].0, "support.json");
    // A name that starts with _ is still read: whether the server loads it is the catalogue's business, not the reader's.
    let underscored = folder.join("_support.json");
    std::fs::write(&underscored, custom("support", "Customer support")).unwrap();
    assert_eq!(read_template_file(&underscored).sources.len(), 1);
    // What the folder would leave out is named: a folder, a missing file, a file that is too big or not text.
    let nested = folder.join("nested.json");
    std::fs::create_dir(&nested).unwrap();
    assert!(read_template_file(&nested).problems[0].issues[0]
        .message
        .contains("not a file"));
    assert!(read_template_file(&folder.join("missing.json")).problems[0].issues[0]
        .message
        .contains("cannot be read"));
    let big = folder.join("big.json");
    std::fs::write(&big, vec![b' '; limits::FILE_BYTES + 1]).unwrap();
    assert!(read_template_file(&big).problems[0].issues[0]
        .message
        .contains("at most"));
    let text = folder.join("text.json");
    std::fs::write(&text, [0xff, 0xfe]).unwrap();
    assert!(read_template_file(&text).problems[0].issues[0]
        .message
        .contains("UTF-8"));
    let _ = std::fs::remove_dir_all(&folder);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_folder_is_not_followed() {
    let folder = scratch_folder("link");
    let elsewhere = scratch_folder("link-target");
    std::fs::write(elsewhere.join("outside.json"), custom("outside", "Outside")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("outside.json"), folder.join("outside.json")).unwrap();
    let read = read_folder(&folder);
    assert!(read.sources.is_empty());
    assert_eq!(read.problems.len(), 1);
    assert!(read.problems[0].issues[0].message.contains("link"));
    let _ = std::fs::remove_dir_all(&folder);
    let _ = std::fs::remove_dir_all(&elsewhere);
}

/// V0.2.192: what a message quotes from a file is made safe to show.
#[test]
fn a_message_that_quotes_the_file_carries_no_control_character_and_is_not_long() {
    let nasty_key = "bad\\n::error::forged\\u001b[2Jkey\\u202e";
    let issues = parse(&VALID.replace("\"format\": 1,", &format!("\"format\": 1, \"{nasty_key}\": 1,"))).unwrap_err();
    let shown = issues[0].to_string();
    assert!(shown.contains("unknown field"), "{shown}");
    assert!(
        !shown
            .chars()
            .any(|character| character.is_control() || character == '\u{202e}'),
        "{shown:?}"
    );
    assert!(shown.contains("\\n") || shown.contains("\\u{1b}"), "{shown:?}");
    // A word of a list that is a megabyte long is quoted cut off.
    let long = "x".repeat(5_000);
    let issues = parse(&with_meta("icon", &format!("\"{long}\""))).unwrap_err();
    assert!(
        issues[0].message.chars().count() <= 320,
        "{}",
        issues[0].message.chars().count()
    );
    assert!(issues[0].message.ends_with('…'));
    // The name of a file is cleaned in the same way, in the problems of a catalogue and of a folder.
    let (_, problems) =
        Catalogue::builtin().extended([("a\u{1b}[2Jb.json", VALID.replace("operations", "other").as_str())]);
    assert_eq!(problems.len(), 1);
    assert!(
        !problems[0].source.chars().any(char::is_control),
        "{:?}",
        problems[0].source
    );
    assert!(problems[0].source.contains("\\u{1b}"));
    // Plain text is left as it is.
    assert_eq!(
        TemplateIssue::new("meta.id", "`x` is not a section — try \"y\"").message,
        "`x` is not a section — try \"y\""
    );
}

/// The example a template author copies (V0.2.188): valid, using every block, and not one of the built-in templates.
#[test]
fn the_example_is_valid_uses_every_block_and_is_not_a_built_in_template() {
    let example = parse(include_str!("../definitions/_example.json")).unwrap();
    assert_eq!(example.id(), "example");
    assert!(Catalogue::builtin().get("example").is_none());
    // Listing every field without `..` stops this compiling when a block is added to the format, until the example
    // (and this list) has it too.
    let TemplateDefinition {
        format: _,
        meta: _,
        starter_folders,
        sections,
        labels,
        overview,
        evidence,
        assistant,
        retrieval,
        ranking,
        health,
        review,
        map,
        repositories,
    } = example;
    assert!(!starter_folders.is_empty());
    assert!(sections.is_some() && labels.is_some() && overview.is_some() && evidence.is_some());
    assert!(assistant.is_some() && retrieval.is_some() && ranking.is_some() && health.is_some());
    assert!(review.is_some() && map.is_some() && repositories.is_some());
}

#[test]
fn a_template_names_a_headline_for_the_dashboard_from_the_cards_one_count_can_answer() {
    assert_eq!(general().overview_headline(), None);
    let chosen = parse(&with_overview(
        r#"{ "cards": ["sources"], "headline": "code_symbols" }"#,
    ))
    .unwrap();
    assert_eq!(chosen.overview_headline().as_deref(), Some("code_symbols"));
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    // Without one the card carries the name only, and nothing is written for it.
    let without = parse(&with_overview(r#"{ "cards": ["sources"] }"#)).unwrap();
    assert_eq!(without.overview_headline(), None);
    assert!(!serde_json::to_string(&without).unwrap().contains("headline"));
    for good in HEADLINE_CARDS {
        assert!(
            parse(&with_overview(&format!(
                r#"{{ "cards": ["sources"], "headline": "{good}" }}"#
            )))
            .is_ok(),
            "{good}"
        );
    }
    // Every headline is a pulse card, but not every pulse card can be one: the dashboard counts them for all of its
    // workspaces at once.
    assert!(HEADLINE_CARDS.iter().all(|card| PULSE_CARDS.contains(card)));
    for bad in ["index_coverage", "no_such_card", ""] {
        let issues = parse(&with_overview(&format!(
            r#"{{ "cards": ["sources"], "headline": "{bad}" }}"#
        )))
        .unwrap_err();
        assert_eq!(issues[0].path, "overview.headline", "{bad:?}");
    }
}

#[test]
fn the_four_first_templates_each_put_one_countable_figure_on_their_workspace_cards() {
    let catalogue = Catalogue::builtin();
    let headline = |id: &str| catalogue.get(id).unwrap().overview_headline();
    assert_eq!(headline("software-engineering").as_deref(), Some("code_symbols"));
    assert_eq!(headline("operations").as_deref(), Some("active_tasks"));
    assert_eq!(headline("product-decisions").as_deref(), Some("memory_cards"));
    assert_eq!(headline("knowledge-base").as_deref(), Some("sources"));
}

/// The valid file with a `repositories` block written as JSON.
fn with_repositories(block: &str) -> String {
    VALID.replacen(
        "\"format\": 1,",
        &format!("\"format\": 1,\n  \"repositories\": {block},"),
        1,
    )
}

#[test]
fn the_link_form_is_offered_no_rules_unless_a_template_chooses() {
    assert_eq!(general().repository_rules(), RepositoriesBlock::default());
    assert_eq!(parse(VALID).unwrap().repository_rules(), RepositoriesBlock::default());
    assert_eq!(
        parse(&with_repositories("{}")).unwrap().repository_rules(),
        RepositoriesBlock::default()
    );
    let chosen = parse(&with_repositories(
        r#"{ "include": ["docs/**", "**/*.md"], "exclude": ["**/__snapshots__/**", "*.snap"] }"#,
    ))
    .unwrap();
    assert_eq!(chosen.repository_rules().include, ["docs/**", "**/*.md"]);
    assert_eq!(chosen.repository_rules().exclude, ["**/__snapshots__/**", "*.snap"]);
    assert_eq!(parse(&serde_json::to_string(&chosen).unwrap()).unwrap(), chosen);
    assert!(!serde_json::to_string(&general()).unwrap().contains("repositories"));
}

#[test]
fn a_repository_pattern_is_a_folder_pattern_listed_once_and_there_are_at_most_sixteen() {
    let include = |pattern: &str| with_repositories(&format!(r#"{{ "include": ["{pattern}"] }}"#));
    for good in ["docs/**", "**/*.md", "*.tfstate", "src/**/generated", "Dockerfile"] {
        assert!(parse(&include(good)).is_ok(), "{good}");
    }
    // Never a regular expression, and the same grammar as the ranking's folders.
    for bad in [
        "",
        "(a|b)",
        "docs/",
        "/docs",
        "../secrets",
        "a//b",
        "src/**x",
        "[ab].rs",
        "a\\\\b",
    ] {
        assert_eq!(
            parse(&include(bad)).unwrap_err()[0].path,
            "repositories.include[0]",
            "{bad:?}"
        );
    }
    let exclude = parse(&with_repositories(r#"{ "exclude": ["*.log", "build/*", "(x)"] }"#)).unwrap_err();
    assert_eq!(exclude.len(), 1);
    assert_eq!(exclude[0].path, "repositories.exclude[2]");
    let twice = parse(&with_repositories(r#"{ "include": ["docs/**", "docs/**"] }"#)).unwrap_err();
    assert_eq!(twice[0].path, "repositories.include[1]");
    // The same pattern may stand in both lists: they say different things.
    assert!(parse(&with_repositories(
        r#"{ "include": ["docs/**"], "exclude": ["docs/**"] }"#
    ))
    .is_ok());
    let patterns = |count: usize| {
        (0..count)
            .map(|n| format!("\"dir{n}/**\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    assert!(parse(&with_repositories(&format!(
        r#"{{ "exclude": [{}] }}"#,
        patterns(REPOSITORY_PATTERNS_MAX)
    )))
    .is_ok());
    let many = parse(&with_repositories(&format!(
        r#"{{ "exclude": [{}] }}"#,
        patterns(REPOSITORY_PATTERNS_MAX + 1)
    )))
    .unwrap_err();
    assert_eq!(many[0].path, "repositories.exclude");
    let unknown_key = parse(&with_repositories(r#"{ "branch": "main" }"#)).unwrap_err();
    assert!(
        unknown_key[0].message.contains("unknown field `branch`"),
        "{}",
        unknown_key[0]
    );
}

#[test]
fn the_four_first_templates_offer_rules_that_fit_the_repositories_their_teams_link() {
    let catalogue = Catalogue::builtin();
    let rules = |id: &str| catalogue.get(id).unwrap().repository_rules();
    // Code teams leave out generated and snapshot files, and keep everything else.
    assert!(rules("software-engineering").include.is_empty());
    assert!(rules("software-engineering")
        .exclude
        .iter()
        .any(|pattern| pattern == "*.snap"));
    // Operations never indexes Terraform state, which can hold secrets.
    assert!(rules("operations").exclude.iter().any(|pattern| pattern == "*.tfstate"));
    // Documents-first teams link repositories for their documents.
    assert!(rules("product-decisions")
        .include
        .iter()
        .any(|pattern| pattern == "**/*.md"));
    assert!(rules("knowledge-base")
        .include
        .iter()
        .any(|pattern| pattern == "**/*.pdf"));
    for id in [
        "software-engineering",
        "operations",
        "product-decisions",
        "knowledge-base",
    ] {
        let rules = rules(id);
        assert!(!rules.include.is_empty() || !rules.exclude.is_empty(), "{id}");
        assert!(
            rules.include.len() <= REPOSITORY_PATTERNS_MAX && rules.exclude.len() <= REPOSITORY_PATTERNS_MAX,
            "{id}"
        );
    }
}

#[test]
fn the_review_queue_is_mixed_unless_a_template_names_the_kinds_to_list_first() {
    assert!(general().review_order().is_empty());
    assert!(parse(VALID).unwrap().review_order().is_empty());
    // A kind the template does not name follows the named ones, in the usual order.
    let health_first = parse(&with_review(r#"{ "order": ["health_finding"] }"#)).unwrap();
    assert_eq!(
        health_first.review_order(),
        ["health_finding", "evidence", "memory_card"]
    );
    let cards_then_files = parse(&with_review(r#"{ "order": ["memory_card", "evidence"] }"#)).unwrap();
    assert_eq!(
        cards_then_files.review_order(),
        ["memory_card", "evidence", "health_finding"]
    );
    assert_eq!(
        parse(&serde_json::to_string(&cards_then_files).unwrap()).unwrap(),
        cards_then_files
    );
    assert!(!serde_json::to_string(&general()).unwrap().contains("review"));
    assert_eq!(REVIEW_SOURCES, ["evidence", "memory_card", "health_finding"]);
}

#[test]
fn a_review_kind_must_exist_be_listed_once_and_at_least_one_must_be_named() {
    assert_eq!(
        parse(&with_review(r#"{ "order": ["files"] }"#)).unwrap_err()[0].path,
        "review.order[0]"
    );
    assert_eq!(
        parse(&with_review(r#"{ "order": ["evidence", "evidence"] }"#)).unwrap_err()[0].path,
        "review.order[1]"
    );
    assert_eq!(
        parse(&with_review(r#"{ "order": [] }"#)).unwrap_err()[0].path,
        "review.order"
    );
    assert_eq!(parse(&with_review("{}")).unwrap_err()[0].path, "review.order");
    let unknown = parse(&with_review(r#"{ "order": ["evidence"], "colour": "red" }"#)).unwrap_err();
    assert!(unknown[0].message.contains("unknown field `colour`"), "{}", unknown[0]);
}

#[test]
fn operations_lists_health_findings_first_and_software_engineering_keeps_the_mixed_list() {
    let catalogue = Catalogue::builtin();
    assert_eq!(
        catalogue.get("operations").unwrap().review_order(),
        ["health_finding", "evidence", "memory_card"]
    );
    assert_eq!(
        catalogue.get("product-decisions").unwrap().review_order()[0],
        "memory_card"
    );
    assert_eq!(catalogue.get("knowledge-base").unwrap().review_order()[0], "evidence");
    assert!(catalogue.get("software-engineering").unwrap().review_order().is_empty());
}

#[test]
fn operations_and_the_knowledge_base_check_for_stale_evidence_and_the_others_do_not() {
    let catalogue = Catalogue::builtin();
    let operations = catalogue.get("operations").unwrap();
    assert_eq!(operations.health_detectors()[0], "stale_evidence", "first in the list");
    assert_eq!(operations.stale_after_days(), Some(90));
    let knowledge = catalogue.get("knowledge-base").unwrap();
    assert_eq!(knowledge.stale_after_days(), Some(180));
    assert!(knowledge.health_detectors().contains(&"stale_evidence".to_owned()));
    for id in ["software-engineering", "product-decisions"] {
        let template = catalogue.get(id).unwrap();
        assert!(
            !template.health_detectors().contains(&"stale_evidence".to_owned()),
            "{id}"
        );
        assert_eq!(template.stale_after_days(), None, "{id}");
    }
}

#[test]
fn the_four_first_templates_run_the_checks_their_teams_need_and_keep_indexing_failures() {
    let catalogue = Catalogue::builtin();
    let checks = |id: &str| catalogue.get(id).unwrap().health_detectors();
    assert_eq!(
        checks("software-engineering"),
        [
            "removed_symbol_mentioned",
            "outdated_evidence_referenced",
            "stale_memory_citation",
            "index_failed"
        ]
    );
    assert_eq!(
        checks("operations"),
        [
            "stale_evidence",
            "outdated_evidence_referenced",
            "stale_memory_citation",
            "index_failed"
        ]
    );
    assert_eq!(
        checks("product-decisions"),
        [
            "outdated_evidence_referenced",
            "older_version_active",
            "stale_memory_citation",
            "index_failed"
        ]
    );
    assert_eq!(
        checks("knowledge-base"),
        [
            "older_version_active",
            "duplicate_content",
            "stale_evidence",
            "outdated_evidence_referenced",
            "index_failed"
        ]
    );
    for id in [
        "software-engineering",
        "operations",
        "product-decisions",
        "knowledge-base",
    ] {
        assert!(
            checks(id).contains(&"index_failed".to_owned()),
            "{id}: a file search cannot see is always reported"
        );
    }
}

#[test]
fn only_operations_prefers_recent_evidence() {
    let catalogue = Catalogue::builtin();
    let recency = catalogue
        .get("operations")
        .unwrap()
        .ranking()
        .recency
        .expect("Operations prefers recent runbooks");
    assert_eq!((recency.half_life_days, recency.max_factor), (90, 1.2));
    for id in ["software-engineering", "product-decisions", "knowledge-base"] {
        assert!(catalogue.get(id).unwrap().ranking().recency.is_none(), "{id}");
    }
}

#[test]
fn operations_and_the_knowledge_base_favour_verified_evidence_more() {
    let catalogue = Catalogue::builtin();
    let verified = |id: &str| catalogue.get(id).unwrap().ranking().verified;
    assert_eq!(verified("operations"), 1.15);
    assert_eq!(verified("knowledge-base"), 1.15);
    assert_eq!(verified("software-engineering"), 1.1);
    assert_eq!(verified("product-decisions"), 1.1);
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
