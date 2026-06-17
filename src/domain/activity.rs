//! Activity Log capability: scope/window value types + provider trait.

use async_trait::async_trait;

use crate::domain::models::ActivityLogEntry;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                          ActivityWindow                                        */
/* ============================================================================================== */

/// A lookback window for the activity log, mapped to the az `--offset` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityWindow {
    OneHour,
    Day,
    Week,
    Month,
}

impl ActivityWindow {
    /// The az `--offset` token for this window.
    pub fn offset(&self) -> &'static str {
        match self {
            ActivityWindow::OneHour => "1h",
            ActivityWindow::Day => "24h",
            ActivityWindow::Week => "7d",
            ActivityWindow::Month => "30d",
        }
    }

    /// A short human label, e.g. "last 24h".
    pub fn label(&self) -> &'static str {
        match self {
            ActivityWindow::OneHour => "last 1h",
            ActivityWindow::Day => "last 24h",
            ActivityWindow::Week => "last 7d",
            ActivityWindow::Month => "last 30d",
        }
    }

    /// The next wider window (saturates at Month).
    pub fn wider(&self) -> Self {
        match self {
            ActivityWindow::OneHour => ActivityWindow::Day,
            ActivityWindow::Day => ActivityWindow::Week,
            ActivityWindow::Week => ActivityWindow::Month,
            ActivityWindow::Month => ActivityWindow::Month,
        }
    }

    /// The next narrower window (saturates at OneHour).
    pub fn narrower(&self) -> Self {
        match self {
            ActivityWindow::Month => ActivityWindow::Week,
            ActivityWindow::Week => ActivityWindow::Day,
            ActivityWindow::Day => ActivityWindow::OneHour,
            ActivityWindow::OneHour => ActivityWindow::OneHour,
        }
    }
}

/* ============================================================================================== */
/*                                           ActivityScope                                        */
/* ============================================================================================== */

/// The scope an activity-log query is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityScope {
    Subscription {
        subscription_id: String,
    },
    ResourceGroup {
        subscription_id: String,
        resource_group: String,
    },
    Resource {
        subscription_id: String,
        resource_group: String,
        resource_id: String,
        resource_name: String,
    },
}

impl ActivityScope {
    pub fn subscription_id(&self) -> &str {
        match self {
            ActivityScope::Subscription { subscription_id }
            | ActivityScope::ResourceGroup { subscription_id, .. }
            | ActivityScope::Resource { subscription_id, .. } => subscription_id,
        }
    }

    /// Short descriptor of the scope for the view title.
    pub fn label(&self) -> String {
        match self {
            ActivityScope::Subscription { .. } => "subscription".to_string(),
            ActivityScope::ResourceGroup { resource_group, .. } => resource_group.clone(),
            ActivityScope::Resource { resource_name, .. } => resource_name.clone(),
        }
    }

    /// The next broader scope (Resource → ResourceGroup → Subscription), or
    /// `None` if already at subscription scope.
    pub fn widened(&self) -> Option<ActivityScope> {
        match self {
            ActivityScope::Resource { subscription_id, resource_group, .. } => {
                Some(ActivityScope::ResourceGroup {
                    subscription_id: subscription_id.clone(),
                    resource_group: resource_group.clone(),
                })
            }
            ActivityScope::ResourceGroup { subscription_id, .. } => {
                Some(ActivityScope::Subscription {
                    subscription_id: subscription_id.clone(),
                })
            }
            ActivityScope::Subscription { .. } => None,
        }
    }
}

/* ============================================================================================== */
/*                                       ActivityLogProvider                                       */
/* ============================================================================================== */

/// Provides read access to the Azure Activity Log.
#[async_trait]
pub trait ActivityLogProvider: Send + Sync {
    async fn list_activity(
        &self,
        scope: &ActivityScope,
        window: ActivityWindow,
    ) -> Result<Vec<ActivityLogEntry>, AppError>;
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_offset_tokens() {
        assert_eq!(ActivityWindow::OneHour.offset(), "1h");
        assert_eq!(ActivityWindow::Day.offset(), "24h");
        assert_eq!(ActivityWindow::Week.offset(), "7d");
        assert_eq!(ActivityWindow::Month.offset(), "30d");
    }

    #[test]
    fn window_widen_narrow_saturate() {
        assert_eq!(ActivityWindow::Month.wider(), ActivityWindow::Month);
        assert_eq!(ActivityWindow::OneHour.narrower(), ActivityWindow::OneHour);
        assert_eq!(ActivityWindow::Day.wider(), ActivityWindow::Week);
    }

    #[test]
    fn scope_widens_outward_then_stops() {
        let r = ActivityScope::Resource {
            subscription_id: "s".into(),
            resource_group: "rg".into(),
            resource_id: "id".into(),
            resource_name: "vm".into(),
        };
        let rg = r.widened().unwrap();
        assert!(matches!(rg, ActivityScope::ResourceGroup { .. }));
        let sub = rg.widened().unwrap();
        assert!(matches!(sub, ActivityScope::Subscription { .. }));
        assert!(sub.widened().is_none());
    }
}