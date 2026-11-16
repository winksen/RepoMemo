//! Workflow states (V0.7.11): a workspace names task states and maps each to one of the four stable categories; the four
//! built-in states stay as they are; a task's status is always its category; removing a state that holds tasks needs a place
//! for them.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn named_states_map_to_stable_categories_and_removal_needs_a_place_for_the_tasks() {
    let fixture = knowledge_fixture("workflow").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let workflow = format!("/v1/workspaces/{workspace}/workflow");
    let states = format!("{workflow}/states");
    let tasks = format!("/v1/workspaces/{workspace}/tasks");

    // A task made the old way is in the built-in state of its status.
    let old = response_json(
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": "Plain", "status": "in_progress", "priority": "medium"}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (old["state_id"].as_str(), old["state_name"].as_str()),
        (Some("in_progress"), Some("In progress"))
    );

    // The four built-in states always exist, with their counts; a reader can see them.
    let listed = response_json(call("GET", workflow.clone(), fixture.guest.clone(), Value::Null).await).await;
    let listed = listed.as_array().unwrap();
    assert_eq!(listed.len(), 4);
    assert!(listed
        .iter()
        .all(|state| state["builtin"] == true && state["id"] == state["category"]));
    assert_eq!(
        listed.iter().find(|state| state["id"] == "in_progress").unwrap()["task_count"],
        1
    );

    // Only an administrator names states; names are unique and cannot repeat a built-in one; the category is one of four.
    assert_eq!(
        call(
            "POST",
            states.clone(),
            fixture.member.clone(),
            json!({"name": "Review", "category": "in_progress"})
        )
        .await
        .status(),
        403
    );
    let review = call(
        "POST",
        states.clone(),
        fixture.owner.clone(),
        json!({"name": " Review ", "category": "in_progress"}),
    )
    .await;
    assert_eq!(review.status(), 201);
    let review = response_json(review).await;
    let review_id = review["id"].as_str().unwrap().to_owned();
    assert_eq!(
        (review["name"].as_str(), review["builtin"].as_bool()),
        (Some("Review"), Some(false))
    );
    for (body, status) in [
        (json!({"name": "review", "category": "open"}), 409),
        (json!({"name": "DONE", "category": "done"}), 409),
        (json!({"name": "Odd", "category": "someday"}), 400),
        (json!({"name": "  ", "category": "open"}), 400),
    ] {
        assert_eq!(
            call("POST", states.clone(), fixture.owner.clone(), body).await.status(),
            status
        );
    }
    assert_eq!(
        call(
            "PUT",
            "/v1/workflow-states/in_progress".to_owned(),
            fixture.owner.clone(),
            json!({"name": "Doing", "category": "in_progress"})
        )
        .await
        .status(),
        400,
        "a built-in state cannot be changed"
    );

    // A task joins a named state and its status is the state's category; a status that disagrees is refused.
    let in_review = call(
        "POST",
        tasks.clone(),
        fixture.member.clone(),
        json!({"title": "Needs a look", "priority": "high", "state_id": review_id}),
    )
    .await;
    assert_eq!(in_review.status(), 201);
    let in_review = response_json(in_review).await;
    assert_eq!(
        (in_review["status"].as_str(), in_review["state_name"].as_str()),
        (Some("in_progress"), Some("Review"))
    );
    assert_eq!(
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": "Mismatch", "status": "done", "priority": "low", "state_id": review_id})
        )
        .await
        .status(),
        400
    );
    let uri = format!("/v1/tasks/{}", in_review["id"].as_str().unwrap());

    // Saved with the same status and no state, it stays in "Review"; saved with another status, it falls back to the built-in one.
    let kept = response_json(
        call(
            "PUT",
            uri.clone(),
            fixture.member.clone(),
            json!({"title": "Needs a look", "status": "in_progress", "priority": "high"}),
        )
        .await,
    )
    .await;
    assert_eq!(kept["state_name"], "Review");

    // Changing the state's category moves its tasks with it.
    let moved = response_json(
        call(
            "PUT",
            format!("/v1/workflow-states/{review_id}"),
            fixture.owner.clone(),
            json!({"name": "Waiting on review", "category": "blocked"}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (moved["category"].as_str(), moved["task_count"].as_i64()),
        (Some("blocked"), Some(1))
    );
    let now = response_json(call("GET", uri.clone(), fixture.member.clone(), Value::Null).await).await;
    assert_eq!(
        (now["status"].as_str(), now["state_name"].as_str()),
        (Some("blocked"), Some("Waiting on review"))
    );

    // A state that holds tasks is not removed until it is told where they go; they land in a state of their own category.
    let removal = format!("/v1/workflow-states/{review_id}");
    let refused = call("DELETE", removal.clone(), fixture.owner.clone(), Value::Null).await;
    assert_eq!(refused.status(), 409);
    assert!(response_json(refused).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("1 task(s)"));
    assert_eq!(
        call(
            "DELETE",
            format!("{removal}?move_to={review_id}"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "DELETE",
            format!("{removal}?move_to=nowhere"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "DELETE",
            format!("{removal}?move_to=done"),
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
            format!("{removal}?move_to=done"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    let finished = response_json(call("GET", uri, fixture.member.clone(), Value::Null).await).await;
    assert_eq!(
        (finished["status"].as_str(), finished["state_name"].as_str()),
        (Some("done"), Some("Done"))
    );
    assert!(
        finished["completed_at"].is_string(),
        "entering done sets the completion time"
    );
    assert_eq!(
        finished["version"], 4,
        "creating, saving, the category change and the move are four versions"
    );
    assert_eq!(
        call("DELETE", removal, fixture.owner.clone(), Value::Null)
            .await
            .status(),
        404
    );
    assert_eq!(
        call(
            "DELETE",
            "/v1/workflow-states/done".to_owned(),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        400
    );
    let after = response_json(call("GET", workflow, fixture.owner.clone(), Value::Null).await).await;
    assert_eq!(after.as_array().unwrap().len(), 4);
    assert_eq!(
        after
            .as_array()
            .unwrap()
            .iter()
            .find(|state| state["id"] == "done")
            .unwrap()["task_count"],
        1
    );
}
