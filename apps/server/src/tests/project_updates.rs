//! Project updates (V0.7.17): a person publishes the health they judge a project to have, with a reason and what it rests on; the
//! product offers counts and never the health; a reader can see what changed since the update before; retracting hides an update
//! without erasing it.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_person_publishes_a_health_with_its_reason_and_readers_can_see_what_changed() {
    let fixture = knowledge_fixture("project-updates").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
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
    let project_id = project["id"].as_str().unwrap().to_owned();
    assert!(project["health"].is_null(), "no health until a person gives one");
    let milestone = response_json(
        call(
            "POST",
            format!("/v1/projects/{project_id}/milestones"),
            fixture.member.clone(),
            json!({"name": "Dual write", "due_date": "2999-01-31"}),
        )
        .await,
    )
    .await;
    let milestone_id = milestone["id"].as_str().unwrap().to_owned();
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: &'static str, status: &'static str| {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "status": status, "priority": "medium", "project_id": project_id}),
        )
    };
    let stuck = response_json(make("Backfill is stuck", "blocked").await).await;
    let finishing = response_json(make("Dual write the queue", "open").await).await;
    make("Cut over", "open").await;
    let stuck_id = stuck["id"].as_str().unwrap().to_owned();
    let updates = format!("/v1/projects/{project_id}/updates");

    // What the product offers: counts, milestones and what is blocked, and no health of its own.
    let draft = response_json(
        call(
            "GET",
            format!("/v1/projects/{project_id}/update-draft"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(draft["snapshot"]["task_count"], 3);
    assert_eq!(draft["snapshot"]["blocked_count"], 1);
    assert_eq!(draft["open_blockers"][0]["key"], "T-1");
    assert!(draft.get("health").is_none() && draft["snapshot"].get("health").is_none());
    assert!(draft["previous"].is_null());
    assert!(draft["note"].as_str().unwrap().contains("health is yours to choose"));

    // What is not an update is refused: no health, a made-up one, no reason, no summary; a reader cannot publish.
    let valid = json!({"health": "at_risk", "summary": "The backfill stalled.", "reason": "Two days late and blocked.",
                       "milestone_ids": [milestone_id], "task_ids": [stuck_id],
                       "evidence_ids": [fixture.current_id], "memory_card_ids": [fixture.card_on_stale_id]});
    for (field, value) in [
        ("health", json!("fine")),
        ("health", json!("")),
        ("reason", json!("  ")),
        ("summary", json!("")),
        ("summary", json!("x".repeat(2_001))),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        assert_eq!(
            call("POST", updates.clone(), fixture.member.clone(), body)
                .await
                .status(),
            400,
            "{field}"
        );
    }
    assert_eq!(
        call("POST", updates.clone(), fixture.guest.clone(), valid.clone())
            .await
            .status(),
        403
    );
    let other_project = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/projects"),
            fixture.member.clone(),
            json!({"name": "Other"}),
        )
        .await,
    )
    .await;
    let other_milestone = response_json(
        call(
            "POST",
            format!("/v1/projects/{}/milestones", other_project["id"].as_str().unwrap()),
            fixture.member.clone(),
            json!({"name": "Elsewhere"}),
        )
        .await,
    )
    .await;
    for (field, value) in [
        ("milestone_ids", json!([other_milestone["id"]])),
        ("task_ids", json!(["not-a-task"])),
        ("evidence_ids", json!(["not-a-file"])),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        let status = call("POST", updates.clone(), fixture.member.clone(), body)
            .await
            .status();
        assert!(status == 400 || status == 404, "{field}: {status}");
    }

    // A person publishes: the update carries who, the health they chose, why, the counts as they were and what it points at.
    let first = call("POST", updates.clone(), fixture.member.clone(), valid.clone()).await;
    assert_eq!(first.status(), 201);
    let first = response_json(first).await;
    assert_eq!(first["health"], "at_risk");
    assert_eq!(first["reason"], "Two days late and blocked.");
    assert_eq!(first["author"]["email"], "project-updates-member@example.com");
    assert_eq!(first["snapshot"]["task_count"], 3);
    assert_eq!(first["changes"][0], "This is the project's first update.");
    assert_eq!(first["links"].as_array().unwrap().len(), 4);
    assert_eq!(first["links"][1]["label"], "T-1 Backfill is stuck");
    let shown = response_json(
        call(
            "GET",
            format!("/v1/projects/{project_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        shown["health"], "at_risk",
        "the project shows the health a person gave it"
    );

    // Things move on: a task is done, the milestone date moves, someone renames the blocker and the evidence is deleted.
    call(
        "PUT",
        format!("/v1/tasks/{}", finishing["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"title": "Dual write the queue", "status": "done", "priority": "medium"}),
    )
    .await;
    call(
        "PUT",
        format!("/v1/milestones/{milestone_id}"),
        fixture.member.clone(),
        json!({"name": "Dual write", "due_date": "2999-02-28"}),
    )
    .await;
    call(
        "PUT",
        format!("/v1/tasks/{stuck_id}"),
        fixture.member.clone(),
        json!({"title": "Backfill, unblocked", "status": "in_progress", "priority": "medium"}),
    )
    .await;
    call(
        "POST",
        format!("/v1/workspaces/{workspace}/artifacts/bulk"),
        fixture.owner.clone(),
        json!({"action": "delete", "artifact_ids": [fixture.current_id]}),
    )
    .await;
    let second = response_json(
        call(
            "POST",
            updates.clone(),
            fixture.member.clone(),
            json!({"health": "on_track", "summary": "Unblocked and one more task done.", "reason": "The backfill resumed."}),
        )
        .await,
    )
    .await;
    let changes: Vec<&str> = second["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change.as_str().unwrap())
        .collect();
    assert!(changes.contains(&"Health: at risk to on track."), "{changes:?}");
    assert!(changes.contains(&"Done: 0 to 1."), "{changes:?}");
    assert!(changes.contains(&"Blocked: 1 to 0."), "{changes:?}");
    assert!(
        changes
            .iter()
            .any(|change| change.contains("moved from 2999-01-31 to 2999-02-28")),
        "{changes:?}"
    );

    // The first update is read as it was published, and what it pointed at is checked as it is now.
    let again = response_json(
        call(
            "GET",
            format!("/v1/project-updates/{}", first["id"].as_str().unwrap()),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(again["snapshot"]["blocked_count"], 1, "the counts stay as they were");
    let links = again["links"].as_array().unwrap();
    let task_link = links.iter().find(|link| link["kind"] == "task").unwrap();
    assert_eq!(
        (task_link["label"].as_str(), task_link["title"].as_str()),
        (Some("T-1 Backfill is stuck"), Some("T-1 Backfill, unblocked"))
    );
    let evidence_link = links.iter().find(|link| link["kind"] == "evidence").unwrap();
    assert_eq!(evidence_link["available"], false);
    assert!(evidence_link["title"].is_null());
    assert_eq!(evidence_link["label"], "Deploy checklist");
    let listed = response_json(call("GET", updates.clone(), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0]["id"], second["id"], "newest first");
    let shown = response_json(
        call(
            "GET",
            format!("/v1/projects/{project_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(shown["health"], "on_track");

    // Retracting hides an update without erasing it: its author or an administrator can, a reader cannot.
    let retract = format!("/v1/project-updates/{}/retract", second["id"].as_str().unwrap());
    assert_eq!(
        call("POST", retract.clone(), fixture.guest.clone(), Value::Null)
            .await
            .status(),
        403
    );
    let retracted = call("POST", retract.clone(), fixture.owner.clone(), Value::Null).await;
    assert_eq!(retracted.status(), 200);
    let retracted = response_json(retracted).await;
    assert!(retracted["retracted_at"].is_string());
    assert_eq!(retracted["retracted_by"]["email"], "project-updates-owner@example.com");
    assert_eq!(
        call("POST", retract, fixture.owner.clone(), Value::Null).await.status(),
        409
    );
    assert_eq!(
        response_json(call("GET", updates.clone(), fixture.guest.clone(), Value::Null).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        response_json(
            call(
                "GET",
                format!("{updates}?include=retracted"),
                fixture.guest.clone(),
                Value::Null
            )
            .await
        )
        .await
        .as_array()
        .unwrap()
        .len(),
        2
    );
    let shown = response_json(
        call(
            "GET",
            format!("/v1/projects/{project_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(shown["health"], "at_risk", "the health falls back to the update before");

    // An archived project takes no new update.
    call(
        "POST",
        format!("/v1/projects/{project_id}/archive"),
        fixture.member.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(call("POST", updates, fixture.member.clone(), valid).await.status(), 409);
}
