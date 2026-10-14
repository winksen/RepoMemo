use crate::*;

pub(crate) async fn list_artifact_index_failures(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<ArtifactIndexFailure>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_index_failures(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn list_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(page): Query<PageQuery>,
) -> Result<(HeaderMap, Json<Vec<SharedArtifactSummary>>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let folder_ids = state
        .storage
        .artifact_folder_ids(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let repository_ids = state
        .storage
        .repo_artifact_sources(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let artifacts = state.core.list_artifacts(workspace_id).await.map_err(map_core_error)?;
    let (headers, artifacts) = page.apply(artifacts);
    Ok((
        headers,
        Json(
            artifacts
                .into_iter()
                .map(|summary| SharedArtifactSummary {
                    folder_id: folder_ids.get(&summary.id).cloned(),
                    repository_id: repository_ids.get(&summary.id).cloned(),
                    summary,
                })
                .collect(),
        ),
    ))
}

pub(crate) async fn list_folders(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<Folder>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_folders(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<Folder>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder = state
        .storage
        .create_folder(
            &workspace_id,
            request.parent_id.as_deref().filter(|id| !id.is_empty()),
            &request.name,
            Some(&subject.user_id),
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_created",
        "folder",
        Some(&folder.id),
        format!("Created folder: {}.", folder.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(folder)))
}

/// The stored original as a download. Always an attachment, sandboxed and
/// never sniffed, so an uploaded HTML or SVG file cannot run in the browser.
/// The stored bytes of an artifact as a download. The content type comes
/// from the file name (`served_mime_type`), never from what an uploader or a
/// repository claimed, unless `content_type` names one (a rendered PDF).
pub(crate) fn file_response(summary: &ArtifactSummary, bytes: Vec<u8>, content_type: Option<&'static str>) -> Response {
    let filename = summary.path.rsplit(['/', '\\']).next().unwrap_or("download").to_owned();
    let ascii: String = filename
        .chars()
        .map(|character| {
            if character.is_ascii_graphic() && !matches!(character, '"' | '\\' | '%') {
                character
            } else if character == ' ' {
                ' '
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = filename
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    let mut headers = HeaderMap::new();
    let content_type = content_type.unwrap_or_else(|| served_mime_type(std::path::Path::new(&summary.path)));
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
    (StatusCode::OK, headers, bytes).into_response()
}

pub(crate) async fn download_artifact_file(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Response, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let bytes = state
        .storage
        .read_artifact_blob(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    Ok(file_response(&summary, bytes, None))
}

pub(crate) async fn artifact_document_preview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<DocumentPreview>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    state
        .core
        .document_preview(&artifact_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

pub(crate) async fn rendered_preview_status(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<RenderStatusResponse>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let (name, message) = match state.converter.state(&state.storage, &summary) {
        RenderState::Ready => ("ready", None),
        RenderState::Converting => ("converting", None),
        RenderState::Disabled => ("disabled", None),
        RenderState::Unsupported => ("unsupported", None),
        RenderState::Failed(message) => ("failed", Some(message)),
    };
    Ok(Json(RenderStatusResponse { state: name, message }))
}

/// The layout-accurate PDF of an Office file, once its conversion is done.
pub(crate) async fn rendered_preview_pdf(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Response, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let path = state
        .converter
        .cached_pdf(&summary.content_hash)
        .ok_or_else(|| ApiError::bad_request("The rendered preview is not ready yet."))?;
    let bytes = tokio::fs::read(path).await.map_err(ApiError::internal)?;
    let mut rendered = summary;
    rendered.mime_type = Some("application/pdf".to_owned());
    rendered.path = format!("{}.pdf", rendered.title);
    Ok(file_response(&rendered, bytes, Some("application/pdf")))
}

/// A short-lived, single-file link that a desktop app (Word, Excel, ...) can
/// fetch without the user's session. It grants read access to one artifact for
/// a few minutes and nothing else.
pub(crate) async fn create_file_link(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<FileLinkResponse>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let claims = FileLinkClaims {
        sub: artifact_id,
        iss: FILE_LINK_ISSUER.to_owned(),
        exp: jsonwebtoken::get_current_timestamp() + FILE_LINK_TTL_SECONDS,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;
    let filename = summary.path.rsplit(['/', '\\']).next().unwrap_or("download").to_owned();
    Ok(Json(FileLinkResponse {
        token,
        filename,
        expires_in_seconds: FILE_LINK_TTL_SECONDS,
    }))
}

pub(crate) async fn shared_file_by_link(
    State(state): State<AppState>,
    Path((token, _filename)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[FILE_LINK_ISSUER]);
    let claims = decode::<FileLinkClaims>(
        &token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized())?
    .claims;
    let summary = state
        .storage
        .get_artifact_summary(&claims.sub)
        .await
        .map_err(|_| ApiError::unauthorized())?;
    let bytes = state
        .storage
        .read_artifact_blob(&claims.sub)
        .await
        .map_err(map_storage_error)?;
    Ok(file_response(&summary, bytes, None))
}

pub(crate) async fn rename_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, folder_id)): Path<(String, String)>,
    Json(request): Json<RenameFolderRequest>,
) -> Result<Json<Folder>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    validate_folder(&state, &workspace_id, Some(&folder_id)).await?;
    let folder = state
        .storage
        .rename_folder(&folder_id, &request.name)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_renamed",
        "folder",
        Some(&folder.id),
        format!("Renamed folder to {}.", folder.name),
    )
    .await;
    Ok(Json(folder))
}

/// Deletes a folder together with every folder and file inside it.
pub(crate) async fn delete_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, folder_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, Some(&folder_id))
        .await?
        .ok_or_else(|| ApiError::bad_request("Folder was not found."))?;
    let folder = state.storage.get_folder(&folder_id).await.map_err(map_storage_error)?;
    let (folder_ids, artifact_ids) = state
        .storage
        .folder_subtree(&folder_id)
        .await
        .map_err(map_storage_error)?;
    for artifact_id in &artifact_ids {
        state
            .core
            .delete_artifact(artifact_id.clone())
            .await
            .map_err(map_core_error)?;
    }
    state
        .storage
        .delete_folders(&folder_ids)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_deleted",
        "folder",
        Some(&folder_id),
        format!(
            "Deleted folder {} with {} file(s) and {} subfolder(s).",
            folder.name,
            artifact_ids.len(),
            folder_ids.len() - 1
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn move_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<MoveArtifactRequest>,
) -> Result<Json<SharedArtifactSummary>, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    let workspace_id = current.summary.workspace_id.clone();
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, request.folder_id.as_deref()).await?;
    state
        .storage
        .move_artifact_to_folder(&artifact_id, folder_id.as_deref())
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_moved",
        "artifact",
        Some(&artifact_id),
        format!("Moved evidence: {}.", current.summary.title),
    )
    .await;
    Ok(Json(SharedArtifactSummary {
        summary: current.summary,
        folder_id,
        repository_id: None,
    }))
}

/// Checks that a folder belongs to the workspace before anything is stored in it.
pub(crate) async fn validate_folder(
    state: &AppState,
    workspace_id: &str,
    folder_id: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(folder_id) = folder_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let folder = state
        .storage
        .get_folder(folder_id)
        .await
        .map_err(|_| ApiError::bad_request("Folder was not found."))?;
    if folder.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Folder was not found."));
    }
    Ok(Some(folder.id))
}

pub(crate) async fn query_artifacts(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(page): Query<PageQuery>,
    Json(request): Json<QueryArtifactsRequest>,
) -> Result<(HeaderMap, Json<Vec<ArtifactSummary>>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let started = std::time::Instant::now();
    let query = request.query.trim().to_lowercase();
    let languages = request
        .languages
        .into_iter()
        .map(|language| language.to_lowercase())
        .collect::<Vec<_>>();
    let artifacts = state
        .core
        .list_artifacts(workspace_id.clone())
        .await
        .map_err(map_core_error)?
        .into_iter()
        .filter(|artifact| {
            let query_matches = query.is_empty()
                || artifact.title.to_lowercase().contains(&query)
                || artifact.path.to_lowercase().contains(&query);
            let type_matches = request.artifact_types.is_empty()
                || request
                    .artifact_types
                    .iter()
                    .any(|artifact_type| artifact_type == &artifact.artifact_type);
            let language_matches = languages.is_empty()
                || artifact
                    .language
                    .as_deref()
                    .map(|language| languages.iter().any(|candidate| candidate == &language.to_lowercase()))
                    .unwrap_or(false);
            let source_matches = request.source_ids.is_empty()
                || request
                    .source_ids
                    .iter()
                    .any(|source_id| source_id == &artifact.source_id);
            let indexing_matches = request
                .indexed
                .map(|indexed| artifact.indexed_at.is_some() == indexed)
                .unwrap_or(true);
            query_matches && type_matches && language_matches && source_matches && indexing_matches
        })
        .collect::<Vec<_>>();
    record_timing("artifact_query", &workspace_id, started, artifacts.len());
    let (headers, artifacts) = page.apply(artifacts);
    Ok((headers, Json(artifacts)))
}

pub(crate) async fn create_text_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateTextArtifactRequest>,
) -> Result<(StatusCode, Json<SharedArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, request.folder_id.as_deref()).await?;
    let artifact = state
        .core
        .import_text(workspace_id.clone(), request.title, request.content, request.language)
        .await
        .map_err(map_core_error)?;
    if folder_id.is_some() {
        state
            .storage
            .set_artifact_folder(&artifact.id, folder_id.as_deref())
            .await
            .map_err(map_storage_error)?;
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_stored",
        "artifact",
        Some(&artifact.id),
        format!("Stored evidence: {}.", artifact.title),
    )
    .await;
    state.index_queue.enqueue(&artifact, Some(&subject.user_id));
    Ok((
        StatusCode::CREATED,
        Json(SharedArtifactSummary {
            summary: artifact,
            folder_id,
            repository_id: None,
        }),
    ))
}

pub(crate) async fn upload_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<SharedArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let requested_folder = headers
        .get("x-repomemo-folder-id")
        .and_then(|value| value.to_str().ok());
    let folder_id = validate_folder(&state, &workspace_id, requested_folder).await?;
    let filename = headers
        .get("x-repomemo-filename")
        .and_then(decode_filename_header)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("X-RepoMemo-Filename is required for uploads."))?;
    if filename.chars().count() > MAX_UPLOAD_FILENAME_CHARS || filename.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "File names must be at most {MAX_UPLOAD_FILENAME_CHARS} characters long, without control characters."
        )));
    }
    let mime_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    state
        .core
        .ensure_upload_allowed(&workspace_id, &filename)
        .await
        .map_err(map_core_error)?;
    let artifact = state
        .core
        .import_upload(workspace_id.clone(), filename, body.to_vec(), mime_type)
        .await
        .map_err(map_core_error)?;
    // Start rendering the faithful preview now so it is ready when opened.
    state.converter.ensure(state.storage.clone(), artifact.clone());
    if folder_id.is_some() {
        state
            .storage
            .set_artifact_folder(&artifact.id, folder_id.as_deref())
            .await
            .map_err(map_storage_error)?;
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_uploaded",
        "artifact",
        Some(&artifact.id),
        format!("Uploaded evidence: {}.", artifact.title),
    )
    .await;
    state.index_queue.enqueue(&artifact, Some(&subject.user_id));
    Ok((
        StatusCode::CREATED,
        Json(SharedArtifactSummary {
            summary: artifact,
            folder_id,
            repository_id: None,
        }),
    ))
}

/// The upload's file name. Clients percent-encode it so names outside ASCII
/// survive the header; a plain ASCII name is used as it is.
pub(crate) fn decode_filename_header(value: &HeaderValue) -> Option<String> {
    let raw = std::str::from_utf8(value.as_bytes()).ok()?;
    if !raw.contains('%') {
        return Some(raw.to_owned());
    }
    percent_decode(raw).or_else(|| Some(raw.to_owned()))
}

/// Decodes `%XX` escapes; `None` when an escape is malformed or the result
/// is not UTF-8.
pub(crate) fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

pub(crate) async fn get_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ArtifactDetail>, ApiError> {
    let mut artifact = state.core.get_artifact(artifact_id).await.map_err(map_core_error)?;
    let role = require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    // Stored chunks are an administrative view of the index. Everyone else
    // works with the artifact content and search results.
    if !capabilities_for_role(role, false).can_inspect_index {
        artifact.chunks.clear();
    }
    Ok(Json(artifact))
}

pub(crate) async fn list_artifact_chunks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<Chunk>>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &summary.workspace_id).await?;
    state
        .storage
        .list_chunks_for_artifact(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn update_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<UpdateArtifactRequest>,
) -> Result<Json<ArtifactSummary>, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.summary.workspace_id).await?;
    repositories::ensure_not_repository_file(&state, &artifact_id).await?;
    let artifact = state
        .core
        .update_artifact_title(artifact_id, request.title)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &artifact.workspace_id,
        &subject.user_id,
        "artifact_updated",
        "artifact",
        Some(&artifact.id),
        format!("Renamed evidence to {}.", artifact.title),
    )
    .await;
    Ok(Json(artifact))
}

pub(crate) async fn delete_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.summary.workspace_id).await?;
    repositories::ensure_not_repository_file(&state, &artifact_id).await?;
    let title = current.summary.title;
    let workspace_id = current.summary.workspace_id;
    state
        .core
        .delete_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "artifact_deleted",
        "artifact",
        Some(&artifact_id),
        format!("Deleted evidence: {title}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn get_artifact_lifecycle(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ArtifactLifecycle>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .get_artifact_lifecycle(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn update_artifact_lifecycle(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<SaveArtifactLifecycleRequest>,
) -> Result<Json<ArtifactLifecycle>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let status = request.status.trim().to_ascii_lowercase();
    if !matches!(
        status.as_str(),
        "active" | "needs_review" | "verified" | "outdated" | "superseded"
    ) {
        return Err(ApiError::bad_request("Evidence lifecycle status is invalid."));
    }
    let review_note = request.review_note.trim().to_owned();
    if review_note.len() > 5_000 {
        return Err(ApiError::bad_request("Evidence lifecycle note is too long."));
    }
    let owner_user_id = request.owner_user_id.filter(|value| !value.trim().is_empty());
    if let Some(owner_user_id) = owner_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(owner_user_id, &artifact.summary.workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request("Evidence owner must be a workspace member."));
        }
    }
    let superseded_by_artifact_id = request
        .superseded_by_artifact_id
        .filter(|value| !value.trim().is_empty());
    if status == "superseded" && superseded_by_artifact_id.is_none() {
        return Err(ApiError::bad_request(
            "Choose the replacement evidence before marking this item superseded.",
        ));
    }
    if let Some(replacement_id) = superseded_by_artifact_id.as_deref() {
        if replacement_id == artifact_id {
            return Err(ApiError::bad_request("Evidence cannot supersede itself."));
        }
        let replacement = state
            .core
            .get_artifact(replacement_id.to_owned())
            .await
            .map_err(map_core_error)?;
        if replacement.summary.workspace_id != artifact.summary.workspace_id {
            return Err(ApiError::bad_request(
                "Replacement evidence must belong to this workspace.",
            ));
        }
    }
    let lifecycle = state
        .storage
        .save_artifact_lifecycle(
            &artifact_id,
            &subject.user_id,
            SaveArtifactLifecycle {
                status: status.clone(),
                owner_user_id,
                review_note,
                superseded_by_artifact_id,
            },
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &artifact.summary.workspace_id,
        &subject.user_id,
        "evidence_lifecycle_updated",
        "artifact",
        Some(&artifact_id),
        format!(
            "Updated evidence lifecycle for {} to {}.",
            artifact.summary.title, status
        ),
    )
    .await;
    Ok(Json(lifecycle))
}

pub(crate) async fn list_artifact_lifecycle_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<ArtifactLifecycleEvent>>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .list_artifact_lifecycle_events(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn index_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let job = state.core.index_artifact(artifact_id).await.map_err(map_core_error)?;
    state.embedding_queue.request(&artifact.summary.workspace_id);
    record_workspace_activity(
        &state,
        &artifact.summary.workspace_id,
        &subject.user_id,
        "artifact_indexed",
        "artifact",
        Some(&artifact.summary.id),
        format!("Indexed evidence: {}.", artifact.summary.title),
    )
    .await;
    Ok(Json(job))
}

/// Re-indexes the whole workspace inside the request. Administrators only:
/// it holds a connection and the server's indexing work for as long as it
/// runs, and new evidence is indexed in the background anyway.
pub(crate) async fn index_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let job = state
        .core
        .index_workspace(workspace_id.clone())
        .await
        .map_err(map_core_error)?;
    state.embedding_queue.request(&workspace_id);
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_indexed",
        "workspace",
        Some(&workspace_id),
        "Indexed the workspace evidence.".to_owned(),
    )
    .await;
    Ok(Json(job))
}

pub(crate) async fn list_workspace_jobs(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<Vec<IndexingJobStatus>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let limit = query.limit.unwrap_or(50);
    state
        .storage
        .list_workspace_jobs(&workspace_id, query.kind.as_deref(), query.status.as_deref(), limit)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn get_job(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let job = state
        .storage
        .get_job(&job_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("The job was not found."))?;
    require_workspace_read(&state, &subject, &job.workspace_id).await?;
    Ok(Json(job))
}

pub(crate) async fn cancel_job(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let job = state
        .storage
        .get_job(&job_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("The job was not found."))?;
    require_workspace_write(&state, &subject, &job.workspace_id).await?;
    let updated = state
        .storage
        .request_job_cancel(&job_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(updated))
}

/// Moves a folder, with its contents, below another folder or to the top level.
pub(crate) async fn move_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, folder_id)): Path<(String, String)>,
    Json(request): Json<MoveFolderRequest>,
) -> Result<Json<Folder>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    validate_folder(&state, &workspace_id, Some(&folder_id)).await?;
    let parent_id = validate_folder(&state, &workspace_id, request.parent_id.as_deref()).await?;
    let folder = state
        .storage
        .move_folder(&folder_id, parent_id.as_deref())
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_moved",
        "folder",
        Some(&folder.id),
        format!("Moved folder {}.", folder.name),
    )
    .await;
    Ok(Json(folder))
}

const MAX_BULK_ARTIFACTS: usize = 200;

/// Moves, deletes or changes the evidence state of many files at once. Each
/// file is handled on its own, so one that cannot be (a repository file, a
/// file already gone) does not stop the others; the answer lists both.
pub(crate) async fn bulk_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<BulkArtifactsRequest>,
) -> Result<Json<BulkArtifactsResponse>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    if request.artifact_ids.is_empty() || request.artifact_ids.len() > MAX_BULK_ARTIFACTS {
        return Err(ApiError::bad_request(format!(
            "Choose between 1 and {MAX_BULK_ARTIFACTS} files."
        )));
    }
    let folder_id = match request.action.as_str() {
        "move" => validate_folder(&state, &workspace_id, request.folder_id.as_deref()).await?,
        "delete" => None,
        "lifecycle" => {
            if !matches!(
                request.status.as_deref(),
                Some("active" | "needs_review" | "verified" | "outdated")
            ) {
                return Err(ApiError::bad_request(
                    "Choose active, needs_review, verified or outdated.",
                ));
            }
            None
        }
        _ => return Err(ApiError::bad_request("The action is move, delete or lifecycle.")),
    };
    let mut done = Vec::new();
    let mut failed = Vec::new();
    for artifact_id in &request.artifact_ids {
        let outcome: Result<(), ApiError> = async {
            let summary = state
                .storage
                .get_artifact_summary(artifact_id)
                .await
                .map_err(map_storage_error)?;
            if summary.workspace_id != workspace_id {
                return Err(ApiError::not_found("Artifact was not found."));
            }
            match request.action.as_str() {
                "move" => {
                    repositories::ensure_not_repository_file(&state, artifact_id).await?;
                    state
                        .storage
                        .move_artifact_to_folder(artifact_id, folder_id.as_deref())
                        .await
                        .map_err(map_storage_error)
                }
                "delete" => {
                    repositories::ensure_not_repository_file(&state, artifact_id).await?;
                    state
                        .core
                        .delete_artifact(artifact_id.clone())
                        .await
                        .map_err(map_core_error)
                }
                _ => {
                    let current = state
                        .storage
                        .get_artifact_lifecycle(artifact_id)
                        .await
                        .map_err(map_storage_error)?;
                    state
                        .storage
                        .save_artifact_lifecycle(
                            artifact_id,
                            &subject.user_id,
                            SaveArtifactLifecycle {
                                status: request.status.clone().unwrap_or_default(),
                                owner_user_id: current.owner.map(|owner| owner.id),
                                review_note: current.review_note,
                                superseded_by_artifact_id: None,
                            },
                        )
                        .await
                        .map(|_| ())
                        .map_err(map_storage_error)
                }
            }
        }
        .await;
        match outcome {
            Ok(()) => done.push(artifact_id.clone()),
            Err(error) => failed.push(BulkFailure {
                artifact_id: artifact_id.clone(),
                message: error.message,
            }),
        }
    }
    if !done.is_empty() {
        let (action, verb) = match request.action.as_str() {
            "move" => ("evidence_moved", "Moved"),
            "delete" => ("artifact_deleted", "Deleted"),
            _ => ("evidence_lifecycle_updated", "Changed the state of"),
        };
        record_workspace_activity(
            &state,
            &workspace_id,
            &subject.user_id,
            action,
            "artifact",
            None,
            format!(
                "{verb} {} {}.",
                done.len(),
                if done.len() == 1 { "file" } else { "files" }
            ),
        )
        .await;
    }
    Ok(Json(BulkArtifactsResponse { done, failed }))
}

/// Uploads a new version of a file into the same artifact.
pub(crate) async fn upload_artifact_version(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SharedArtifactSummary>, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    let workspace_id = current.summary.workspace_id.clone();
    require_workspace_write(&state, &subject, &workspace_id).await?;
    repositories::ensure_not_repository_file(&state, &artifact_id).await?;
    let filename = headers
        .get("x-repomemo-filename")
        .and_then(decode_filename_header)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("X-RepoMemo-Filename is required for uploads."))?;
    if filename.chars().count() > MAX_UPLOAD_FILENAME_CHARS || filename.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "The file name is too long or has control characters.",
        ));
    }
    let artifact = state
        .core
        .replace_artifact_version(&artifact_id, &filename, body.to_vec())
        .await
        .map_err(map_core_error)?;
    state.converter.ensure(state.storage.clone(), artifact.clone());
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "artifact_updated",
        "artifact",
        Some(&artifact.id),
        format!("Uploaded a new version of {}.", artifact.title),
    )
    .await;
    state.index_queue.enqueue(&artifact, Some(&subject.user_id));
    let folder_id = None;
    Ok(Json(SharedArtifactSummary {
        summary: artifact,
        folder_id,
        repository_id: None,
    }))
}
