//! CSV migration (V0.7.18): tasks out as a CSV a spreadsheet opens safely, and a CSV in with a mapping and a report that explains
//! every row before anything is written; a retry creates nothing twice.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

async fn body_text(response: axum::response::Response) -> String {
    String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
}

#[tokio::test]
async fn tasks_go_out_as_safe_csv_and_come_in_with_a_report_and_no_duplicates() {
    let fixture = knowledge_fixture("task-csv").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let count_tasks = || {
        let call = &call;
        let uri = tasks.clone();
        let who = fixture.owner.clone();
        async move {
            response_json(call("GET", uri, who, Value::Null).await)
                .await
                .as_array()
                .unwrap()
                .len()
        }
    };
    let project = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/projects"),
            fixture.member.clone(),
            json!({"name": "Queue migration"}),
        )
        .await,
    )
    .await;
    call(
        "POST",
        format!("/v1/workspaces/{workspace}/task-labels"),
        fixture.member.clone(),
        json!({"name": "Bug", "color": "red"}),
    )
    .await;

    // Export: a spreadsheet-safe CSV. A formula, a comma, a quote and a line break are all text.
    for (title, description) in [
        ("=HYPERLINK(\"http://example.com\",\"click\")", "Looks like a formula."),
        ("Plain, with a comma", "Two\nlines and a \"quote\"."),
        ("+cmd and @mention", ""),
    ] {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "description": description, "status": "open", "priority": "high", "project_id": project["id"]}),
        )
        .await;
    }
    let exported = call("GET", format!("{tasks}/export.csv"), fixture.guest.clone(), Value::Null).await;
    assert_eq!(exported.status(), 200);
    assert_eq!(exported.headers()["content-type"], "text/csv; charset=utf-8");
    assert!(exported.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains("attachment"));
    let csv = body_text(exported).await;
    assert!(
        csv.starts_with('\u{feff}'),
        "a byte order mark so a spreadsheet reads it as UTF-8"
    );
    assert!(
        csv.contains("\"'=HYPERLINK(\"\"http://example.com\"\",\"\"click\"\")\""),
        "{csv}"
    );
    assert!(csv.contains("\"Plain, with a comma\""));
    assert!(csv.contains("\"Two\nlines and a \"\"quote\"\".\""));
    assert!(csv.contains("\"'+cmd and @mention\""));
    assert!(
        !csv.lines()
            .any(|line| line.starts_with('=') || line.starts_with('+') || line.contains(",=")),
        "no cell starts as a formula"
    );
    let only_open = body_text(
        call(
            "GET",
            format!("{tasks}/export.csv?status=done"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        only_open.trim_start_matches('\u{feff}').lines().count(),
        1,
        "a filter exports only what matches: the header"
    );
    let (stranger, _) = register_account(&app, "task-csv-stranger@example.com").await;
    assert_eq!(
        call("GET", format!("{tasks}/export.csv"), stranger, Value::Null)
            .await
            .status(),
        403
    );

    // Taken out and brought back into the same workspace, every row is already there.
    let before = count_tasks().await;
    let roundtrip = response_json(
        call(
            "POST",
            format!("{tasks}/import"),
            fixture.member.clone(),
            json!({"csv": csv, "dry_run": false}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (roundtrip["created"].as_i64(), roundtrip["duplicates"].as_i64()),
        (Some(0), Some(3))
    );
    assert_eq!(count_tasks().await, before);

    // A file from another tool, with its own column names, mapped by the caller; some rows are fine, some are not.
    let source = "Issue ID,Summary,Details,State,Owner email,Due,Area,Tags\r\n\
        JIRA-1,Backfill the topic,\"Two lines\nin one cell\",In Progress,task-csv-member@example.com,2027-02-01,Queue migration,Bug\r\n\
        JIRA-2,,No title here,Open,,,,\r\n\
        JIRA-3,Ask the vendor,,Waiting,nobody@example.com,,,\r\n\
        JIRA-4,Cut over,,Done,,not a date,,\r\n\
        JIRA-5,Tidy the docs,,Open,,,Nowhere,Wishlist\r\n\
        JIRA-6,'=Imported formula,,To Do,,2027-03-01 09:30,,\r\n\
        JIRA-1,Backfill the topic again,,Open,,,,\r\n";
    let mapping = json!({"external_id": "Issue ID", "title": "Summary", "description": "Details", "status": "State",
                         "assignee": "Owner email", "due_at": "Due", "project": "Area", "labels": "Tags"});
    let import = format!("{tasks}/import");
    let preview = call(
        "POST",
        import.clone(),
        fixture.member.clone(),
        json!({"csv": source, "mapping": mapping}),
    )
    .await;
    assert_eq!(preview.status(), 200);
    let preview = response_json(preview).await;
    assert_eq!(preview["dry_run"], true, "a preview is the default");
    assert_eq!(preview["committed"], false);
    assert_eq!(
        (
            preview["rows"].as_i64(),
            preview["valid"].as_i64(),
            preview["invalid"].as_i64(),
            preview["duplicates"].as_i64()
        ),
        (Some(7), Some(2), Some(4), Some(1))
    );
    assert_eq!(preview["unknown_assignees"], json!(["nobody@example.com"]));
    assert_eq!(preview["mapping"]["title"], "Summary");
    let item = |row: i64| {
        preview["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["row"] == row)
            .unwrap()
            .clone()
    };
    assert_eq!(item(2)["outcome"], "valid");
    assert!(item(3)["problems"][0].as_str().unwrap().contains("title is empty"));
    assert!(item(4)["problems"].to_string().contains("Unknown assignee"));
    assert!(item(4)["problems"].to_string().contains("nobody@example.com"));
    assert!(
        !item(4)["problems"].to_string().contains("\"Waiting\""),
        "waiting is a blocked status"
    );
    assert!(item(5)["problems"].to_string().contains("not a date"));
    assert!(item(6)["problems"].to_string().contains("Unknown project"));
    assert!(item(6)["problems"].to_string().contains("Unknown label"));
    assert_eq!(
        item(7)["outcome"],
        "valid",
        "an exported apostrophe is removed on the way back in"
    );
    assert_eq!(item(8)["outcome"], "duplicate");
    assert!(item(8)["problems"][0]
        .as_str()
        .unwrap()
        .contains("earlier in this file"));
    assert_eq!(count_tasks().await, before, "a preview writes nothing");

    // With problems, a commit is refused and nothing is written; a reader cannot import at all.
    let refused = call(
        "POST",
        import.clone(),
        fixture.member.clone(),
        json!({"csv": source, "mapping": mapping, "dry_run": false}),
    )
    .await;
    assert_eq!(refused.status(), 409);
    let refused = response_json(refused).await;
    assert_eq!(
        (refused["committed"].as_bool(), refused["created"].as_i64()),
        (Some(false), Some(0))
    );
    assert!(refused["note"].as_str().unwrap().contains("Nothing was imported"));
    assert_eq!(count_tasks().await, before);
    assert_eq!(
        call(
            "POST",
            import.clone(),
            fixture.guest.clone(),
            json!({"csv": source, "mapping": mapping, "dry_run": false})
        )
        .await
        .status(),
        403
    );

    // Skipping the invalid rows imports the rest, and the tasks are what the file said.
    let committed = call(
        "POST",
        import.clone(),
        fixture.member.clone(),
        json!({"csv": source, "mapping": mapping, "dry_run": false, "skip_invalid": true}),
    )
    .await;
    assert_eq!(committed.status(), 200);
    let committed = response_json(committed).await;
    assert_eq!(
        (committed["committed"].as_bool(), committed["created"].as_i64()),
        (Some(true), Some(2))
    );
    assert_eq!(count_tasks().await, before + 2);
    let made = response_json(call("GET", tasks.clone(), fixture.owner.clone(), Value::Null).await).await;
    let find = |title: &str| {
        made.as_array()
            .unwrap()
            .iter()
            .find(|task| task["title"] == title)
            .unwrap()
            .clone()
    };
    let formula = find("=Imported formula");
    assert_eq!(
        (formula["status"].as_str(), formula["due_at"].as_str()),
        (Some("open"), Some("2027-03-01T09:30:00+00:00"))
    );
    let backfill = find("Backfill the topic");
    assert_eq!(backfill["status"], "in_progress");
    assert_eq!(backfill["description"], "Two lines\nin one cell");
    assert_eq!(backfill["due_at"], "2027-02-01");
    assert_eq!(backfill["project_name"], "Queue migration");
    assert_eq!(backfill["labels"][0]["name"], "Bug");
    assert_eq!(backfill["assignee"]["email"], "task-csv-member@example.com");
    assert_eq!(backfill["created_by"]["email"], "task-csv-member@example.com");

    // A retry of the same file, and a second attempt after the other rows were fixed, create nothing twice.
    let retry = response_json(
        call(
            "POST",
            import.clone(),
            fixture.member.clone(),
            json!({"csv": source, "mapping": mapping, "dry_run": false, "skip_invalid": true}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (retry["created"].as_i64(), retry["duplicates"].as_i64()),
        (Some(0), Some(3))
    );
    let fixed = source
        .replace("JIRA-2,,No title", "JIRA-2,Now with a title,No title")
        .replace("not a date", "2027-04-01");
    let second = response_json(
        call(
            "POST",
            import,
            fixture.member.clone(),
            json!({"csv": fixed, "mapping": mapping, "dry_run": false, "skip_invalid": true}),
        )
        .await,
    )
    .await;
    assert_eq!(
        second["created"], 2,
        "the two rows that were fixed; the two imported before are skipped"
    );
    assert_eq!(count_tasks().await, before + 4);

    // What cannot be read at all is a bad request with what to do about it.
    for (body, needle) in [
        (json!({"csv": ""}), "empty"),
        (json!({"csv": "Name,Notes\r\nx,y"}), "Map one"),
        (
            json!({"csv": "Title\r\nx", "mapping": {"title": "Subject"}}),
            "does not have",
        ),
        (
            json!({"csv": "Title\r\nx", "mapping": {"colour": "Title"}}),
            "not a field",
        ),
        (json!({"csv": "Title\r\n\"unclosed"}), "not closed"),
    ] {
        let response = call("POST", format!("{tasks}/import"), fixture.member.clone(), body).await;
        assert_eq!(response.status(), 400);
        assert!(
            response_json(response).await["error"]["message"]
                .as_str()
                .unwrap()
                .contains(needle),
            "{needle}"
        );
    }
    let many = format!("Title\r\n{}", "row\r\n".repeat(1_001));
    assert_eq!(
        call(
            "POST",
            format!("{tasks}/import"),
            fixture.member.clone(),
            json!({"csv": many})
        )
        .await
        .status(),
        400
    );
}
