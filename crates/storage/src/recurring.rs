//! Recurring work (V0.7.16): tasks made again on an explicit schedule. An occurrence is a date, and (recurring task, date) is its
//! identity, so a retry or a restart cannot make it twice; each pass makes its tasks in one transaction with the record of the
//! occurrence. Missed occurrences follow the schedule's policy, nothing is made while it is paused or after it is stopped, and
//! stopping keeps every task already made.

use super::*;
use chrono::{Datelike, Duration, NaiveDate};
use repomemo_domain::{RecurringOccurrence, RecurringSchedule, RecurringTask};

/// The most recurring tasks (active and paused) one workspace can have.
pub const MAX_RECURRING_TASKS: i64 = 50;
/// The most occurrence dates one pass looks back over; older ones are recorded as skipped.
const LOOK_BACK_LIMIT: usize = 400;

/// What a person sets on a recurring task.
#[derive(Debug, Clone, Default)]
pub struct NewRecurringTask {
    pub title: String,
    pub description: String,
    pub priority: String,
    pub assignee_user_id: Option<String>,
    pub project_id: Option<String>,
    pub due_after_days: Option<i64>,
    pub schedule: Option<RecurringSchedule>,
    /// `latest` or `skip`.
    pub missed_policy: String,
    /// Starts paused: nothing is made until it is resumed.
    pub paused: bool,
}

/// Says what is wrong with a schedule, or nothing.
pub fn check_schedule(schedule: &RecurringSchedule) -> std::result::Result<NaiveDate, String> {
    let start = NaiveDate::parse_from_str(&schedule.start_date, "%Y-%m-%d")
        .map_err(|_| "The start date must be a calendar date, YYYY-MM-DD.".to_owned())?;
    if !(1..=60).contains(&schedule.interval) {
        return Err("The interval must be between 1 and 60.".to_owned());
    }
    match schedule.kind.as_str() {
        "daily" => {}
        "weekly" => {
            if !schedule.weekday.is_some_and(|day| (0..=6).contains(&day)) {
                return Err("A weekly schedule needs a weekday from 0 (Monday) to 6 (Sunday).".to_owned());
            }
        }
        "monthly" => {
            if !schedule.day_of_month.is_some_and(|day| (1..=28).contains(&day)) {
                return Err("A monthly schedule needs a day of the month from 1 to 28.".to_owned());
            }
        }
        _ => return Err("The schedule must be daily, weekly or monthly.".to_owned()),
    }
    Ok(start)
}

/// The occurrence dates of a schedule after `after` (exclusive) and up to `until` (inclusive), oldest first, at most `limit`.
pub fn occurrence_dates(
    schedule: &RecurringSchedule,
    after: Option<NaiveDate>,
    until: NaiveDate,
    limit: usize,
) -> Vec<NaiveDate> {
    let Ok(start) = check_schedule(schedule) else {
        return Vec::new();
    };
    let interval = schedule.interval;
    // The first occurrence on or after the start date, and how to reach the one that is `k` steps later.
    let first = match schedule.kind.as_str() {
        "weekly" => {
            let wanted = schedule.weekday.unwrap_or(0);
            let behind = (wanted - i64::from(start.weekday().num_days_from_monday())).rem_euclid(7);
            start + Duration::days(behind)
        }
        "monthly" => {
            let day = schedule.day_of_month.unwrap_or(1) as u32;
            let this_month = NaiveDate::from_ymd_opt(start.year(), start.month(), day);
            match this_month {
                Some(date) if date >= start => date,
                _ => {
                    let next = start.year() * 12 + start.month0() as i32 + 1;
                    match NaiveDate::from_ymd_opt(next.div_euclid(12), next.rem_euclid(12) as u32 + 1, day) {
                        Some(date) => date,
                        None => return Vec::new(),
                    }
                }
            }
        }
        _ => start,
    };
    let at = |k: i64| -> Option<NaiveDate> {
        match schedule.kind.as_str() {
            "weekly" => first.checked_add_signed(Duration::days(k * interval * 7)),
            "monthly" => {
                let total = first.year() as i64 * 12 + first.month0() as i64 + k * interval;
                NaiveDate::from_ymd_opt(
                    total.div_euclid(12) as i32,
                    total.rem_euclid(12) as u32 + 1,
                    first.day(),
                )
            }
            _ => first.checked_add_signed(Duration::days(k * interval)),
        }
    };
    // Jump close to `after` instead of walking from the start.
    let mut k = match after {
        Some(after) if after > first => {
            let steps = match schedule.kind.as_str() {
                "weekly" => (after - first).num_days() / (interval * 7),
                "monthly" => {
                    let months = (after.year() as i64 - first.year() as i64) * 12
                        + (after.month0() as i64 - first.month0() as i64);
                    months / interval
                }
                _ => (after - first).num_days() / interval,
            };
            (steps - 1).max(0)
        }
        _ => 0,
    };
    let mut found = Vec::new();
    while found.len() < limit {
        let Some(date) = at(k) else { break };
        if date > until {
            break;
        }
        if after.is_none_or(|after| date > after) {
            found.push(date);
        }
        k += 1;
    }
    found
}

fn schedule_of(row: &SqliteRow) -> RecurringSchedule {
    RecurringSchedule {
        kind: row.get("schedule_kind"),
        interval: row.get("interval_count"),
        weekday: row.get("weekday"),
        day_of_month: row.get("day_of_month"),
        start_date: row.get("start_date"),
    }
}

const SELECT_RECURRING: &str = r#"
    SELECT recurring.id, recurring.workspace_id, recurring.title, recurring.description, recurring.priority,
           recurring.project_id, recurring.due_after_days, recurring.schedule_kind, recurring.interval_count,
           recurring.weekday, recurring.day_of_month, recurring.start_date, recurring.missed_policy, recurring.status,
           recurring.processed_through, recurring.created_at, recurring.updated_at, recurring.stopped_at,
           assignee.id AS assignee_id, assignee.display_name AS assignee_name, assignee.email AS assignee_email,
           creator.id AS creator_id, creator.display_name AS creator_name, creator.email AS creator_email,
           (SELECT COUNT(*) FROM recurring_occurrences AS made WHERE made.recurring_id = recurring.id AND made.outcome = 'created') AS made_count
    FROM recurring_tasks AS recurring
    LEFT JOIN users AS assignee ON assignee.id = recurring.assignee_user_id
    LEFT JOIN users AS creator ON creator.id = recurring.created_by_user_id"#;

fn person(row: &SqliteRow, prefix: &str) -> Option<SharedUser> {
    let id: Option<String> = row.get(format!("{prefix}_id").as_str());
    id.map(|id| SharedUser {
        id,
        display_name: row.get(format!("{prefix}_name").as_str()),
        email: row.get(format!("{prefix}_email").as_str()),
    })
}

fn recurring_from(row: &SqliteRow, today: NaiveDate) -> RecurringTask {
    let schedule = schedule_of(row);
    let status: String = row.get("status");
    let upcoming = if status == "stopped" {
        Vec::new()
    } else {
        occurrence_dates(
            &schedule,
            Some(today - Duration::days(1)),
            today + Duration::days(800),
            5,
        )
        .into_iter()
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect()
    };
    RecurringTask {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        title: row.get("title"),
        description: row.get("description"),
        priority: row.get("priority"),
        assignee: person(row, "assignee"),
        project_id: row.get("project_id"),
        due_after_days: row.get("due_after_days"),
        schedule,
        missed_policy: row.get("missed_policy"),
        status,
        processed_through: row.get("processed_through"),
        next_occurrences: upcoming,
        occurrences_made: row.get("made_count"),
        created_by: person(row, "creator"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        stopped_at: row.get("stopped_at"),
    }
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

impl StorageEngine {
    pub async fn list_recurring_tasks(&self, workspace_id: &str) -> Result<Vec<RecurringTask>> {
        let rows = sqlx::query(&format!(
            "{SELECT_RECURRING} WHERE recurring.workspace_id = ?1 ORDER BY CASE recurring.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END, recurring.title COLLATE NOCASE ASC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let today = today();
        Ok(rows.iter().map(|row| recurring_from(row, today)).collect())
    }

    pub async fn get_recurring_task(&self, recurring_id: &str) -> Result<RecurringTask> {
        sqlx::query(&format!("{SELECT_RECURRING} WHERE recurring.id = ?1"))
            .bind(recurring_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| recurring_from(&row, today()))
            .ok_or_else(|| repomemo_domain::not_found!("Recurring task was not found."))
    }

    /// The occurrences looked at so far, newest first.
    pub async fn list_recurring_occurrences(&self, recurring_id: &str, limit: i64) -> Result<Vec<RecurringOccurrence>> {
        let rows = sqlx::query(
            "SELECT occurrence.occurrence_date, occurrence.outcome, occurrence.task_id, occurrence.created_at,
                    task.number AS task_number, task.title AS task_title, task.status AS task_status
             FROM recurring_occurrences AS occurrence
             LEFT JOIN workspace_tasks AS task ON task.id = occurrence.task_id
             WHERE occurrence.recurring_id = ?1 ORDER BY occurrence.occurrence_date DESC LIMIT ?2",
        )
        .bind(recurring_id)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let number: Option<i64> = row.get("task_number");
                RecurringOccurrence {
                    occurrence_date: row.get("occurrence_date"),
                    outcome: row.get("outcome"),
                    task_id: row.get("task_id"),
                    task_key: number.map(|number| format!("T-{number}")),
                    task_title: row.get("task_title"),
                    task_status: row.get("task_status"),
                }
            })
            .collect())
    }

    pub async fn create_recurring_task(
        &self,
        workspace_id: &str,
        created_by: &str,
        recurring: NewRecurringTask,
    ) -> Result<RecurringTask> {
        let schedule = recurring
            .schedule
            .ok_or_else(|| repomemo_domain::invalid!("A recurring task needs a schedule."))?;
        let start = check_schedule(&schedule).map_err(|message| repomemo_domain::invalid!("{message}"))?;
        let held: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM recurring_tasks WHERE workspace_id = ?1 AND status != 'stopped'")
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?;
        if held >= MAX_RECURRING_TASKS {
            repomemo_domain::bail_conflict!("A workspace can have at most {MAX_RECURRING_TASKS} recurring tasks.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        // Looked at through the day before the start, so the start date itself is the first occurrence.
        let processed_through = (start - Duration::days(1)).format("%Y-%m-%d").to_string();
        sqlx::query(
            "INSERT INTO recurring_tasks (id, workspace_id, title, description, priority, assignee_user_id, project_id, due_after_days,
                schedule_kind, interval_count, weekday, day_of_month, start_date, missed_policy, status, processed_through,
                created_by_user_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?18)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(recurring.title)
        .bind(recurring.description)
        .bind(recurring.priority)
        .bind(recurring.assignee_user_id)
        .bind(recurring.project_id)
        .bind(recurring.due_after_days)
        .bind(&schedule.kind)
        .bind(schedule.interval)
        .bind(schedule.weekday)
        .bind(schedule.day_of_month)
        .bind(&schedule.start_date)
        .bind(recurring.missed_policy)
        .bind(if recurring.paused { "paused" } else { "active" })
        .bind(processed_through)
        .bind(created_by)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_recurring_task(&id).await
    }

    /// Changes what a schedule makes and when. Looked-at dates stay looked at, so editing never makes the past again. A
    /// stopped schedule is not edited.
    pub async fn update_recurring_task(
        &self,
        recurring_id: &str,
        recurring: NewRecurringTask,
    ) -> Result<RecurringTask> {
        let schedule = recurring
            .schedule
            .ok_or_else(|| repomemo_domain::invalid!("A recurring task needs a schedule."))?;
        check_schedule(&schedule).map_err(|message| repomemo_domain::invalid!("{message}"))?;
        let changed = sqlx::query(
            "UPDATE recurring_tasks SET title = ?2, description = ?3, priority = ?4, assignee_user_id = ?5, project_id = ?6,
                due_after_days = ?7, schedule_kind = ?8, interval_count = ?9, weekday = ?10, day_of_month = ?11,
                start_date = ?12, missed_policy = ?13, updated_at = ?14
             WHERE id = ?1 AND status != 'stopped'",
        )
        .bind(recurring_id)
        .bind(recurring.title)
        .bind(recurring.description)
        .bind(recurring.priority)
        .bind(recurring.assignee_user_id)
        .bind(recurring.project_id)
        .bind(recurring.due_after_days)
        .bind(&schedule.kind)
        .bind(schedule.interval)
        .bind(schedule.weekday)
        .bind(schedule.day_of_month)
        .bind(&schedule.start_date)
        .bind(recurring.missed_policy)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            self.get_recurring_task(recurring_id).await?;
            repomemo_domain::bail_conflict!("A stopped recurring task is not changed; make a new one.");
        }
        self.get_recurring_task(recurring_id).await
    }

    /// Pauses, resumes or stops a schedule. Resuming does not make up what a pause skipped: looking resumes from yesterday, so
    /// only today's occurrence can be made. Stopping is final and keeps every task the schedule made.
    pub async fn set_recurring_status(&self, recurring_id: &str, status: &str) -> Result<RecurringTask> {
        let current = self.get_recurring_task(recurring_id).await?;
        if current.status == "stopped" {
            repomemo_domain::bail_conflict!("A stopped recurring task cannot be started again; make a new one.");
        }
        let now = Utc::now().to_rfc3339();
        let yesterday = (today() - Duration::days(1)).format("%Y-%m-%d").to_string();
        sqlx::query(
            "UPDATE recurring_tasks SET status = ?2,
                processed_through = CASE WHEN ?2 = 'active' AND processed_through < ?3 THEN ?3 ELSE processed_through END,
                stopped_at = CASE WHEN ?2 = 'stopped' THEN ?4 ELSE stopped_at END, updated_at = ?4
             WHERE id = ?1",
        )
        .bind(recurring_id)
        .bind(status)
        .bind(yesterday)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_recurring_task(recurring_id).await
    }

    /// Makes the tasks that are owed for every active schedule up to `today`. Returns the tasks made, so the caller can tell
    /// their assignees. Each schedule is one transaction: its occurrence records, its tasks and the date it has been looked at
    /// through are written together, so a crash leaves nothing half made and a second pass makes nothing again.
    pub async fn make_due_recurring_tasks(&self, today: NaiveDate) -> Result<Vec<CollaborationTask>> {
        let today_text = today.format("%Y-%m-%d").to_string();
        let due = sqlx::query(&format!(
            "{SELECT_RECURRING} WHERE recurring.status = 'active' AND recurring.processed_through < ?1 ORDER BY recurring.created_at ASC"
        ))
        .bind(&today_text)
        .fetch_all(self.reader())
        .await?;
        let mut made = Vec::new();
        for row in &due {
            let id: String = row.get("id");
            let workspace_id: String = row.get("workspace_id");
            let schedule = schedule_of(row);
            let policy: String = row.get("missed_policy");
            let through: String = row.get("processed_through");
            let after = NaiveDate::parse_from_str(&through, "%Y-%m-%d").ok();
            let pending = occurrence_dates(&schedule, after, today, LOOK_BACK_LIMIT);
            let make_date = match (policy.as_str(), pending.last()) {
                ("skip", Some(last)) if *last == today => Some(*last),
                ("skip", _) => None,
                (_, last) => last.copied(),
            };
            let creator: Option<String> = row
                .get::<Option<String>, _>("creator_id")
                .or_else(|| row.get::<Option<String>, _>("assignee_id"));
            let mut transaction = self.pool.begin().await?;
            let now = Utc::now().to_rfc3339();
            let mut new_task_id = None;
            for date in &pending {
                let date_text = date.format("%Y-%m-%d").to_string();
                let making = Some(*date) == make_date && creator.is_some();
                let claimed = sqlx::query(
                    "INSERT OR IGNORE INTO recurring_occurrences (recurring_id, occurrence_date, outcome, task_id, created_at)
                     VALUES (?1, ?2, ?3, NULL, ?4)",
                )
                .bind(&id)
                .bind(&date_text)
                .bind(if making { "created" } else { "skipped" })
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
                if !making || claimed.rows_affected() == 0 {
                    continue;
                }
                let task_id = Uuid::new_v4().to_string();
                let number = crate::collaboration::take_task_number(&mut transaction, &workspace_id).await?;
                let due_at: Option<i64> = row.get("due_after_days");
                let description: String = row.get("description");
                sqlx::query(
                    "INSERT INTO workspace_tasks (id, workspace_id, title, description, status, priority, assignee_user_id,
                        created_by_user_id, artifact_id, due_at, completed_at, created_at, updated_at, project_id, number)
                     VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?7, NULL, ?8, NULL, ?9, ?9, ?10, ?11)",
                )
                .bind(&task_id)
                .bind(&workspace_id)
                .bind(row.get::<String, _>("title"))
                .bind(if description.is_empty() {
                    format!("Made by a recurring task for {date_text}.")
                } else {
                    format!("{description}\n\nMade by a recurring task for {date_text}.")
                })
                .bind(row.get::<String, _>("priority"))
                .bind(row.get::<Option<String>, _>("assignee_id"))
                .bind(creator.clone())
                .bind(due_at.map(|days| (*date + Duration::days(days)).format("%Y-%m-%d").to_string()))
                .bind(&now)
                .bind(row.get::<Option<String>, _>("project_id"))
                .bind(number)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "UPDATE recurring_occurrences SET task_id = ?3 WHERE recurring_id = ?1 AND occurrence_date = ?2",
                )
                .bind(&id)
                .bind(&date_text)
                .bind(&task_id)
                .execute(&mut *transaction)
                .await?;
                new_task_id = Some(task_id);
            }
            sqlx::query("UPDATE recurring_tasks SET processed_through = ?2 WHERE id = ?1")
                .bind(&id)
                .bind(&today_text)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            if let Some(task_id) = new_task_id {
                made.push(self.get_collaboration_task(&task_id).await?);
            }
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(kind: &str, interval: i64, weekday: Option<i64>, day: Option<i64>, start: &str) -> RecurringSchedule {
        RecurringSchedule {
            kind: kind.to_owned(),
            interval,
            weekday,
            day_of_month: day,
            start_date: start.to_owned(),
        }
    }

    fn dates(found: Vec<NaiveDate>) -> Vec<String> {
        found
            .into_iter()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .collect()
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn daily_weekly_and_monthly_schedules_name_their_dates() {
        // 2026-12-07 is a Monday.
        assert_eq!(
            dates(occurrence_dates(
                &schedule("daily", 2, None, None, "2026-12-07"),
                None,
                date("2026-12-12"),
                10
            )),
            ["2026-12-07", "2026-12-09", "2026-12-11"]
        );
        assert_eq!(
            dates(occurrence_dates(
                &schedule("weekly", 1, Some(2), None, "2026-12-07"),
                None,
                date("2026-12-31"),
                10
            )),
            ["2026-12-09", "2026-12-16", "2026-12-23", "2026-12-30"]
        );
        assert_eq!(
            dates(occurrence_dates(
                &schedule("weekly", 2, Some(0), None, "2026-12-07"),
                None,
                date("2027-01-10"),
                10
            )),
            ["2026-12-07", "2026-12-21", "2027-01-04"]
        );
        assert_eq!(
            dates(occurrence_dates(
                &schedule("monthly", 1, None, Some(15), "2026-12-20"),
                None,
                date("2027-03-31"),
                10
            )),
            ["2027-01-15", "2027-02-15", "2027-03-15"]
        );
        assert_eq!(
            dates(occurrence_dates(
                &schedule("monthly", 3, None, Some(28), "2026-11-01"),
                None,
                date("2027-12-31"),
                10
            )),
            ["2026-11-28", "2027-02-28", "2027-05-28", "2027-08-28", "2027-11-28"]
        );
    }

    #[test]
    fn a_pass_looks_only_after_the_last_date_it_has_seen_and_jumps_ahead_cheaply() {
        let daily = schedule("daily", 1, None, None, "2020-01-01");
        let found = occurrence_dates(&daily, Some(date("2026-12-08")), date("2026-12-10"), 10);
        assert_eq!(dates(found), ["2026-12-09", "2026-12-10"]);
        let weekly = schedule("weekly", 1, Some(6), None, "2020-01-01");
        let found = occurrence_dates(&weekly, Some(date("2026-12-01")), date("2026-12-14"), 10);
        assert_eq!(dates(found), ["2026-12-06", "2026-12-13"]);
        assert!(
            occurrence_dates(&daily, None, date("2019-12-31"), 10).is_empty(),
            "nothing before the start"
        );
        assert_eq!(
            occurrence_dates(&daily, None, date("2030-01-01"), 3).len(),
            3,
            "the limit holds"
        );
    }

    #[test]
    fn a_schedule_is_checked_in_words() {
        let bad = |kind: &str, interval, weekday, day, start: &str| {
            check_schedule(&schedule(kind, interval, weekday, day, start)).unwrap_err()
        };
        assert!(bad("hourly", 1, None, None, "2026-12-07").contains("daily, weekly or monthly"));
        assert!(bad("daily", 0, None, None, "2026-12-07").contains("interval"));
        assert!(bad("daily", 1, None, None, "soon").contains("start date"));
        assert!(bad("weekly", 1, Some(7), None, "2026-12-07").contains("weekday"));
        assert!(bad("monthly", 1, None, Some(31), "2026-12-07").contains("1 to 28"));
        assert!(check_schedule(&schedule("monthly", 1, None, Some(28), "2026-12-07")).is_ok());
    }
}
