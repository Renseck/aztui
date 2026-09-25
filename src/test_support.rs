//! Shared unit-test fixtures. Compiled only under `cfg(test)`.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::{mpsc, RwLock};

use crate::app::{dispatch_command, AppState};
use crate::az::AzCliExecutor;
use crate::cache::CacheStore;
use crate::command::Command;
use crate::config::{AppConfig, CacheConfig};
use crate::domain::models::{
    AzureContext, GlobalResource, Resource, ResourceGroup, Subscription, SubscriptionState, Tenant,
};
use crate::errors::{AppError, ErrorKind};
use crate::event::Event;
use crate::providers::{
    AzActivityLogProvider, AzAuthProvider, AzCostProvider, AzGraphProvider, AzResourceProvider,
    AzVmProvider,
};
use crate::security::SecurityManager;

pub const VM_TYPE: &str = "Microsoft.Compute/virtualMachines";
pub const STORAGE_TYPE: &str = "Microsoft.Storage/storageAccounts";

/* ============================================================================================== */
/*                                          State fixtures                                        */
/* ============================================================================================== */

/// A context for subscription `sub` (named `<sub>-name`) in tenant `tenant`.
pub fn ctx(sub: &str, tenant: &str) -> AzureContext {
    AzureContext {
        tenant: Tenant {
            id: tenant.to_string(),
            tenant_display_name: tenant.to_uppercase(),
            tenant_default_domain: String::new(),
        },
        subscription: Subscription {
            id: sub.to_string(),
            name: format!("{sub}-name"),
            tenant_id: tenant.to_string(),
            state: SubscriptionState::Enabled,
        },
    }
}

/* ============================================================================================== */
/// State with tenant `t1` holding subscriptions `sub-a` and `sub-b`. `active`
/// selects which one (if any) is the active context.
pub fn state_with_contexts(active: Option<&str>) -> AppState {
    let mut s = AppState::new(AppConfig::default(), SecurityManager::disabled());
    let a = ctx("sub-a", "t1");
    let b = ctx("sub-b", "t1");
    s.tenants = vec![a.tenant.clone()];
    s.subscriptions_by_tenant
        .insert("t1".to_string(), vec![a.subscription.clone(), b.subscription.clone()]);
    s.active_context = match active {
        Some("sub-a") => Some(a),
        Some("sub-b") => Some(b),
        _ => None,
    };
    s
}

/* ============================================================================================== */
pub fn resource_group(name: &str) -> ResourceGroup {
    ResourceGroup {
        name: name.to_string(),
        subscription_id: "sub-a".to_string(),
        location: "westeurope".to_string(),
        tags: Default::default(),
    }
}

/* ============================================================================================== */
pub fn resource(name: &str, rg: &str, resource_type: &str) -> Resource {
    Resource {
        id: format!("/subscriptions/sub-a/resourceGroups/{rg}/providers/{resource_type}/{name}"),
        name: name.to_string(),
        resource_type: resource_type.to_string(),
        resource_group: rg.to_string(),
        location: "westeurope".to_string(),
        tags: Default::default(),
    }
}

/* ============================================================================================== */
/// A Resource Graph row in resource group `rg-app`.
pub fn global(name: &str, resource_type: &str, sub: &str) -> GlobalResource {
    GlobalResource {
        id: format!("/subscriptions/{sub}/resourceGroups/rg-app/providers/{resource_type}/{name}"),
        name: name.to_string(),
        resource_type: resource_type.to_lowercase(),
        resource_group: "rg-app".to_string(),
        subscription_id: sub.to_string(),
        location: "westeurope".to_string(),
    }
}

/* ============================================================================================== */
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/* ============================================================================================== */
pub fn key_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

/* ============================================================================================== */
/*                                         Dispatch harness                                       */
/* ============================================================================================== */

/// Executor that fails every call. Dispatch tests only assert on state and
/// queued commands, never on provider output.
struct FailingExecutor;

#[async_trait]
impl AzCliExecutor for FailingExecutor {
    async fn execute(&self, _args: &[&str], _dur: Duration) -> Result<String, AppError> {
        Err(AppError::new(ErrorKind::CliExecutionFailed, "test executor"))
    }
}

/* ============================================================================================== */
/// Runs [`dispatch_command`] with providers backed by [`FailingExecutor`].
/// Commands the dispatcher queues with `try_send` land in the returned receiver.
pub async fn dispatch(state: &mut AppState, cmd: Command) -> (Vec<Event>, mpsc::Receiver<Command>) {
    let (tx, rx) = mpsc::channel(64);
    let exec: Arc<dyn AzCliExecutor> = Arc::new(FailingExecutor);
    let cache = Arc::new(RwLock::new(CacheStore::new()));
    let cfg = CacheConfig::default();
    let events = dispatch_command(
        state,
        cmd,
        &tx,
        Arc::new(AzAuthProvider::new(exec.clone(), cache.clone(), cfg.clone())),
        Arc::new(AzResourceProvider::new(exec.clone(), cache.clone(), cfg.clone())),
        Arc::new(AzCostProvider::new(exec.clone(), cache.clone(), cfg.clone())),
        Arc::new(AzVmProvider::new(exec.clone(), Duration::from_secs(1))),
        Arc::new(AzActivityLogProvider::new(exec.clone(), Duration::from_secs(1))),
        Arc::new(AzGraphProvider::new(exec, cache, cfg)),
    )
    .await;
    (events, rx)
}

/* ============================================================================================== */
/// Collects every command currently queued in `rx`.
pub fn drain(rx: &mut mpsc::Receiver<Command>) -> Vec<Command> {
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}
