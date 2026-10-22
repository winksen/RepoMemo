use super::*;

/// V0.2.170: the four first templates are offered, and a workspace made from each has exactly its folders and
/// opens as any workspace does.
#[tokio::test]
async fn the_four_first_templates_are_offered_and_each_creates_its_folders() {
    let data_dir = temp_data_dir("templates-first-four");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "templates-four-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-four-owner@example.com").await;
    let organization = response_json(
        send(
            "POST",
            "/v1/organizations".to_owned(),
            owner.clone(),
            json!({"name": "Team"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let listed = response_json(send("GET", "/v1/templates".to_owned(), owner.clone(), json!(null)).await).await;
    let ids: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|template| template["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "general",
            "knowledge-base",
            "operations",
            "product-decisions",
            "software-engineering"
        ]
    );
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .all(|template| template["name"].as_str().is_some_and(|name| !name.is_empty())));

    let expected: [(&str, &[&str]); 4] = [
        ("software-engineering", &["Architecture", "Decisions", "Runbooks"]),
        ("operations", &["Incidents", "On-call", "Postmortems", "Runbooks"]),
        (
            "product-decisions",
            &["Decisions", "Meeting notes", "Research", "Specs"],
        ),
        ("knowledge-base", &["Archive", "Contracts", "Policies", "Reports"]),
    ];
    for (id, folders) in expected {
        let created = send(
            "POST",
            "/v1/workspaces".to_owned(),
            owner.clone(),
            json!({"organization_id": organization, "name": format!("From {id}"), "template_id": id}),
        )
        .await;
        assert_eq!(created.status(), 201, "{id}");
        let workspace = response_json(created).await["workspace"]["id"]
            .as_str()
            .unwrap()
            .to_owned();

        let made = response_json(
            send(
                "GET",
                format!("/v1/workspaces/{workspace}/folders"),
                owner.clone(),
                json!(null),
            )
            .await,
        )
        .await;
        let mut names: Vec<&str> = made
            .as_array()
            .unwrap()
            .iter()
            .map(|folder| folder["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, folders, "{id}");
        assert!(
            made.as_array()
                .unwrap()
                .iter()
                .all(|folder| folder["parent_id"].is_null()),
            "{id}"
        );

        let view = response_json(
            send(
                "GET",
                format!("/v1/workspaces/{workspace}/template"),
                owner.clone(),
                json!(null),
            )
            .await,
        )
        .await;
        assert_eq!(
            (view["source"].as_str(), view["template"]["id"].as_str()),
            (Some("workspace"), Some(id))
        );
        // Every section is in the tabs or under More, once; the administrators' sections follow the others.
        let list = |key: &str| -> Vec<String> {
            view["sections"][key]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect()
        };
        let (primary, more) = (list("primary"), list("more"));
        assert_eq!(primary.len() + more.len(), 12, "{id}");
        assert_eq!(&primary[primary.len() - 2..], ["people", "settings"], "{id}");
        assert_eq!(primary[0], "overview", "{id}");
        let landing = match id {
            "operations" => "review",
            "product-decisions" => "memory",
            "knowledge-base" => "documents",
            _ => "overview",
        };
        assert_eq!(view["sections"]["landing"], landing, "{id}");
        // The Overview's cards (V0.2.174): Software engineering leads with the code symbols.
        // The ledger (V0.2.175): Knowledge base and Operations open as a list.
        let list = id == "knowledge-base" || id == "operations";
        assert_eq!(
            view["evidence"]["default_view"],
            if list { "list" } else { "grid" },
            "{id}"
        );
        assert!(!view["evidence"]["types_first"].as_array().unwrap().is_empty(), "{id}");
        // The assistant and Retrieval (V0.2.176): every template suggests questions and chips; Operations asks first.
        assert!(!view["assistant"]["suggestions"].as_array().unwrap().is_empty(), "{id}");
        assert!(!view["retrieval"]["type_chips"].as_array().unwrap().is_empty(), "{id}");
        if id == "operations" {
            assert_eq!(view["assistant"]["order"][0], "ask_question");
        }
        // Ranking (V0.2.178): Operations and Knowledge base favour verified evidence more.
        let factor = if id == "operations" || id == "knowledge-base" {
            1.15
        } else {
            1.1
        };
        assert_eq!(view["ranking"]["verified"], json!(factor), "{id}");
        let cards = view["overview"]["cards"].as_array().unwrap();
        assert!(!cards.is_empty() && cards.len() <= 8, "{id}");
        assert_eq!(
            cards[0],
            if id == "software-engineering" {
                "code_symbols"
            } else {
                cards[0].as_str().unwrap()
            },
            "{id}"
        );
        // The names it gives (V0.2.173): Operations calls Tasks "Actions"; Software engineering names two kinds of evidence.
        assert_eq!(
            view["labels"]["sections"]["tasks"],
            if id == "operations" {
                json!("Actions")
            } else {
                Value::Null
            },
            "{id}"
        );
        assert_eq!(
            view["labels"]["artifact_types"]["pr"],
            if id == "software-engineering" {
                json!("Pull request")
            } else {
                Value::Null
            },
            "{id}"
        );
        assert!(!more.is_empty(), "{id}");
        for page in ["overview", "metrics", "capabilities"] {
            let opened = send(
                "GET",
                format!("/v1/workspaces/{workspace}/{page}"),
                owner.clone(),
                json!(null),
            )
            .await;
            assert_eq!(opened.status(), 200, "{id} {page}");
        }
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.177: a template can be looked at before it is chosen, and an administrator can adjust the landing section, what
/// is under More and the suggested questions, then reset them; a bad adjustment is refused and the feed records each change.
#[tokio::test]
async fn a_template_can_be_previewed_and_its_adjustments_saved_and_reset() {
    let data_dir = temp_data_dir("templates-adjust");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "templates-adjust-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-adjust-owner@example.com").await;
    let (viewer, _) = register_account(&app, "templates-adjust-viewer@example.com").await;
    let workspace = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace,
        "templates-adjust-viewer@example.com",
        "viewer",
    )
    .await;
    let uri = format!("/v1/workspaces/{workspace}/template");

    // The preview: what a template arranges and names, for anyone signed in; an unknown id is 404.
    let detail = response_json(
        send(
            "GET",
            "/v1/templates/operations".to_owned(),
            viewer.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    assert_eq!(detail["template"]["id"], "operations");
    assert_eq!(detail["sections"]["landing"], "review");
    assert_eq!(detail["labels"]["sections"]["tasks"], "Actions");
    assert!(detail["overview"]["cards"].as_array().unwrap().len() >= 1 && detail.get("source").is_none());
    assert_eq!(
        send(
            "GET",
            "/v1/templates/no-such-template".to_owned(),
            viewer.clone(),
            json!(null)
        )
        .await
        .status(),
        404
    );
    let unsigned = Request::builder()
        .method("GET")
        .uri("/v1/templates/operations")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(unsigned).await.unwrap().status(), 401);

    // Adjust: Operations opens on Memory, keeps only Tasks under More and asks its own question.
    let adjustments = json!({"landing": "memory", "more": ["tasks"], "suggestions": ["Who owns the pager?"]});
    let saved = send(
        "PUT",
        uri.clone(),
        owner.clone(),
        json!({"template_id": "operations", "overrides": adjustments}),
    )
    .await;
    assert_eq!(saved.status(), 200);
    let saved = response_json(saved).await;
    assert_eq!(saved["sections"]["landing"], "memory");
    assert_eq!(saved["sections"]["more"], json!(["tasks"]));
    assert_eq!(saved["assistant"]["suggestions"], json!(["Who owns the pager?"]));
    assert_eq!(saved["overrides"], adjustments);
    assert_eq!(saved["source"], "workspace");
    let read = response_json(send("GET", uri.clone(), viewer.clone(), json!(null)).await).await;
    assert_eq!(
        read["sections"]["landing"], "memory",
        "a member sees the adjusted template"
    );

    // A bad adjustment is 400 and changes nothing; a viewer cannot adjust.
    let bad = send(
        "PUT",
        uri.clone(),
        owner.clone(),
        json!({"template_id": "operations", "overrides": {"landing": "settings"}}),
    )
    .await;
    assert_eq!(bad.status(), 400);
    assert!(response_json(bad).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("overrides.landing"));
    assert_eq!(
        response_json(send("GET", uri.clone(), owner.clone(), json!(null)).await).await["sections"]["landing"],
        "memory"
    );
    assert_eq!(
        send(
            "PUT",
            uri.clone(),
            viewer.clone(),
            json!({"template_id": "operations", "overrides": {}})
        )
        .await
        .status(),
        403
    );

    // Reset: an empty object clears the adjustments and the template's own values return.
    let reset = response_json(
        send(
            "PUT",
            uri.clone(),
            owner.clone(),
            json!({"template_id": "operations", "overrides": {}}),
        )
        .await,
    )
    .await;
    assert_eq!(reset["sections"]["landing"], "review");
    assert!(reset["overrides"].is_null());

    // The feed has the choice (saved with its adjustments) and the reset, each once.
    let feed = response_json(
        send(
            "GET",
            format!("/v1/workspaces/{workspace}/activity"),
            owner.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    let summaries: Vec<&str> = feed
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "workspace_template_changed")
        .map(|event| event["summary"].as_str().unwrap())
        .collect();
    assert_eq!(summaries.len(), 2, "{summaries:?}");
    assert!(summaries
        .iter()
        .any(|summary| summary.contains("Changed the workspace template to Operations and incidents")));
    assert!(summaries.contains(&"Reset the workspace template's adjustments."));

    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.168: a workspace can be created with a template; `general` saves the choice and makes no folders, no
/// template saves nothing as before, and a template the server lacks is refused before anything is made.
#[tokio::test]
async fn a_workspace_is_created_from_a_template_or_not_at_all() {
    let data_dir = temp_data_dir("templates-create");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "templates-create-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-create-owner@example.com").await;
    let organization = response_json(
        send(
            "POST",
            "/v1/organizations".to_owned(),
            owner.clone(),
            json!({"name": "Team"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let workspaces =
        || async { response_json(send("GET", "/v1/workspaces".to_owned(), owner.clone(), json!(null)).await).await };

    // No template: nothing is saved and no folder is made, as before.
    let plain = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            owner.clone(),
            json!({"organization_id": organization, "name": "Plain"}),
        )
        .await,
    )
    .await;
    assert_eq!(plain["workspace"]["settings"], json!({}));
    let plain_id = plain["workspace"]["id"].as_str().unwrap().to_owned();
    let folders = response_json(
        send(
            "GET",
            format!("/v1/workspaces/{plain_id}/folders"),
            owner.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    assert_eq!(folders, json!([]));
    let view = response_json(
        send(
            "GET",
            format!("/v1/workspaces/{plain_id}/template"),
            owner.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    assert_eq!(view["source"], "default");

    // General, named on purpose: the choice is saved and General lists no folders.
    let general = send(
        "POST",
        "/v1/workspaces".to_owned(),
        owner.clone(),
        json!({"organization_id": organization, "name": "General one", "template_id": "general"}),
    )
    .await;
    assert_eq!(general.status(), 201);
    let general = response_json(general).await;
    assert_eq!(general["workspace"]["settings"]["template"]["id"], "general");
    let general_id = general["workspace"]["id"].as_str().unwrap().to_owned();
    let folders = response_json(
        send(
            "GET",
            format!("/v1/workspaces/{general_id}/folders"),
            owner.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    assert_eq!(folders, json!([]));
    let view = response_json(
        send(
            "GET",
            format!("/v1/workspaces/{general_id}/template"),
            owner.clone(),
            json!(null),
        )
        .await,
    )
    .await;
    assert_eq!(
        (view["source"].as_str(), view["chosen_id"].as_str()),
        (Some("workspace"), Some("general"))
    );

    // A template the server lacks is refused and nothing is made.
    let before = workspaces().await.as_array().unwrap().len();
    let refused = send(
        "POST",
        "/v1/workspaces".to_owned(),
        owner.clone(),
        json!({"organization_id": organization, "name": "Ghost", "template_id": "no-such-template"}),
    )
    .await;
    assert_eq!(refused.status(), 400);
    assert!(response_json(refused).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("template"));
    assert_eq!(workspaces().await.as_array().unwrap().len(), before);

    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.167: the catalogue is for anyone signed in, a workspace's template is read by its members, and
/// only an owner or administrator with a session or an `admin` token changes it (decision 26); a change
/// is in the activity feed, an unknown template writes nothing and choosing the same one again records nothing.
#[tokio::test]
async fn templates_are_listed_read_by_members_and_changed_only_by_administrators() {
    let data_dir = temp_data_dir("templates");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "templates-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-owner@example.com").await;
    let (viewer, _) = register_account(&app, "templates-viewer@example.com").await;
    let (outsider, _) = register_account(&app, "templates-outsider@example.com").await;
    let workspace = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace,
        "templates-viewer@example.com",
        "viewer",
    )
    .await;
    let template_uri = format!("/v1/workspaces/{workspace}/template");

    // The catalogue: General first, only what identifies each template, and not for the anonymous.
    let listed = response_json(send("GET", "/v1/templates".to_owned(), viewer.clone(), json!(null)).await).await;
    assert_eq!(listed[0]["id"], "general");
    assert_eq!(listed[0]["name"], "General");
    assert!(listed[0]["version"].is_u64() && listed[0]["summary"].is_string() && listed[0]["icon"].is_string());
    assert!(
        listed[0].as_object().unwrap().len() == 5,
        "no block is listed: {}",
        listed[0]
    );
    let unsigned = Request::builder()
        .method("GET")
        .uri("/v1/templates")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(unsigned).await.unwrap().status(), 401);

    // A workspace that was never given one is on General, by default; a member reads it, a stranger does not.
    let by_default = response_json(send("GET", template_uri.clone(), viewer.clone(), json!(null)).await).await;
    assert_eq!(by_default["template"]["id"], "general");
    assert_eq!(by_default["source"], "default");
    assert!(by_default["reason"].is_null() && by_default["chosen_id"].is_null());
    // General arranges the twelve sections as they always were, with nothing under More.
    assert_eq!(
        by_default["sections"]["primary"],
        json!([
            "overview",
            "evidence",
            "documents",
            "repositories",
            "retrieval",
            "map",
            "health",
            "review",
            "memory",
            "tasks",
            "people",
            "settings"
        ])
    );
    assert_eq!(by_default["sections"]["more"], json!([]));
    assert_eq!(by_default["sections"]["landing"], "overview");
    // General renames nothing.
    assert_eq!(by_default["labels"], json!({"sections": {}, "artifact_types": {}}));
    assert_eq!(
        by_default["evidence"],
        json!({"default_view": "grid", "types_first": []})
    );
    assert_eq!(by_default["assistant"], json!({"order": [], "suggestions": []}));
    assert_eq!(by_default["retrieval"], json!({"type_chips": []}));
    assert_eq!(by_default["ranking"], json!({"verified": 1.1}));
    assert_eq!(
        by_default["overview"]["cards"],
        json!([
            "index_coverage",
            "indexed_storage",
            "fresh_evidence",
            "knowledge_density",
            "code_symbols",
            "team_access",
            "active_tasks",
            "at_risk",
            "team_discussion"
        ])
    );
    assert_eq!(
        send("GET", template_uri.clone(), outsider.clone(), json!(null))
            .await
            .status(),
        403
    );

    // A viewer cannot change it, and neither can a read-and-write token of the owner.
    let choose_general = json!({"template_id": "general"});
    assert_eq!(
        send("PUT", template_uri.clone(), viewer.clone(), choose_general.clone())
            .await
            .status(),
        403
    );
    let mint = |scope: &'static str| {
        let send = &send;
        let owner = owner.clone();
        async move {
            let created = response_json(
                send(
                    "POST",
                    "/v1/profile/tokens".to_owned(),
                    owner,
                    json!({"name": scope, "scope": scope, "expires_in_days": 7}),
                )
                .await,
            )
            .await;
            format!("Bearer {}", created["secret"].as_str().unwrap())
        }
    };
    let write_token = mint("write").await;
    let refused = send("PUT", template_uri.clone(), write_token.clone(), choose_general.clone()).await;
    assert_eq!(refused.status(), 403);
    assert!(response_json(refused).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("admin scope"));
    let read_with_write_token = send("GET", template_uri.clone(), write_token, json!(null)).await;
    assert_eq!(
        read_with_write_token.status(),
        200,
        "a token that may read the workspace may read its template"
    );
    assert_eq!(
        send("GET", template_uri.clone(), viewer.clone(), json!(null))
            .await
            .status(),
        200
    );

    // An unknown template is not found and nothing is written.
    let unknown = send(
        "PUT",
        template_uri.clone(),
        owner.clone(),
        json!({"template_id": "no-such-template"}),
    )
    .await;
    assert_eq!(unknown.status(), 404);
    let after_unknown = response_json(send("GET", template_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(after_unknown["source"], "default");
    let changes = |activity: &Value| {
        activity
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["action"] == "workspace_template_changed")
            .count()
    };
    let feed_uri = format!("/v1/workspaces/{workspace}/activity");
    let feed = response_json(send("GET", feed_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 0);

    // The owner chooses General on purpose: it is the workspace's own choice, and it is in the feed once.
    let chosen = send("PUT", template_uri.clone(), owner.clone(), choose_general.clone()).await;
    assert_eq!(chosen.status(), 200);
    let chosen = response_json(chosen).await;
    assert_eq!(chosen["source"], "workspace");
    assert_eq!(chosen["chosen_id"], "general");
    let again = send("PUT", template_uri.clone(), owner.clone(), choose_general).await;
    assert_eq!(again.status(), 200);
    let feed = response_json(send("GET", feed_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 1, "choosing the same template again records nothing");
    let event = feed
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["action"] == "workspace_template_changed")
        .unwrap();
    assert!(event["summary"].as_str().unwrap().contains("General"));

    // A token granted the admin scope may return the workspace to the default; that is a second change.
    let admin_token = mint("admin").await;
    let reset = send("PUT", template_uri.clone(), admin_token, json!({"template_id": null})).await;
    assert_eq!(reset.status(), 200);
    assert_eq!(response_json(reset).await["source"], "default");
    let feed = response_json(send("GET", feed_uri, owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 2);

    let _ = std::fs::remove_dir_all(data_dir);
}
