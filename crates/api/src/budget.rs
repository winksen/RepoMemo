//! A daily allowance of AI calls the server makes on its own (image
//! descriptions and embedding batches), per workspace (review M12). Calls
//! people ask for (Ask, overviews, the assistant) have their own per-person
//! quota in the server.
//!
//! Counts live in memory and start again at midnight UTC and after a restart,
//! which only ever errs on the side of allowing work.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{NaiveDate, Utc};

#[derive(Debug, Default)]
struct Day {
    date: Option<NaiveDate>,
    spent: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundAiBudget {
    /// Calls per workspace per day; 0 is unlimited. Shared so the server's
    /// run-time settings can change it.
    limit: Arc<AtomicU32>,
    day: Arc<Mutex<Day>>,
}

impl BackgroundAiBudget {
    /// The handle that sets the daily limit.
    pub fn limit_handle(&self) -> Arc<AtomicU32> {
        self.limit.clone()
    }

    pub fn daily_limit(&self) -> u32 {
        self.limit.load(Ordering::Relaxed)
    }

    /// Spends `calls` of the workspace's allowance for today. Returns false,
    /// and spends nothing, when that would go over the limit.
    pub fn try_spend(&self, workspace_id: &str, calls: u32) -> bool {
        self.try_spend_on(Utc::now().date_naive(), workspace_id, calls)
    }

    fn try_spend_on(&self, today: NaiveDate, workspace_id: &str, calls: u32) -> bool {
        let limit = self.daily_limit();
        if limit == 0 {
            return true;
        }
        let Ok(mut day) = self.day.lock() else {
            return true;
        };
        if day.date != Some(today) {
            day.date = Some(today);
            day.spent.clear();
        }
        let spent = day.spent.entry(workspace_id.to_owned()).or_insert(0);
        if spent.saturating_add(calls) > limit {
            return false;
        }
        *spent += calls;
        true
    }

    /// Calls made for the workspace today.
    pub fn spent_today(&self, workspace_id: &str) -> u32 {
        let today = Utc::now().date_naive();
        self.day
            .lock()
            .ok()
            .filter(|day| day.date == Some(today))
            .and_then(|day| day.spent.get(workspace_id).copied())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allowance_is_per_workspace_and_per_day() {
        let budget = BackgroundAiBudget::default();
        let monday = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let tuesday = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        assert!(budget.try_spend_on(monday, "a", 1_000), "no limit set");

        budget.limit_handle().store(3, Ordering::Relaxed);
        assert!(budget.try_spend_on(monday, "a", 2));
        assert!(!budget.try_spend_on(monday, "a", 2), "would go over, spends nothing");
        assert!(budget.try_spend_on(monday, "a", 1));
        assert!(!budget.try_spend_on(monday, "a", 1));
        assert!(budget.try_spend_on(monday, "b", 3), "another workspace has its own");
        assert!(budget.try_spend_on(tuesday, "a", 3), "a new day starts again");
    }
}
