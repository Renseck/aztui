//! Command palette: state and pure row building (application layer). The
//! renderer lives in `ui::widgets::palette`.

use crate::actions::{self, ActionId, Scope, Target};
use crate::app::{AppState, Modal, SLOT_GRAPH};
use crate::domain::models::AzureContext;
use crate::ui::fuzzy::fuzzy_score;
use crate::ui::widgets::resource_browser::abbreviate_resource_type;

/// Maximum context rows in `All` mode.
pub const MAX_CONTEXTS: usize = 20;
/// Maximum resource rows in `All` mode.
pub const MAX_RESOURCES: usize = 50;

/* ============================================================================================== */
/*                                              Types                                             */
/* ============================================================================================== */

/// What the palette is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteMode {
    /// Actions, contexts, and resources (`:`).
    All,
    /// Contexts only (`Ctrl+G`).
    ContextsOnly,
    /// The actions available on one chosen target (Tab on a row).
    TargetActions(Target),
}

/* ============================================================================================== */

/// One rendered palette line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteRow {
    Header(&'static str),
    Action(ActionId, Target),
    Context(AzureContext),
    /// Index into `AppState::global_resources`.
    Resource(usize),
    /// Non-selectable status text, e.g. "Loading inventory…".
    Info(String),
}

impl PaletteRow {
    /// True for rows the cursor can land on.
    pub fn is_selectable(&self) -> bool {
        matches!(self, PaletteRow::Action(..) | PaletteRow::Context(_) | PaletteRow::Resource(_))
    }
}

/* ============================================================================================== */

/// Palette modal state.
#[derive(Debug, Clone)]
pub struct PaletteState {
    pub mode: PaletteMode,
    pub query: String,
    /// `All`-mode query saved when drilling into `TargetActions`.
    pub return_query: String,
    pub rows: Vec<PaletteRow>,
    /// Index among selectable rows (headers and info rows are skipped).
    pub cursor: usize,
    /// The view's selection when the palette opened; the ACTIONS section acts on it.
    pub opened_target: Target,
}

impl PaletteState {
    /// Opens the palette in `mode` over the current state.
    pub fn new(state: &AppState, mode: PaletteMode) -> Self {
        let opened_target = actions::current_target(state);
        let rows = build_palette_rows(state, &mode, "", &opened_target);
        Self {
            mode,
            query: String::new(),
            return_query: String::new(),
            rows,
            cursor: 0,
            opened_target,
        }
    }

    /* ========================================================================================== */
    pub fn selectable_count(&self) -> usize {
        self.rows.iter().filter(|r| r.is_selectable()).count()
    }

    /* ========================================================================================== */
    /// The row under the cursor.
    pub fn selected(&self) -> Option<&PaletteRow> {
        self.rows.iter().filter(|r| r.is_selectable()).nth(self.cursor)
    }
}

/* ============================================================================================== */
/*                                          Row building                                          */
/* ============================================================================================== */

/// Builds the palette rows for `mode` and `query`. Pure: reads `state`, mutates nothing.
pub fn build_palette_rows(
    state: &AppState,
    mode: &PaletteMode,
    query: &str,
    opened_target: &Target,
) -> Vec<PaletteRow> {
    let mut rows = Vec::new();
    match mode {
        PaletteMode::All => {
            push_section(&mut rows, "ACTIONS", action_rows(state, opened_target, query, false));
            push_section(&mut rows, "CONTEXTS", context_rows(state, query));
            rows.extend(resource_section(state, query));
        }
        PaletteMode::ContextsOnly => {
            let recent_ids: Vec<&str> =
                state.recent_contexts.iter().map(|c| c.subscription.id.as_str()).collect();
            let (recent, rest): (Vec<AzureContext>, Vec<AzureContext>) = scored_contexts(state, query)
                .into_iter()
                .partition(|c| recent_ids.contains(&c.subscription.id.as_str()));
            push_section(&mut rows, "RECENT", recent.into_iter().map(PaletteRow::Context).collect());
            push_section(&mut rows, "ALL MATCHES", rest.into_iter().map(PaletteRow::Context).collect());
        }
        PaletteMode::TargetActions(target) => {
            push_section(&mut rows, "ACTIONS", action_rows(state, target, query, true));
        }
    }
    rows
}

/* ============================================================================================== */
/// Palette label for an action: target-scoped actions name their target.
pub fn action_label(id: ActionId, target: &Target) -> String {
    let spec = id.spec();
    match (spec.scope, target.kind()) {
        (Scope::Target(_), Some(_)) => format!("{}: {}", spec.label, target.name()),
        _ => spec.label.to_string(),
    }
}

/* ============================================================================================== */
/// Match strings for every inventory row: name, type label, resource group, and
/// subscription name. Built once per inventory load, not per keystroke.
pub fn resource_haystacks(state: &AppState) -> Vec<String> {
    state
        .global_resources
        .iter()
        .map(|r| {
            format!(
                "{} {} {} {}",
                r.name,
                abbreviate_resource_type(&r.resource_type),
                r.resource_group,
                subscription_name(state, &r.subscription_id)
            )
        })
        .collect()
}

/* ============================================================================================== */
/// Indices of the best `cap` haystack matches, best first (ties by index).
pub fn top_resource_matches(haystacks: &[String], query: &str, cap: usize) -> Vec<usize> {
    let mut scored: Vec<(i64, usize)> = haystacks
        .iter()
        .enumerate()
        .filter_map(|(i, h)| fuzzy_score(h, query).map(|s| (s, i)))
        .collect();
    if cap > 0 && scored.len() > cap {
        scored.select_nth_unstable_by(cap - 1, |a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.truncate(cap);
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/* ============================================================================================== */
/// Display name for a subscription ID, falling back to the ID itself.
pub fn subscription_name(state: &AppState, subscription_id: &str) -> String {
    state
        .subscriptions_by_tenant
        .values()
        .flatten()
        .find(|s| s.id == subscription_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| subscription_id.to_string())
}

/* ============================================================================================== */
/// Rebuilds the open palette's rows from current state and clamps its cursor.
/// No-op when the palette is not open.
pub fn refresh(state: &mut AppState) {
    let rows = match &state.modal {
        Some(Modal::Palette(p)) => build_palette_rows(state, &p.mode, &p.query, &p.opened_target),
        _ => return,
    };
    if let Some(Modal::Palette(p)) = state.modal.as_mut() {
        p.rows = rows;
        p.cursor = p.cursor.min(p.selectable_count().saturating_sub(1));
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn push_section(rows: &mut Vec<PaletteRow>, title: &'static str, items: Vec<PaletteRow>) {
    if !items.is_empty() {
        rows.push(PaletteRow::Header(title));
        rows.extend(items);
    }
}

/* ============================================================================================== */
/// Applicable actions for `target`, fuzzy-ranked. `target_only` restricts to
/// target-scoped actions and lists the default action first.
fn action_rows(state: &AppState, target: &Target, query: &str, target_only: bool) -> Vec<PaletteRow> {
    let mut ids: Vec<ActionId> = actions::ordered_visible(state, target)
        .into_iter()
        .filter(|id| *id != ActionId::OpenPalette)
        .filter(|id| !target_only || matches!(id.spec().scope, Scope::Target(_)))
        .filter(|id| id.applies(state, target))
        .collect();

    if target_only {
        if let Some(default) = actions::default_action(target) {
            if let Some(pos) = ids.iter().position(|id| *id == default) {
                let d = ids.remove(pos);
                ids.insert(0, d);
            }
        }
    }

    let mut scored: Vec<(i64, ActionId)> = ids
        .into_iter()
        .filter_map(|id| fuzzy_score(&action_label(id, target), query).map(|s| (s, id)))
        .collect();
    if !query.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    scored.into_iter().map(|(_, id)| PaletteRow::Action(id, target.clone())).collect()
}

/* ============================================================================================== */
/// Every context in tenant order, fuzzy-filtered on its label (sorted by score
/// when `query` is non-empty).
fn scored_contexts(state: &AppState, query: &str) -> Vec<AzureContext> {
    let mut scored: Vec<(i64, AzureContext)> = state
        .tenants
        .iter()
        .flat_map(|tenant| {
            state.subscriptions_by_tenant.get(&tenant.id).into_iter().flatten().map(move |sub| {
                AzureContext { tenant: tenant.clone(), subscription: sub.clone() }
            })
        })
        .filter_map(|ctx| fuzzy_score(&ctx.label(), query).map(|s| (s, ctx)))
        .collect();
    if !query.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    scored.into_iter().map(|(_, ctx)| ctx).collect()
}

/* ============================================================================================== */
/// `All`-mode contexts: recent first on an empty query, capped at [`MAX_CONTEXTS`].
fn context_rows(state: &AppState, query: &str) -> Vec<PaletteRow> {
    let mut ctxs = scored_contexts(state, query);
    if query.is_empty() {
        let recent_ids: Vec<&str> =
            state.recent_contexts.iter().map(|c| c.subscription.id.as_str()).collect();
        // Stable sort: recent contexts move to the front, others keep tenant order.
        ctxs.sort_by_key(|c| !recent_ids.contains(&c.subscription.id.as_str()));
    }
    ctxs.truncate(MAX_CONTEXTS);
    ctxs.into_iter().map(PaletteRow::Context).collect()
}

/* ============================================================================================== */
/// The RESOURCES section: matches, or an info row explaining why there are none.
fn resource_section(state: &AppState, query: &str) -> Vec<PaletteRow> {
    let info = |text: String| vec![PaletteRow::Header("RESOURCES"), PaletteRow::Info(text)];
    let total = state.global_resources.len();

    if total == 0 {
        if state.pending_operations.contains_key(&SLOT_GRAPH) {
            return info("Loading inventory…".to_string());
        }
        if state.active_context.is_some() {
            return info("Resources unavailable — press 5, then r to retry".to_string());
        }
        return Vec::new();
    }
    if query.is_empty() {
        return info(format!("Type to search {} resources", total));
    }

    let matches = top_resource_matches(&state.resource_haystacks, query, MAX_RESOURCES);
    let mut rows = Vec::new();
    push_section(&mut rows, "RESOURCES", matches.into_iter().map(PaletteRow::Resource).collect());
    rows
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::app::PendingOperation;
    use crate::domain::models::{Subscription, SubscriptionState};
    use crate::test_support::{ctx, global, state_with_contexts, STORAGE_TYPE, VM_TYPE};

    fn headers(rows: &[PaletteRow]) -> Vec<&'static str> {
        rows.iter()
            .filter_map(|r| match r {
                PaletteRow::Header(h) => Some(*h),
                _ => None,
            })
            .collect()
    }

    fn with_inventory(names: &[&str]) -> AppState {
        let mut s = state_with_contexts(Some("sub-a"));
        s.global_resources = names.iter().map(|n| global(n, VM_TYPE, "sub-a")).collect();
        s.resource_haystacks = resource_haystacks(&s);
        s
    }

    fn rows(s: &AppState, mode: PaletteMode, query: &str) -> Vec<PaletteRow> {
        build_palette_rows(s, &mode, query, &actions::current_target(s))
    }

    #[test]
    fn all_mode_sections_in_order() {
        let s = with_inventory(&["web-01"]);
        assert_eq!(headers(&rows(&s, PaletteMode::All, "")), vec!["ACTIONS", "CONTEXTS", "RESOURCES"]);
    }

    #[test]
    fn empty_query_shows_resource_count_instead_of_rows() {
        let s = with_inventory(&["web-01", "web-02"]);
        let r = rows(&s, PaletteMode::All, "");
        assert!(r.contains(&PaletteRow::Info("Type to search 2 resources".into())));
        assert!(!r.iter().any(|row| matches!(row, PaletteRow::Resource(_))));
    }

    #[test]
    fn loading_and_unavailable_info_rows() {
        let mut s = state_with_contexts(Some("sub-a"));
        assert!(rows(&s, PaletteMode::All, "").iter().any(|r| matches!(r, PaletteRow::Info(t) if t.starts_with("Resources unavailable"))));
        s.pending_operations.insert(
            SLOT_GRAPH,
            PendingOperation { id: SLOT_GRAPH, description: String::new(), started_at: Instant::now(), abort_handle: None },
        );
        assert!(rows(&s, PaletteMode::All, "").contains(&PaletteRow::Info("Loading inventory…".into())));
    }

    #[test]
    fn non_matching_sections_are_omitted() {
        let s = with_inventory(&["web-01"]);
        let r = rows(&s, PaletteMode::All, "web-01");
        assert_eq!(headers(&r), vec!["RESOURCES"]);
        assert!(r.contains(&PaletteRow::Resource(0)));
    }

    #[test]
    fn contexts_and_resources_are_capped() {
        let mut s = with_inventory(&[]);
        let subs: Vec<Subscription> = (0..30)
            .map(|i| Subscription {
                id: format!("sub-{i}"),
                name: format!("sub-{i}-name"),
                tenant_id: "t1".into(),
                state: SubscriptionState::Enabled,
            })
            .collect();
        s.subscriptions_by_tenant.insert("t1".into(), subs);
        s.global_resources = (0..60).map(|i| global(&format!("vm-{i}"), VM_TYPE, "sub-a")).collect();
        s.resource_haystacks = resource_haystacks(&s);

        let contexts = rows(&s, PaletteMode::All, "").iter().filter(|r| matches!(r, PaletteRow::Context(_))).count();
        assert_eq!(contexts, MAX_CONTEXTS);
        let resources = rows(&s, PaletteMode::All, "vm").iter().filter(|r| matches!(r, PaletteRow::Resource(_))).count();
        assert_eq!(resources, MAX_RESOURCES);
    }

    #[test]
    fn recent_contexts_come_first_on_empty_query() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.recent_contexts = vec![ctx("sub-b", "t1")];
        let first = rows(&s, PaletteMode::All, "").into_iter().find_map(|r| match r {
            PaletteRow::Context(c) => Some(c.subscription.id),
            _ => None,
        });
        assert_eq!(first.as_deref(), Some("sub-b"));
    }

    #[test]
    fn contexts_only_mode_splits_recent_and_all_matches() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.recent_contexts = vec![ctx("sub-b", "t1")];
        let r = rows(&s, PaletteMode::ContextsOnly, "");
        assert_eq!(headers(&r), vec!["RECENT", "ALL MATCHES"]);
        assert!(!r.iter().any(|row| matches!(row, PaletteRow::Action(..) | PaletteRow::Resource(_))));
    }

    #[test]
    fn target_actions_list_default_first() {
        let s = state_with_contexts(Some("sub-a"));
        let vm = Target::from_global(&global("web-01", VM_TYPE, "sub-a"));
        let r = rows(&s, PaletteMode::TargetActions(vm.clone()), "");
        assert_eq!(r.get(1), Some(&PaletteRow::Action(ActionId::RunCommand, vm)));
        assert!(r.iter().all(|row| match row {
            PaletteRow::Action(id, _) => matches!(id.spec().scope, Scope::Target(_)),
            PaletteRow::Header(_) => true,
            _ => false,
        }));
    }

    #[test]
    fn storage_target_defaults_to_go_to_resource_group() {
        let s = state_with_contexts(Some("sub-a"));
        let st = Target::from_global(&global("st01", STORAGE_TYPE, "sub-a"));
        let r = rows(&s, PaletteMode::TargetActions(st.clone()), "");
        assert_eq!(r.get(1), Some(&PaletteRow::Action(ActionId::GoToResourceGroup, st)));
    }

    #[test]
    fn selected_skips_headers() {
        let s = state_with_contexts(Some("sub-a"));
        let p = PaletteState::new(&s, PaletteMode::ContextsOnly);
        assert!(matches!(p.selected(), Some(PaletteRow::Context(_))));
        assert_eq!(p.selectable_count(), 2);
    }

    #[test]
    fn top_matches_rank_and_cap() {
        let hay: Vec<String> = vec!["alpha".into(), "web-prod".into(), "web".into(), "zzz".into()];
        let top = top_resource_matches(&hay, "web", 1);
        assert_eq!(top.len(), 1);
        assert!(top[0] == 1 || top[0] == 2);
        assert_eq!(top_resource_matches(&hay, "web", 10).len(), 2);
    }

    #[test]
    fn action_labels_name_their_target() {
        let vm = Target::from_global(&global("web-01", VM_TYPE, "sub-a"));
        assert_eq!(action_label(ActionId::ActivityForTarget, &vm), "Activity log: web-01");
        assert_eq!(action_label(ActionId::Quit, &vm), "Quit");
    }

    #[test]
    fn palette_does_not_list_itself() {
        let s = state_with_contexts(Some("sub-a"));
        assert!(!rows(&s, PaletteMode::All, "").contains(&PaletteRow::Action(
            ActionId::OpenPalette,
            actions::current_target(&s)
        )));
    }

}
