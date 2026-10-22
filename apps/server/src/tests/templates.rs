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
        // Health (V0.2.181): a team runs the checks it needs; indexing failures are always reported.
        let checks = view["health"]["detectors"].as_array().unwrap();
        assert!(checks.len() >= 3 && checks.contains(&json!("index_failed")), "{id}");
        assert_eq!(
            checks.contains(&json!("removed_symbol_mentioned")),
            id == "software-engineering",
            "{id}"
        );
        // The assistant's phrases and focus (V0.2.185): every template gives some.
        assert!(!view["assistant"]["phrases"].as_array().unwrap().is_empty(), "{id}");
        assert!(
            view["assistant"]["focus"]
                .as_str()
                .is_some_and(|focus| focus.starts_with("This workspace holds")),
            "{id}"
        );
        // The headline of the dashboard's card (V0.2.187): the figure each template counts.
        assert_eq!(
            view["overview"]["headline"],
            match id {
                "software-engineering" => json!("code_symbols"),
                "operations" => json!("active_tasks"),
                "product-decisions" => json!("memory_cards"),
                _ => json!("sources"),
            },
            "{id}"
        );
        // The link form's rules (V0.2.186): code teams leave files out, documents-first teams ask for documents.
        let offered = |key: &str| view["repositories"][key].as_array().map_or(0, Vec::len);
        assert!(
            match id {
                "software-engineering" | "operations" => offered("exclude") > 0 && offered("include") == 0,
                _ => offered("include") > 0 && offered("exclude") == 0,
            },
            "{id}: {}",
            view["repositories"]
        );
        // The map (V0.2.184): Software engineering puts repositories and code first.
        let coverage_first = view["map"]["coverage_first"].get(0).cloned().unwrap_or(Value::Null);
        assert_eq!(
            coverage_first,
            match id {
                "software-engineering" => json!("repository"),
                "operations" | "product-decisions" => json!("markdown"),
                _ => json!("word"),
            },
            "{id}"
        );
        // The review queue (V0.2.183): Operations lists health findings first; Software engineering keeps the mixed list.
        let review_first = view["review"]["order"].get(0).cloned().unwrap_or(Value::Null);
        assert_eq!(
            review_first,
            match id {
                "operations" => json!("health_finding"),
                "product-decisions" => json!("memory_card"),
                "knowledge-base" => json!("evidence"),
                _ => Value::Null,
            },
            "{id}"
        );
        // Evidence nobody has touched (V0.2.182): Operations after 90 days, the Knowledge base after 180, nobody else.
        let days = match id {
            "operations" => json!(90),
            "knowledge-base" => json!(180),
            _ => Value::Null,
        };
        assert_eq!(view["health"]["stale_after_days"], days, "{id}");
        assert_eq!(checks.contains(&json!("stale_evidence")), !days.is_null(), "{id}");
        // Ranking (V0.2.178): Operations and Knowledge base favour verified evidence more.
        let factor = if id == "operations" || id == "knowledge-base" {
            1.15
        } else {
            1.1
        };
        assert_eq!(view["ranking"]["verified"], json!(factor), "{id}");
        assert!(!view["ranking"]["boosts"].as_array().unwrap().is_empty(), "{id}");
        // Only Operations prefers recent evidence (V0.2.180).
        assert_eq!(
            view["ranking"]["recency"],
            if id == "operations" {
                json!({"half_life_days": 90, "max_factor": 1.2})
            } else {
                Value::Null
            },
            "{id}"
        );
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
    assert_eq!(
        by_default["assistant"],
        json!({"order": [], "suggestions": [], "phrases": [], "focus": null})
    );
    assert_eq!(by_default["retrieval"], json!({"type_chips": []}));
    // General names no headline for the dashboard.
    assert!(by_default["overview"]["headline"].is_null());
    // General offers the link form nothing.
    assert_eq!(by_default["repositories"], json!({"include": [], "exclude": []}));
    assert_eq!(
        by_default["ranking"],
        json!({"verified": 1.1, "boosts": [], "recency": null})
    );
    assert_eq!(
        by_default["health"]["detectors"],
        json!([
            "older_version_active",
            "duplicate_content",
            "removed_symbol_mentioned",
            "outdated_evidence_referenced",
            "stale_memory_citation",
            "index_failed",
            "unconnected"
        ])
    );
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

/// V0.2.189: the operator's folder adds its valid files, names the others in System › Overview and never stops the
/// server; a file named like a built-in template is refused.
#[tokio::test]
async fn a_folder_of_templates_adds_the_valid_files_and_names_the_rest() {
    let data_dir = temp_data_dir("templates-folder");
    let folder = temp_data_dir("templates-folder-files");
    std::fs::create_dir_all(&folder).unwrap();
    let file = |id: &str, name: &str, extra: &str| {
        format!(
            r#"{{ "format": 1, "meta": {{ "id": "{id}", "version": 1, "name": "{name}", "summary": "From the operator's folder.", "icon": "messages" }}{extra} }}"#
        )
    };
    std::fs::write(
        folder.join("support.json"),
        file(
            "support",
            "Customer support",
            r#", "sections": { "order": ["review"] }"#,
        ),
    )
    .unwrap();
    std::fs::write(
        folder.join("operations.json"),
        file("operations", "Operations, my way", ""),
    )
    .unwrap();
    std::fs::write(
        folder.join("typo.json"),
        file("typo", "Typo", r#", "sections": { "order": ["evidnce"] }"#),
    )
    .unwrap();
    std::fs::write(folder.join("readme.txt"), "not a template").unwrap();

    let mut config = ServerConfig::for_test(data_dir.clone());
    config.templates_dir = Some(folder.clone());
    let app = router(config).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "templates-folder-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-folder-owner@example.com").await;

    // The valid file is offered, tagged custom; the built-in templates are not, and operations is still the built-in one.
    let listed = response_json(send("GET", "/v1/templates".to_owned(), owner.clone(), json!(null)).await).await;
    let listed = listed.as_array().unwrap();
    let find = |id: &str| {
        listed
            .iter()
            .filter(|template| template["id"] == id)
            .collect::<Vec<_>>()
    };
    assert_eq!(find("support").len(), 1);
    assert_eq!(find("support")[0]["custom"], true);
    assert_eq!(find("operations").len(), 1);
    assert_eq!(find("operations")[0]["custom"], false);
    assert_eq!(find("operations")[0]["name"], "Operations and incidents");
    assert!(find("typo").is_empty());
    assert_eq!(find("general")[0]["custom"], false);

    // System › Overview names the folder, what it added and each file it left out with the place and the reason.
    let overview = response_json(send("GET", "/v1/system/overview".to_owned(), admin.clone(), json!(null)).await).await;
    let files = &overview["template_files"];
    assert_eq!(files["folder"], folder.display().to_string());
    assert_eq!(
        files["loaded"],
        json!([{"id": "support", "name": "Customer support", "file": "support.json"}])
    );
    // The list names every template and where it came from (V0.2.190).
    let sources: Vec<(String, String)> = files["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|template| {
            (
                template["id"].as_str().unwrap().to_owned(),
                template["source"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(sources[0], ("general".to_owned(), "code".to_owned()));
    assert!(sources.contains(&("operations".to_owned(), "built-in".to_owned())));
    assert!(sources.contains(&("support".to_owned(), "custom".to_owned())));
    assert_eq!(
        files["templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|template| template["id"] == "support")
            .unwrap()["file"],
        "support.json"
    );
    let problems = files["problems"].as_array().unwrap();
    let problem = |file: &str| {
        problems
            .iter()
            .find(|problem| problem["file"] == file)
            .unwrap_or(&Value::Null)
    };
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problem("operations.json")["issues"][0]["message"]
        .as_str()
        .unwrap()
        .contains("built into this server"));
    assert_eq!(problem("typo.json")["issues"][0]["path"], "sections.order[0]");

    // The console lists the same templates and the same problems, shows one, and sends validate to the terminal client.
    let console = |command: &'static str| {
        let request = json_request("POST", "/v1/system/console", &admin, json!({ "command": command }));
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap() }
    };
    let listed_in_console = response_json(console("templates list").await).await;
    assert_eq!(listed_in_console["command"], "templates list");
    let rows = listed_in_console["data"].as_array().unwrap();
    let row = |id: &str| rows.iter().find(|row| row["id"] == id).unwrap_or(&Value::Null);
    assert_eq!(row("support")["source"], "custom (support.json)");
    assert_eq!(row("operations")["source"], "built into the server");
    assert_eq!(row("general")["source"], "built into the server (code)");
    assert_eq!(
        rows.iter().filter(|row| row["id"] == "-").count(),
        2,
        "one row for each problem"
    );
    assert!(rows
        .iter()
        .any(|row| row["name"] == "typo.json" && row["state"].as_str().unwrap().contains("sections.order[0]")));
    assert!(listed_in_console["summary"]
        .as_str()
        .unwrap()
        .contains("2 file(s) left out"));
    // A group name alone runs its first command.
    assert_eq!(
        response_json(console("templates").await).await["command"],
        "templates list"
    );
    let shown = response_json(console("templates show support").await).await;
    assert_eq!(shown["view"]["kind"], "facts");
    let facts = shown["data"].as_array().unwrap();
    let fact = |label: &str| {
        facts.iter().find(|pair| pair[0] == label).unwrap()[1]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(fact("Source"), "custom (support.json)");
    assert!(fact("Tabs").starts_with("review"));
    assert_eq!(fact("Id · version"), "support · 1");
    let missing = console("templates show no-such-template").await;
    assert_eq!(missing.status(), 404);
    let validate = console("templates validate support.json").await;
    assert_eq!(validate.status(), 400);
    assert!(response_json(validate).await["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("repomemo-console"));

    // A workspace made from the custom template uses it as any other, and says it is custom.
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
    let created = send(
        "POST",
        "/v1/workspaces".to_owned(),
        owner.clone(),
        json!({"organization_id": organization, "name": "Help desk", "template_id": "support"}),
    )
    .await;
    assert_eq!(created.status(), 201);
    let workspace = response_json(created).await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
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
    assert_eq!(view["template"]["id"], "support");
    assert_eq!(view["template"]["custom"], true);
    assert_eq!(view["sections"]["primary"][0], "review");

    // A server without a folder says nothing about one.
    let plain_dir = temp_data_dir("templates-no-folder");
    let plain = router(ServerConfig::for_test(plain_dir.clone())).await.unwrap();
    let (plain_admin, _) = register_account(&plain, "templates-plain-admin@example.com").await;
    let overview = response_json(
        plain
            .clone()
            .oneshot(json_request("GET", "/v1/system/overview", &plain_admin, json!(null)))
            .await
            .unwrap(),
    )
    .await;
    assert!(overview["template_files"]["folder"].is_null());
    assert_eq!(overview["template_files"]["problems"], json!([]));
    // It still lists the templates it has, none of them custom.
    let plain_templates = overview["template_files"]["templates"].as_array().unwrap();
    assert_eq!(plain_templates[0]["id"], "general");
    assert!(plain_templates.len() >= 5 && plain_templates.iter().all(|template| template["source"] != "custom"));

    let _ = std::fs::remove_dir_all(data_dir);
    let _ = std::fs::remove_dir_all(folder);
    let _ = std::fs::remove_dir_all(plain_dir);
}
