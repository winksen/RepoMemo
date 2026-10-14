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
            .bind(&id)
            .bind(target_id)
            .bind(target_type)
            .bind(citation.confidence)
            .bind(&now)
            .execute(&self.pool)
            .await?;
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
    ) -> Result<MemoryCard> {
        let now = Utc::now().to_rfc3339();
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
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Memory card was not found.");
        }
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
        Ok(MemoryCardDetail {
            card: MemoryCard::from(row),
            evidence: evidence_rows.into_iter().map(MemoryEvidence::from).collect(),
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
        Ok(rows.into_iter().map(MemoryCardSummary::from).collect())
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
