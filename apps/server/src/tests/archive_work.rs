//! Project portability (V0.7.19): projects, milestones, task numbers and relations, labels, named states, schedules and updates go
//! out in an archive and come back with the same relationships; a schedule comes back paused; a number that was given out and
//! deleted is not reused; and an archive that points outside itself is refused.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn projects_and_task_relations_survive_a_move_and_an_archive_that_points_outside_itself_is_refused() {
    let fixture = knowledge_fixture("archive-work").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let read = |uri: String, who: String| {
        let call = &call;
        async move { response_json(call("GET", uri, who, Value::Null).await).await }
    };
    let post = |uri: String, who: String, body: Value| {
        let call = &call;
        async move { response_json(call("POST", uri, who, body).await).await }
    };
    let admin_session = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": "archive-work-root@example.com", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let admin = format!("Bearer {}", admin_session["access_token"].as_str().unwrap());
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // ---- The source workspace, with the work of the 0.7 line in it.
    let label = post(
        format!("/v1/workspaces/{workspace}/task-labels"),
        fixture.member.clone(),
        json!({"name": "Bug", "color": "red"}),
    )
    .await;
    let review = call(
        "POST",
        format!("/v1/workspaces/{workspace}/workflow/states"),
        fixture.owner.clone(),
        json!({"name": "Review", "category": "in_progress"}),
    )
    .await;
    let review = response_json(review).await;
    let project = post(
        format!("/v1/workspaces/{workspace}/projects"),
        fixture.member.clone(),
        json!({"name": "Queue migration", "goal": "Move the worker."}),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_owned();
    for (kind, target) in [
        ("memory_card", &fixture.card_on_stale_id),
        ("evidence", &fixture.current_id),
    ] {
        call(
            "POST",
            format!("/v1/projects/{project_id}/references"),
            fixture.member.clone(),
            json!({"kind": kind, "target_id": target, "note": "Why."}),
        )
        .await;
    }
    let milestone = post(
        format!("/v1/projects/{project_id}/milestones"),
        fixture.member.clone(),
        json!({"name": "Dual write", "due_date": "2999-01-31"}),
    )
    .await;
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |body: Value| post(tasks.clone(), fixture.member.clone(), body);
    let first = make(
        json!({"title": "Dual write", "priority": "high", "project_id": project_id, "milestone_id": milestone["id"],
                            "state_id": review["id"], "artifact_id": fixture.stale_id}),
    )
    .await;
    let child =
        make(json!({"title": "Backfill", "status": "open", "priority": "medium", "parent_task_id": first["id"]})).await;
    let vendor = make(json!({"title": "Vendor call", "status": "open", "priority": "low"})).await;
    let temp = make(json!({"title": "Temporary", "status": "open", "priority": "low"})).await;
    let temp_number = temp["number"].as_i64().unwrap();
    call(
        "DELETE",
        format!("/v1/tasks/{}", temp["id"].as_str().unwrap()),
        fixture.member.clone(),
        Value::Null,
    )
    .await;
    call(
        "PUT",
        format!("/v1/tasks/{}/labels", first["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"label_ids": [label["id"]]}),
    )
    .await;
    call(
        "POST",
        format!("/v1/tasks/{}/comments", first["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"body": "Looks right to me."}),
    )
    .await;
    call(
        "POST",
        format!("/v1/tasks/{}/dependencies", child["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"blocker_task_id": vendor["id"]}),
    )
    .await;
    post(
        format!("/v1/workspaces/{workspace}/recurring-tasks"),
        fixture.member.clone(),
        json!({"title": "Daily note", "schedule": {"kind": "daily", "interval": 1, "start_date": day}}),
    )
    .await;
    call(
        "POST",
        "/v1/system/maintenance/run".to_owned(),
        admin.clone(),
        json!({}),
    )
    .await;
    let update = call(
        "POST",
        format!("/v1/projects/{project_id}/updates"),
        fixture.member.clone(),
        json!({"health": "at_risk", "summary": "The backfill is late.", "reason": "Waiting on the vendor.",
               "milestone_ids": [milestone["id"]], "task_ids": [vendor["id"]],
               "evidence_ids": [fixture.current_id], "memory_card_ids": [fixture.card_on_stale_id]}),
    )
    .await;
    assert_eq!(update.status(), 201);
    let source_tasks = read(tasks.clone(), fixture.owner.clone()).await;
    let source_key = |title: &str| {
        source_tasks
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["title"] == title)
            .unwrap()["key"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(
        source_tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["title"] == "Daily note"),
        "the schedule made today's task"
    );

    // ---- Out.
    let archive = to_bytes(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/export"),
            fixture.owner.clone(),
            Value::Null,
        )
        .await
        .into_body(),
        usize::MAX,
    )
    .await
    .unwrap()
    .to_vec();
    let entries: std::collections::HashMap<String, Vec<u8>> =
        repomemo_api::read_archive_entries(archive.as_slice(), 1 << 26)
            .unwrap()
            .into_iter()
            .collect();
    let manifest: Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    assert_eq!(manifest["format_version"], 4);
    for (counted, expected) in [
        ("projects", 1),
        ("milestones", 1),
        ("labels", 1),
        ("workflow_states", 1),
        ("recurring", 1),
        ("project_updates", 1),
        ("task_comments", 1),
        ("dependencies", 1),
    ] {
        assert_eq!(manifest["counts"][counted], expected, "{counted}");
    }
    let data: Value = serde_json::from_slice(&entries["workspace.json"]).unwrap();
    assert!(data["work"]["task_counter"].as_i64().unwrap() > temp_number);
    assert!(
        !String::from_utf8_lossy(&entries["workspace.json"]).contains("user_id"),
        "nobody is named by account id"
    );

    // ---- In.
    let organization = read("/v1/organizations".into(), fixture.owner.clone()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let import = |archive: Vec<u8>, query: &str| {
        let app = app.clone();
        let owner = fixture.owner.clone();
        let uri = format!("/v1/workspaces/import?organization_id={organization}{query}");
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("authorization", owner)
                    .header("content-type", "application/gzip")
                    .body(Body::from(archive))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };
    let imported = import(archive.clone(), "&name=Moved").await;
    assert_eq!(imported.status(), 201);
    let report = response_json(imported).await;
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("paused")),
        "{report}"
    );
    let moved = report["workspace"]["workspace"]["id"].as_str().unwrap().to_owned();
    let moved_tasks = read(format!("/v1/workspaces/{moved}/tasks"), fixture.owner.clone()).await;
    let moved_task = |title: &str| {
        moved_tasks
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["title"] == title)
            .unwrap()
            .clone()
    };

    // The same task has the same number and key; its place, label, state and parent came across.
    for title in ["Dual write", "Backfill", "Vendor call", "Daily note"] {
        assert_eq!(
            moved_task(title)["key"],
            source_key(title).as_str(),
            "{title} keeps its key"
        );
    }
    let dual = moved_task("Dual write");
    assert_eq!(dual["project_name"], "Queue migration");
    assert_eq!(dual["milestone_name"], "Dual write");
    assert_eq!(dual["state_name"], "Review");
    assert_eq!(dual["status"], "in_progress");
    assert_eq!(dual["labels"][0]["name"], "Bug");
    assert_eq!(moved_task("Backfill")["parent_key"], dual["key"]);
    let detail = read(
        format!("/v1/workspaces/{moved}/tasks/{}", dual["key"].as_str().unwrap()),
        fixture.owner.clone(),
    )
    .await;
    assert_eq!(detail["comment_count"], 1);
    let comments = read(
        format!("/v1/tasks/{}/comments", dual["id"].as_str().unwrap()),
        fixture.owner.clone(),
    )
    .await;
    assert!(comments[0]["body"].as_str().unwrap().contains("Looks right to me."));
    assert!(
        comments[0]["body"].as_str().unwrap().contains("Written by"),
        "the author is named in the text"
    );
    assert!(detail["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reference| reference["title"] == "Refund policy"));
    let backfill = read(
        format!(
            "/v1/workspaces/{moved}/tasks/{}",
            moved_task("Backfill")["key"].as_str().unwrap()
        ),
        fixture.owner.clone(),
    )
    .await;
    assert_eq!(
        backfill["dependencies"]["blocked_by"][0]["blocker"]["title"],
        "Vendor call"
    );

    // The project came with its brief, its milestone and its update, all pointing inside the new workspace.
    let projects = read(format!("/v1/workspaces/{moved}/projects"), fixture.owner.clone()).await;
    assert_eq!(projects.as_array().unwrap().len(), 1);
    let new_project = projects[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(projects[0]["health"], "at_risk");
    let brief = read(format!("/v1/projects/{new_project}/brief"), fixture.owner.clone()).await;
    assert_eq!(brief["references"].as_array().unwrap().len(), 2);
    assert!(brief["references"]
        .as_array()
        .unwrap()
        .iter()
        .all(|reference| reference["available"] == true));
    let milestones = read(format!("/v1/projects/{new_project}/milestones"), fixture.owner.clone()).await;
    assert_eq!(
        (milestones[0]["name"].as_str(), milestones[0]["task_count"].as_i64()),
        (Some("Dual write"), Some(1))
    );
    let updates = read(format!("/v1/projects/{new_project}/updates"), fixture.owner.clone()).await;
    assert_eq!(updates[0]["health"], "at_risk");
    assert!(updates[0]["author"].is_null());
    assert!(updates[0]["reason"].as_str().unwrap().contains("Published by"));
    assert!(
        updates[0]["links"]
            .as_array()
            .unwrap()
            .iter()
            .all(|link| link["available"] == true),
        "{}",
        updates[0]["links"]
    );

    // A schedule comes back paused, and a number that was given out and deleted is not given out again.
    let schedules = read(format!("/v1/workspaces/{moved}/recurring-tasks"), fixture.owner.clone()).await;
    assert_eq!(schedules[0]["status"], "paused");
    let before = moved_tasks.as_array().unwrap().len();
    call(
        "POST",
        "/v1/system/maintenance/run".to_owned(),
        admin.clone(),
        json!({}),
    )
    .await;
    assert_eq!(
        read(format!("/v1/workspaces/{moved}/tasks"), fixture.owner.clone())
            .await
            .as_array()
            .unwrap()
            .len(),
        before,
        "a paused schedule makes nothing"
    );
    let detail = read(
        format!("/v1/recurring-tasks/{}", schedules[0]["id"].as_str().unwrap()),
        fixture.owner.clone(),
    )
    .await;
    assert_eq!(
        detail["occurrences"].as_array().unwrap().len(),
        1,
        "its history came with it"
    );
    let fresh = post(
        format!("/v1/workspaces/{moved}/tasks"),
        fixture.owner.clone(),
        json!({"title": "After the move", "status": "open", "priority": "low"}),
    )
    .await;
    assert!(
        fresh["number"].as_i64().unwrap() > temp_number,
        "{} must be above {temp_number}",
        fresh["number"]
    );

    // ---- An archive that points outside itself is refused before anything is written.
    let mut hostile: Value = serde_json::from_slice(&entries["workspace.json"]).unwrap();
    hostile["work"]["task_extras"][0]["project_id"] = json!("a-project-of-another-workspace");
    hostile["work"]["task_extras"][1]["blocked_by"] = json!(["a-task-of-another-workspace"]);
    let data_bytes = serde_json::to_vec(&hostile).unwrap();
    let mut forged_manifest = manifest.clone();
    forged_manifest["data_sha256"] = json!(repomemo_storage::StorageEngine::content_hash(&data_bytes));
    forged_manifest["data_bytes"] = json!(data_bytes.len());
    let mut encoder = repomemo_api::ArchiveEncoder::new();
    encoder
        .add("manifest.json", &serde_json::to_vec(&forged_manifest).unwrap(), 0)
        .unwrap();
    encoder.add("workspace.json", &data_bytes, 0).unwrap();
    for (name, bytes) in &entries {
        if name.starts_with("blobs/") {
            encoder.add(name, bytes, 0).unwrap();
        }
    }
    let forged = encoder.finish().unwrap();
    let checked = import(forged.clone(), "&dry_run=true").await;
    assert_eq!(checked.status(), 200);
    let checked = response_json(checked).await;
    assert_eq!(checked["ok"], false, "{checked}");
    let problems = checked["problems"].to_string();
    assert!(problems.contains("project that is not in the archive"), "{problems}");
    assert!(problems.contains("not in the archive"), "{problems}");
    let refused = import(forged, "&name=Never").await;
    assert_ne!(refused.status(), 201);
    let workspaces = read("/v1/workspaces".into(), fixture.owner.clone()).await;
    assert!(
        !workspaces
            .as_array()
            .unwrap()
            .iter()
            .any(|found| found["name"] == "Never"),
        "nothing was written"
    );
}
