//! Labels (V0.7.10): a workspace names its kinds of work with accessible colours, applies them to tasks and filters by them;
//! renaming and merging keep every assignment; a label never decides who can see a task.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn labels_are_renamed_and_merged_without_losing_assignments_and_never_gate_access() {
    let fixture = knowledge_fixture("task-labels").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let labels = format!("/v1/workspaces/{workspace}/task-labels");
    let tasks = format!("/v1/workspaces/{workspace}/tasks");

    // A member makes labels; the colour comes from a fixed set and the name is unique whatever its case.
    let bug = call(
        "POST",
        labels.clone(),
        fixture.member.clone(),
        json!({"name": "  Bug ", "color": "red"}),
    )
    .await;
    assert_eq!(bug.status(), 201);
    let bug = response_json(bug).await;
    assert_eq!(bug["name"], "Bug");
    assert_eq!(bug["background_color"], "#FECACA");
    assert_eq!(bug["text_color"], "#7F1D1D");
    let bug_id = bug["id"].as_str().unwrap().to_owned();
    let docs = response_json(
        call(
            "POST",
            labels.clone(),
            fixture.member.clone(),
            json!({"name": "Docs", "color": "blue"}),
        )
        .await,
    )
    .await;
    let docs_id = docs["id"].as_str().unwrap().to_owned();
    for (body, status) in [
        (json!({"name": "bug"}), 409),
        (json!({"name": "Plain", "color": "neon"}), 400),
        (json!({"name": "   "}), 400),
        (json!({"name": "x".repeat(41)}), 400),
    ] {
        assert_eq!(
            call("POST", labels.clone(), fixture.member.clone(), body)
                .await
                .status(),
            status
        );
    }
    assert_eq!(
        call(
            "POST",
            labels.clone(),
            fixture.guest.clone(),
            json!({"name": "Reader's"})
        )
        .await
        .status(),
        403
    );

    // Two tasks carry labels; the list shows how many tasks each label has.
    let make = |title: &'static str| {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "status": "open", "priority": "medium"}),
        )
    };
    let first = response_json(make("Crash on start").await).await;
    let second = response_json(make("Explain the queue").await).await;
    let labels_of = |task: &Value| format!("/v1/tasks/{}/labels", task["id"].as_str().unwrap());
    let set = |task: &Value, ids: Value| {
        call(
            "PUT",
            labels_of(task),
            fixture.member.clone(),
            json!({"label_ids": ids}),
        )
    };
    let applied = response_json(set(&first, json!([bug_id])).await).await;
    assert_eq!(applied[0]["name"], "Bug");
    set(&second, json!([bug_id, docs_id, bug_id])).await;
    let listed = response_json(call("GET", labels.clone(), fixture.guest.clone(), Value::Null).await).await;
    let count = |name: &str| {
        listed
            .as_array()
            .unwrap()
            .iter()
            .find(|label| label["name"] == name)
            .unwrap()["task_count"]
            .as_i64()
    };
    assert_eq!((count("Bug"), count("Docs")), (Some(2), Some(1)));
    let seen = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", second["id"].as_str().unwrap()),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        seen["labels"].as_array().unwrap().len(),
        2,
        "a reader sees the labels, once each"
    );

    // The list filters by a label and never hides a task from someone who may read it.
    let by_docs = response_json(
        call(
            "GET",
            format!("{tasks}?label={docs_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(by_docs.as_array().unwrap().len(), 1);
    assert_eq!(by_docs[0]["title"], "Explain the queue");
    let all = response_json(call("GET", tasks.clone(), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    // A label of another workspace, or too many labels, is refused.
    let other_workspace = create_workspace(&app, &fixture.owner).await;
    let foreign = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/task-labels"),
            fixture.owner.clone(),
            json!({"name": "Elsewhere"}),
        )
        .await,
    )
    .await;
    assert_eq!(set(&first, json!([foreign["id"]])).await.status(), 400);
    let mut many = Vec::new();
    for index in 0..11 {
        let label = response_json(
            call(
                "POST",
                labels.clone(),
                fixture.member.clone(),
                json!({"name": format!("Kind {index}")}),
            )
            .await,
        )
        .await;
        many.push(label["id"].clone());
    }
    assert_eq!(set(&first, json!(many)).await.status(), 400);
    assert_eq!(set(&first, json!([bug_id])).await.status(), 200);

    // Renaming keeps every assignment.
    let renamed = call(
        "PUT",
        format!("/v1/task-labels/{bug_id}"),
        fixture.member.clone(),
        json!({"name": "Defect", "color": "orange"}),
    )
    .await;
    assert_eq!(renamed.status(), 200);
    let renamed = response_json(renamed).await;
    assert_eq!(
        (renamed["name"].as_str(), renamed["task_count"].as_i64()),
        (Some("Defect"), Some(2))
    );
    let after = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", first["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(after["labels"][0]["name"], "Defect");
    assert_eq!(after["labels"][0]["background_color"], "#FED7AA");
    assert_eq!(
        call(
            "PUT",
            format!("/v1/task-labels/{docs_id}"),
            fixture.member.clone(),
            json!({"name": "defect"})
        )
        .await
        .status(),
        409
    );

    // Merging moves the tasks of one label to another; only an administrator merges or deletes.
    assert_eq!(
        call(
            "POST",
            format!("/v1/task-labels/{docs_id}/merge"),
            fixture.member.clone(),
            json!({"into_label_id": bug_id})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        call(
            "POST",
            format!("/v1/task-labels/{docs_id}/merge"),
            fixture.owner.clone(),
            json!({"into_label_id": docs_id})
        )
        .await
        .status(),
        400
    );
    let merged = call(
        "POST",
        format!("/v1/task-labels/{docs_id}/merge"),
        fixture.owner.clone(),
        json!({"into_label_id": bug_id}),
    )
    .await;
    assert_eq!(merged.status(), 200);
    let merged = response_json(merged).await;
    assert_eq!(
        (merged["name"].as_str(), merged["task_count"].as_i64()),
        (Some("Defect"), Some(2))
    );
    let both = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", second["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        both["labels"].as_array().unwrap().len(),
        1,
        "a task that had both keeps one"
    );
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/task-labels/{bug_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/task-labels/{bug_id}"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    // The tasks are still there, unlabelled, and still readable by everyone who could read them.
    let gone = response_json(call("GET", tasks, fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(gone.as_array().unwrap().len(), 2);
    assert!(gone[0]["labels"].as_array().unwrap().is_empty());
}
