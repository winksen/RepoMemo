use super::*;

impl StorageEngine {
    pub async fn create_memory_card(
        &self,
        workspace_id: &str,
        title: &str,
        body_markdown: &str,
        source: &str,
        confidence: Option<f64>,
        citations: &[Citation],
    ) -> Result<MemoryCard> {
        for citation in citations {
            self.validate_memory_evidence(workspace_id, citation).await?;
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO memory_cards (
              id, workspace_id, title, body_markdown, source, confidence,
              created_at, updated_at, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '{}')
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(title.trim())
        .bind(body_markdown.trim())
        .bind(source.trim())
        .bind(confidence)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for citation in citations {
            insert_citation_link(&self.pool, workspace_id, &id, citation, &now).await?;
        }

        self.get_memory_card(&id).await.map(|detail| detail.card)
    }

    pub async fn update_memory_card(
        &self,
        card_id: &str,
        title: &str,
        body_markdown: &str,
        source: &str,
        confidence: Option<f64>,
        citations: Option<&[Citation]>,
    ) -> Result<MemoryCard> {
        let now = Utc::now().to_rfc3339();
        // New citations are checked before anything changes, against the
        // card's own workspace.
        let workspace_id = match citations {
            Some(citations) => {
                let workspace_id = self.get_memory_card(card_id).await?.card.workspace_id;
                for citation in citations {
                    self.validate_memory_evidence(&workspace_id, citation).await?;
                }
                Some(workspace_id)
            }
            None => None,
        };
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            r#"
            UPDATE memory_cards
            SET title = ?1, body_markdown = ?2, source = ?3, confidence = ?4, updated_at = ?5
            WHERE id = ?6
            "#,
        )
        .bind(title.trim())
        .bind(body_markdown.trim())
        .bind(source.trim())
        .bind(confidence)
        .bind(&now)
        .bind(card_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Memory card was not found.");
        }
        if let (Some(citations), Some(workspace_id)) = (citations, workspace_id.as_deref()) {
            sqlx::query(
                "DELETE FROM links WHERE from_type = 'memory_card' AND from_id = ?1 AND relation_type = 'cites'",
            )
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
            for citation in citations {
                insert_citation_link(&mut *tx, workspace_id, card_id, citation, &now).await?;
            }
        }
        tx.commit().await?;
        self.get_memory_card(card_id).await.map(|detail| detail.card)
    }

    pub async fn delete_memory_card(&self, card_id: &str) -> Result<()> {
        let detail = self.get_memory_card(card_id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM links WHERE workspace_id = ?1 AND from_type = 'memory_card' AND from_id = ?2")
            .bind(&detail.card.workspace_id)
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM memory_cards WHERE id = ?1")
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_memory_cards(&self, workspace_id: &str) -> Result<Vec<MemoryCardSummary>> {
        self.query_memory_cards(workspace_id, None).await
    }

    pub async fn search_memory_cards(&self, workspace_id: &str, query: &str) -> Result<Vec<MemoryCardSummary>> {
        self.query_memory_cards(workspace_id, Some(query.trim())).await
    }

    pub async fn get_memory_card(&self, card_id: &str) -> Result<MemoryCardDetail> {
        let row = sqlx::query_as::<_, MemoryCardRow>(
            r#"
            SELECT id, workspace_id, title, body_markdown, source, confidence,
                   created_at, updated_at, metadata_json
            FROM memory_cards WHERE id = ?1
            "#,
        )
        .bind(card_id)
        .fetch_one(self.reader())
        .await?;
        let evidence_rows = sqlx::query_as::<_, MemoryEvidenceRow>(
            r#"
            SELECT
              links.id AS link_id,
              links.to_id AS target_id,
              links.to_type AS target_type,
              artifacts.id AS artifact_id,
              chunks.id AS chunk_id,
              artifacts.title AS title,
              artifacts.path AS path,
              chunks.start_line AS start_line,
              chunks.end_line AS end_line
            FROM links
            LEFT JOIN chunks ON links.to_type = 'chunk' AND chunks.id = links.to_id
            LEFT JOIN artifacts ON artifacts.id = CASE
              WHEN links.to_type = 'artifact' THEN links.to_id
              ELSE chunks.artifact_id
            END
            WHERE links.from_type = 'memory_card' AND links.from_id = ?1
            ORDER BY links.created_at ASC
            "#,
        )
        .bind(card_id)
        .fetch_all(self.reader())
        .await?;
        let review = self
            .memory_review_reasons(&row.workspace_id, Some(card_id))
            .await?
            .remove(card_id)
            .unwrap_or_default();
        Ok(MemoryCardDetail {
            card: MemoryCard::from(row),
            evidence: evidence_rows.into_iter().map(MemoryEvidence::from).collect(),
            review,
        })
    }

    pub(crate) async fn query_memory_cards(
        &self,
        workspace_id: &str,
        search: Option<&str>,
    ) -> Result<Vec<MemoryCardSummary>> {
        let rows = if let Some(search) = search.filter(|value| !value.is_empty()) {
            sqlx::query_as::<_, MemoryCardSummaryRow>(
                r#"
                SELECT memory_cards.id, memory_cards.workspace_id, memory_cards.title,
                       substr(memory_cards.body_markdown, 1, 220) AS body_excerpt,
                       memory_cards.source, COUNT(links.id) AS evidence_count,
                       memory_cards.created_at, memory_cards.updated_at
                FROM memory_cards
                LEFT JOIN links ON links.from_type = 'memory_card' AND links.from_id = memory_cards.id
                WHERE memory_cards.workspace_id = ?1
                  AND lower(memory_cards.title || ' ' || memory_cards.body_markdown) LIKE '%' || lower(?2) || '%'
                GROUP BY memory_cards.id
                ORDER BY memory_cards.updated_at DESC, memory_cards.title ASC
                "#,
            )
            .bind(workspace_id)
            .bind(search)
            .fetch_all(self.reader())
            .await?
        } else {
            sqlx::query_as::<_, MemoryCardSummaryRow>(
                r#"
                SELECT memory_cards.id, memory_cards.workspace_id, memory_cards.title,
                       substr(memory_cards.body_markdown, 1, 220) AS body_excerpt,
                       memory_cards.source, COUNT(links.id) AS evidence_count,
                       memory_cards.created_at, memory_cards.updated_at
                FROM memory_cards
                LEFT JOIN links ON links.from_type = 'memory_card' AND links.from_id = memory_cards.id
                WHERE memory_cards.workspace_id = ?1
                GROUP BY memory_cards.id
                ORDER BY memory_cards.updated_at DESC, memory_cards.title ASC
                "#,
            )
            .bind(workspace_id)
            .fetch_all(self.reader())
            .await?
        };
        let mut review = self.memory_review_reasons(workspace_id, None).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let mut summary = MemoryCardSummary::from(row);
                summary.needs_review = review.remove(&summary.id).is_some_and(|reasons| !reasons.is_empty());
                summary
            })
            .collect())
    }

    /// Why memory cards need another look, by card id: a cited passage or file
    /// that no longer exists, a cited file marked outdated or superseded, or a
    /// cited file updated after the card was last written. Cards whose
    /// evidence is as it was are absent. Passages are identified by content,
    /// so a cited passage that is still there has not changed.
    pub async fn memory_review_reasons(
        &self,
        workspace_id: &str,
        only_card: Option<&str>,
    ) -> Result<HashMap<String, Vec<MemoryReviewReason>>> {
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT links.from_id AS card_id, links.to_type, links.to_id,
                   memory_cards.updated_at AS card_updated_at,
                   chunks.id AS chunk_id,
                   target.id AS artifact_id, target.title AS title, target.updated_at AS artifact_updated_at,
                   artifact_lifecycle.status AS status,
                   successor.title AS successor_title
            FROM links
            JOIN memory_cards ON memory_cards.id = links.from_id
            LEFT JOIN chunks ON links.to_type = 'chunk' AND chunks.id = links.to_id
            LEFT JOIN artifacts AS target ON target.id = CASE
              WHEN links.to_type = 'artifact' THEN links.to_id
              ELSE chunks.artifact_id
            END
            LEFT JOIN artifact_lifecycle ON artifact_lifecycle.artifact_id = target.id
            LEFT JOIN artifacts AS successor ON successor.id = artifact_lifecycle.superseded_by_artifact_id
            WHERE links.workspace_id = "#,
        );
        query.push_bind(workspace_id);
        query.push(" AND links.from_type = 'memory_card'");
        if let Some(card_id) = only_card {
            query.push(" AND links.from_id = ");
            query.push_bind(card_id);
        }
        let rows = query.build().fetch_all(self.reader()).await?;
        let mut reasons: HashMap<String, Vec<MemoryReviewReason>> = HashMap::new();
        for row in rows {
            let card_id: String = row.get("card_id");
            let to_type: String = row.get("to_type");
            let artifact_id: Option<String> = row.get("artifact_id");
            let title: Option<String> = row.get("title");
            let reason = if artifact_id.is_none() {
                Some(MemoryReviewReason {
                    kind: "missing".to_owned(),
                    artifact_id: None,
                    title: None,
                    detail: if to_type == "chunk" {
                        "A passage this card cites was changed or removed.".to_owned()
                    } else {
                        "A file this card cites no longer exists.".to_owned()
                    },
                })
            } else if matches!(
                row.get::<Option<String>, _>("status").as_deref(),
                Some("outdated" | "superseded")
            ) {
                let name = title.clone().unwrap_or_default();
                let successor: Option<String> = row.get("successor_title");
                Some(MemoryReviewReason {
                    kind: "retired".to_owned(),
                    artifact_id: artifact_id.clone(),
                    title: title.clone(),
                    detail: match successor {
                        Some(next) => format!("{name} is marked outdated; {next} replaces it."),
                        None => format!("{name} is marked outdated."),
                    },
                })
            } else if to_type == "artifact"
                && row.get::<Option<String>, _>("artifact_updated_at").as_deref()
                    > Some(row.get::<String, _>("card_updated_at").as_str())
            {
                let name = title.clone().unwrap_or_default();
                Some(MemoryReviewReason {
                    kind: "changed".to_owned(),
                    artifact_id: artifact_id.clone(),
                    title: title.clone(),
                    detail: format!("{name} was updated after this card was written."),
                })
            } else {
                None
            };
            if let Some(reason) = reason {
                reasons.entry(card_id).or_default().push(reason);
            }
        }
        Ok(reasons)
    }

    pub(crate) async fn validate_memory_evidence(&self, workspace_id: &str, citation: &Citation) -> Result<()> {
        let exists = if let Some(chunk_id) = &citation.chunk_id {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chunks WHERE id = ?1 AND workspace_id = ?2")
                .bind(chunk_id)
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts WHERE id = ?1 AND workspace_id = ?2")
                .bind(&citation.artifact_id)
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?
        };
        if exists == 0 {
            repomemo_domain::bail_invalid!("A cited evidence record is missing from this workspace.");
        }
        Ok(())
    }
}

/// Links a memory card to what it cites: the passage when there is one, else
/// the whole file.
async fn insert_citation_link<'e, E>(
    executor: E,
    workspace_id: &str,
    card_id: &str,
    citation: &Citation,
    now: &str,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let (target_id, target_type) = citation
        .chunk_id
        .as_ref()
        .map(|chunk_id| (chunk_id.as_str(), "chunk"))
        .unwrap_or((citation.artifact_id.as_str(), "artifact"));
    sqlx::query(
        r#"
        INSERT INTO links (
          id, workspace_id, from_id, from_type, to_id, to_type,
          relation_type, confidence, created_by, metadata_json, created_at
        ) VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, 'cites', ?6, 'user', '{}', ?7)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(workspace_id)
    .bind(card_id)
    .bind(target_id)
    .bind(target_type)
    .bind(citation.confidence)
    .bind(now)
    .execute(executor)
    .await?;
    Ok(())
}
