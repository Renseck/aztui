//! Action registry: the single table of named actions. Keybindings, hint bars,
//! the help screen, and the command palette all read from here.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, CostGrouping, CostView, Modal, Pane, View};
use crate::command::Command;
use crate::domain::activity::ActivityScope;
use crate::domain::models::{AzureContext, GlobalResource};
use crate::ui::widgets::{context_switcher, cost_explorer, global_search, quick_switch, resource_browser};

/* ============================================================================================== */
/*                                             Targets                                            */
/* ============================================================================================== */

/// The kind of thing an action can operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Context,
    ResourceGroup,
    Resource,
}

/* ============================================================================================== */

/// The concrete thing an action operates on: the current selection in a view,
/// or the row chosen in the command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    None,
    Context(AzureContext),
    ResourceGroup {
        subscription_id: String,
        name: String,
    },
    Resource {
        subscription_id: String,
        resource_group: String,
        id: String,
        name: String,
        resource_type: String,
    },
}

impl Target {
    /// Builds a resource target from a Resource Graph row.
    pub fn from_global(r: &GlobalResource) -> Self {
        Target::Resource {
            subscription_id: r.subscription_id.clone(),
            resource_group: r.resource_group.clone(),
            id: r.id.clone(),
            name: r.name.clone(),
            resource_type: r.resource_type.clone(),
        }
    }

    /* ========================================================================================== */
    /// The target's kind, or `None` for [`Target::None`].
    pub fn kind(&self) -> Option<TargetKind> {
        match self {
            Target::None => None,
            Target::Context(_) => Some(TargetKind::Context),
            Target::ResourceGroup { .. } => Some(TargetKind::ResourceGroup),
            Target::Resource { .. } => Some(TargetKind::Resource),
        }
    }

    /* ========================================================================================== */
    /// Short display name: a resource or group name, or a context label.
    pub fn name(&self) -> String {
        match self {
            Target::None => String::new(),
            Target::Context(ctx) => ctx.label(),
            Target::ResourceGroup { name, .. } => name.clone(),
            Target::Resource { name, .. } => name.clone(),
        }
    }
}

/* ============================================================================================== */
/*                                       Scopes and bindings                                      */
/* ============================================================================================== */

/// Where an action is visible and its key is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Everywhere.
    Global,
    /// Only in these views.
    View(&'static [View]),
    /// Only when the current target is one of these kinds.
    Target(&'static [TargetKind]),
}

/* ============================================================================================== */

/// A key that triggers an action, plus how it is shown to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub mods: KeyModifiers,
    pub display: &'static str,
}

impl KeyBinding {
    /// A plain character key.
    pub const fn plain(c: char, display: &'static str) -> Self {
        Self { code: KeyCode::Char(c), mods: KeyModifiers::NONE, display }
    }

    /* ========================================================================================== */
    /// A `Ctrl+<letter>` chord.
    pub const fn ctrl(c: char, display: &'static str) -> Self {
        Self { code: KeyCode::Char(c), mods: KeyModifiers::CONTROL, display }
    }

    /* ========================================================================================== */
    /// Returns true if `key` triggers this binding.
    ///
    /// - `Ctrl` chords compare letters case-insensitively, because terminals
    ///   disagree on whether `Ctrl+G` arrives as `g` or `G`.
    /// - Plain characters ignore `SHIFT`, because Windows reports it
    ///   inconsistently for symbols like `?` and `:`. Letters still compare
    ///   exactly, so `q` does not match `Q`.
    /// - Non-character keys (e.g. `F5`) compare the key code only.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match (self.code, key.code) {
            (KeyCode::Char(want), KeyCode::Char(got)) => {
                if self.mods.contains(KeyModifiers::CONTROL) {
                    ctrl && want.eq_ignore_ascii_case(&got)
                } else {
                    !ctrl && !alt && want == got
                }
            }
            (want, got) => want == got,
        }
    }
}

/* ============================================================================================== */

/// Static description of an action.
#[derive(Debug, Clone, Copy)]
pub struct ActionSpec {
    /// Palette and help text, e.g. "Activity log".
    pub label: &'static str,
    /// Primary binding, shown in hints, help, and the palette.
    pub key: Option<KeyBinding>,
    /// Secondary bindings, e.g. `h` for `[`.
    pub alt_keys: &'static [KeyBinding],
    pub scope: Scope,
    /// Short hint-bar label, e.g. "activity". `None` keeps the action out of hint bars.
    pub hint: Option<&'static str>,
    /// Overrides the key shown in the hint bar, e.g. "[/]" for a prev/next pair.
    pub hint_key: Option<&'static str>,
}

/* ============================================================================================== */
/*                                             Actions                                            */
/* ============================================================================================== */

const COST_AND_ACTIVITY: &[View] = &[View::CostExplorer, View::ActivityLog];
const COST_ONLY: &[View] = &[View::CostExplorer];
const ACTIVITY_ONLY: &[View] = &[View::ActivityLog];
const RUN_COMMAND_ONLY: &[View] = &[View::RunCommand];
const RG_OR_RESOURCE: &[TargetKind] = &[TargetKind::ResourceGroup, TargetKind::Resource];
const RESOURCE_ONLY: &[TargetKind] = &[TargetKind::Resource];
const CONTEXT_ONLY: &[TargetKind] = &[TargetKind::Context];
const F5: KeyBinding = KeyBinding { code: KeyCode::F(5), mods: KeyModifiers::NONE, display: "F5" };
// Const items so the slices are `'static`; an inline `&[KeyBinding::plain(..)]`
// is a temporary because const-fn calls are not promoted.
const PREV_ALT_KEYS: &[KeyBinding] = &[KeyBinding::plain('h', "h")];
const NEXT_ALT_KEYS: &[KeyBinding] = &[KeyBinding::plain('l', "l")];

/// Every named action in aztui. Navigation mechanics (j/k, Tab, Enter, Esc,
/// text entry) are deliberately not actions; they stay in `ui::input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    GoContexts,
    GoResources,
    GoCost,
    GoActivity,
    GoGlobalSearch,
    QuickSwitch,
    Help,
    Quit,
    Refresh,
    FocusSearch,
    PrevPeriod,
    NextPeriod,
    ToggleCostGrouping,
    BroadenActivityScope,
    ToggleFailedOnly,
    RunScript,
    ActivityForTarget,
    CostForResourceGroup,
    RunCommand,
    GoToResourceGroup,
    SwitchToContext,
}

/* ============================================================================================== */

/// Builds an [`ActionSpec`] with no alternate keys and no hint-key override.
const fn action(
    label: &'static str,
    key: Option<KeyBinding>,
    scope: Scope,
    hint: Option<&'static str>,
) -> ActionSpec {
    ActionSpec { label, key, alt_keys: &[], scope, hint, hint_key: None }
}

/* ============================================================================================== */

impl ActionId {
    /// Every action, in display order (within each scope class).
    pub const ALL: &'static [ActionId] = &[
        ActionId::GoContexts,
        ActionId::GoResources,
        ActionId::GoCost,
        ActionId::GoActivity,
        ActionId::GoGlobalSearch,
        ActionId::QuickSwitch,
        ActionId::Help,
        ActionId::Quit,
        ActionId::Refresh,
        ActionId::FocusSearch,
        ActionId::PrevPeriod,
        ActionId::NextPeriod,
        ActionId::ToggleCostGrouping,
        ActionId::BroadenActivityScope,
        ActionId::ToggleFailedOnly,
        ActionId::RunScript,
        ActionId::ActivityForTarget,
        ActionId::CostForResourceGroup,
        ActionId::RunCommand,
        ActionId::GoToResourceGroup,
        ActionId::SwitchToContext,
    ];

    /* ========================================================================================== */
    /// Static description: label, key, scope, and hint.
    pub fn spec(self) -> ActionSpec {
        use ActionId::*;
        match self {
            GoContexts => action("Go to context switcher", Some(KeyBinding::plain('1', "1")), Scope::Global, None),
            GoResources => action("Go to resource browser", Some(KeyBinding::plain('2', "2")), Scope::Global, None),
            GoCost => action("Go to cost explorer", Some(KeyBinding::plain('3', "3")), Scope::Global, None),
            GoActivity => action("Go to activity log", Some(KeyBinding::plain('4', "4")), Scope::Global, None),
            GoGlobalSearch => action("Go to global search", Some(KeyBinding::plain('5', "5")), Scope::Global, None),
            QuickSwitch => action("Switch context", Some(KeyBinding::ctrl('g', "Ctrl+G")), Scope::Global, None),
            Help => action("Help", Some(KeyBinding::plain('?', "?")), Scope::Global, None),
            Quit => action("Quit", Some(KeyBinding::plain('q', "q")), Scope::Global, None),
            Refresh => action("Refresh view", Some(KeyBinding::plain('r', "r")), Scope::Global, Some("refresh")),
            FocusSearch => action("Search", Some(KeyBinding::plain('/', "/")), Scope::Global, Some("search")),
            PrevPeriod => ActionSpec {
                alt_keys: PREV_ALT_KEYS,
                hint_key: Some("[/]"),
                ..action("Previous period", Some(KeyBinding::plain('[', "[")), Scope::View(COST_AND_ACTIVITY), Some("period"))
            },
            NextPeriod => ActionSpec {
                alt_keys: NEXT_ALT_KEYS,
                ..action("Next period", Some(KeyBinding::plain(']', "]")), Scope::View(COST_AND_ACTIVITY), None)
            },
            ToggleCostGrouping => action("Toggle cost grouping", Some(KeyBinding::plain('g', "g")), Scope::View(COST_ONLY), Some("grouping")),
            BroadenActivityScope => action("Broaden activity scope", Some(KeyBinding::plain('s', "s")), Scope::View(ACTIVITY_ONLY), Some("scope")),
            ToggleFailedOnly => action("Toggle failed-only", Some(KeyBinding::plain('f', "f")), Scope::View(ACTIVITY_ONLY), Some("failed-only")),
            RunScript => action("Run script", Some(F5), Scope::View(RUN_COMMAND_ONLY), Some("run")),
            ActivityForTarget => action("Activity log", Some(KeyBinding::plain('a', "a")), Scope::Target(RG_OR_RESOURCE), Some("activity")),
            CostForResourceGroup => action("Cost for resource group", Some(KeyBinding::plain('c', "c")), Scope::Target(RG_OR_RESOURCE), Some("costs")),
            RunCommand => action("Run command", None, Scope::Target(RESOURCE_ONLY), Some("run command")),
            GoToResourceGroup => action("Go to resource group", None, Scope::Target(RESOURCE_ONLY), Some("go to RG")),
            SwitchToContext => action("Switch to context", None, Scope::Target(CONTEXT_ONLY), Some("switch")),
        }
    }

    /* ========================================================================================== */
    /// The command this action produces for target `t` in state `s`, or `None`
    /// when it does not apply (the action is then hidden and its key is a no-op).
    pub fn build(self, s: &AppState, t: &Target) -> Option<Command> {
        use ActionId::*;
        match self {
            GoContexts => go(s, View::ContextSwitcher),
            GoResources => go(s, View::ResourceBrowser),
            GoCost => go(s, View::CostExplorer),
            GoActivity => go(s, View::ActivityLog),
            GoGlobalSearch => go(s, View::GlobalSearch),
            QuickSwitch => Some(Command::OpenModal(Box::new(Modal::QuickSwitch {
                query: String::new(),
                filtered: quick_switch::build_filtered(s, ""),
                cursor: 0,
            }))),
            Help => Some(if s.active_view == View::Help {
                Command::NavigateTo(s.previous_view.clone())
            } else {
                Command::NavigateTo(View::Help)
            }),
            Quit => Some(Command::Quit),
            Refresh => match s.active_view {
                View::ContextSwitcher => Some(Command::RefreshContextList),
                View::ResourceBrowser => Some(Command::ListResourceGroups),
                View::CostExplorer => Some(Command::FetchCostSummary {
                    period: s.cost_period.clone(),
                    view: s.cost_view.clone(),
                }),
                View::ActivityLog => s.activity.as_ref().map(|_| Command::FetchActivityLog),
                View::GlobalSearch => Some(Command::FetchGlobalInventory),
                View::RunCommand | View::Help => None,
            },
            FocusSearch => match s.active_view {
                View::ContextSwitcher | View::ResourceBrowser => Some(Command::UpdateSearch(String::new())),
                View::ActivityLog => s.activity.as_ref().map(|_| Command::SetActivitySearchFocus(true)),
                View::GlobalSearch => Some(Command::SetGlobalSearchFocus(true)),
                View::CostExplorer | View::RunCommand | View::Help => None,
            },
            PrevPeriod => match s.active_view {
                View::CostExplorer => Some(Command::FetchCostSummary {
                    period: s.cost_period.previous_month(),
                    view: s.cost_view.clone(),
                }),
                View::ActivityLog => s.activity.as_ref().map(|_| Command::CycleActivityWindow(-1)),
                _ => None,
            },
            NextPeriod => match s.active_view {
                View::CostExplorer => s.cost_period.next_month().map(|period| Command::FetchCostSummary {
                    period,
                    view: s.cost_view.clone(),
                }),
                View::ActivityLog => s.activity.as_ref().map(|_| Command::CycleActivityWindow(1)),
                _ => None,
            },
            ToggleCostGrouping => match s.cost_view {
                CostView::Subscription(_) => Some(Command::ToggleCostGrouping),
                CostView::ResourceGroup(_) => None,
            },
            BroadenActivityScope => s
                .activity
                .as_ref()
                .and_then(|a| a.scope.widened())
                .map(|_| Command::CycleActivityScope),
            ToggleFailedOnly => s.activity.as_ref().map(|_| Command::ToggleActivityFailedOnly),
            RunScript => {
                let session = s.run_command.as_ref()?;
                if session.script().trim().is_empty() {
                    return None;
                }
                Some(Command::OpenModal(Box::new(Modal::Confirm {
                    message: format!(
                        "Run this PowerShell script on {} (rg: {})?",
                        session.vm_name, session.resource_group
                    ),
                    on_confirm: Box::new(Command::RunVmCommand),
                })))
            }
            ActivityForTarget => match t {
                Target::ResourceGroup { subscription_id, name } => Some(Command::OpenResourceActivity {
                    scope: ActivityScope::ResourceGroup {
                        subscription_id: subscription_id.clone(),
                        resource_group: name.clone(),
                    },
                }),
                Target::Resource { subscription_id, resource_group, id, name, .. } => {
                    Some(Command::OpenResourceActivity {
                        scope: ActivityScope::Resource {
                            subscription_id: subscription_id.clone(),
                            resource_group: resource_group.clone(),
                            resource_id: id.clone(),
                            resource_name: name.clone(),
                        },
                    })
                }
                _ => None,
            },
            CostForResourceGroup => {
                let (sub, rg) = match t {
                    Target::ResourceGroup { subscription_id, name } => (subscription_id, name),
                    Target::Resource { subscription_id, resource_group, .. } => (subscription_id, resource_group),
                    _ => return None,
                };
                Some(Command::InContext {
                    subscription_id: sub.clone(),
                    then: Box::new(Command::OpenResourceGroupCost { resource_group: rg.clone() }),
                })
            }
            RunCommand => match t {
                Target::Resource { subscription_id, resource_group, name, resource_type, .. }
                    if resource_browser::is_vm(resource_type) && s.active_view != View::RunCommand =>
                {
                    Some(Command::OpenRunCommand {
                        subscription_id: subscription_id.clone(),
                        resource_group: resource_group.clone(),
                        vm_name: name.clone(),
                    })
                }
                _ => None,
            },
            GoToResourceGroup => match t {
                // Already in the browser: the RG is on screen, so Enter stays a no-op.
                Target::Resource { subscription_id, resource_group, .. }
                    if s.active_view != View::ResourceBrowser =>
                {
                    Some(Command::InContext {
                        subscription_id: subscription_id.clone(),
                        then: Box::new(Command::OpenResourceGroup(resource_group.clone())),
                    })
                }
                _ => None,
            },
            SwitchToContext => match t {
                Target::Context(ctx) => Some(Command::SwitchContext(ctx.clone())),
                _ => None,
            },
        }
    }

    /* ========================================================================================== */
    /// True if [`ActionId::build`] yields a command for `t`.
    pub fn applies(self, s: &AppState, t: &Target) -> bool {
        self.build(s, t).is_some()
    }
}

/* ============================================================================================== */
/*                                      Targets and defaults                                      */
/* ============================================================================================== */

/// The target under the cursor in the active view.
pub fn current_target(s: &AppState) -> Target {
    let active_sub = s.active_context.as_ref().map(|c| c.subscription.id.clone());
    let target = match s.active_view {
        View::ContextSwitcher => context_switcher::selected_context(s).map(Target::Context),
        View::ResourceBrowser => active_sub.and_then(|sub| match s.resource_browser_focus {
            Pane::Left => resource_browser::selected_resource_group_name(s)
                .map(|name| Target::ResourceGroup { subscription_id: sub, name }),
            Pane::Right => {
                let filtered = resource_browser::filtered_resources(s);
                let cursor = s.resource_cursor.min(filtered.len().saturating_sub(1));
                filtered.get(cursor).map(|r| Target::Resource {
                    subscription_id: sub,
                    resource_group: r.resource_group.clone(),
                    id: r.id.clone(),
                    name: r.name.clone(),
                    resource_type: r.resource_type.clone(),
                })
            }
        }),
        View::CostExplorer => match s.cost_view {
            CostView::Subscription(CostGrouping::ByResourceGroup) => active_sub.and_then(|sub| {
                cost_explorer::selected_row_label(s)
                    .map(|name| Target::ResourceGroup { subscription_id: sub, name })
            }),
            _ => None,
        },
        View::GlobalSearch => global_search::selected_global_resource(s).map(Target::from_global),
        View::RunCommand => s.run_command.as_ref().map(|r| Target::Resource {
            subscription_id: r.subscription_id.clone(),
            resource_group: r.resource_group.clone(),
            id: format!(
                "/subscriptions/{}/resourceGroups/{}/providers/{}/{}",
                r.subscription_id,
                r.resource_group,
                resource_browser::VM_RESOURCE_TYPE,
                r.vm_name
            ),
            name: r.vm_name.clone(),
            resource_type: resource_browser::VM_RESOURCE_TYPE.to_string(),
        }),
        View::ActivityLog | View::Help => None,
    };
    target.unwrap_or(Target::None)
}

/* ============================================================================================== */
/// The action Enter runs on `t`: VMs open run-command, other resources go to
/// their resource group, contexts switch.
pub fn default_action(t: &Target) -> Option<ActionId> {
    match t {
        Target::Resource { resource_type, .. } if resource_browser::is_vm(resource_type) => {
            Some(ActionId::RunCommand)
        }
        Target::Resource { .. } => Some(ActionId::GoToResourceGroup),
        Target::Context(_) => Some(ActionId::SwitchToContext),
        Target::ResourceGroup { .. } | Target::None => None,
    }
}

/* ============================================================================================== */
/*                                     Visibility and resolution                                  */
/* ============================================================================================== */

/// True if `id`'s scope makes it visible in the active view for target `t`.
/// Scope only: says nothing about whether `build` succeeds.
pub fn is_visible(id: ActionId, s: &AppState, t: &Target) -> bool {
    match id.spec().scope {
        Scope::Global => true,
        Scope::View(views) => views.contains(&s.active_view),
        Scope::Target(kinds) => t.kind().map_or(false, |k| kinds.contains(&k)),
    }
}

/* ============================================================================================== */
/// Scope-visible actions for `t` in precedence order: View-scoped, then
/// Target-scoped, then Global, each in [`ActionId::ALL`] order.
pub fn ordered_visible(s: &AppState, t: &Target) -> Vec<ActionId> {
    let mut ids: Vec<ActionId> = ActionId::ALL
        .iter()
        .copied()
        .filter(|id| is_visible(*id, s, t))
        .collect();
    // Stable sort: keeps ALL order within each scope class.
    ids.sort_by_key(|id| scope_rank(id.spec().scope));
    ids
}

/* ============================================================================================== */
/// Maps a key to the first visible, applicable action bound to it.
pub fn resolve_key(key: KeyEvent, s: &AppState) -> Option<Command> {
    let t = current_target(s);
    ordered_visible(s, &t)
        .into_iter()
        .filter(|id| {
            let spec = id.spec();
            spec.key.iter().chain(spec.alt_keys.iter()).any(|kb| kb.matches(&key))
        })
        .find_map(|id| id.build(s, &t))
}

/* ============================================================================================== */
/// True if Enter in the active view runs the current target's default action.
pub fn uses_default_enter(s: &AppState) -> bool {
    match s.active_view {
        View::ContextSwitcher | View::GlobalSearch => true,
        View::ResourceBrowser => s.resource_browser_focus == Pane::Right,
        _ => false,
    }
}

/* ============================================================================================== */
/// The command for the current target's default action, if any.
pub fn default_command(s: &AppState) -> Option<Command> {
    let t = current_target(s);
    default_action(&t)?.build(s, &t)
}

/* ============================================================================================== */
/*                                              Hints                                             */
/* ============================================================================================== */

/// Hint-bar entries `(key, label)` for the active view. The order is: the
/// default Enter action, then the applicable registry actions that have a hint,
/// then the view's mechanic hints.
pub fn hints_for(s: &AppState) -> Vec<(String, String)> {
    let t = current_target(s);
    let mut out: Vec<(String, String)> = Vec::new();

    if uses_default_enter(s) {
        if let Some(id) = default_action(&t) {
            if let (true, Some(hint)) = (id.applies(s, &t), id.spec().hint) {
                out.push(("↵".to_string(), hint.to_string()));
            }
        }
    }

    for id in ordered_visible(s, &t) {
        let spec = id.spec();
        let (Some(hint), Some(key)) = (spec.hint, spec.key) else {
            continue;
        };
        if id.applies(s, &t) {
            out.push((spec.hint_key.unwrap_or(key.display).to_string(), hint.to_string()));
        }
    }

    out.extend(mechanic_hints(s).iter().map(|(k, l)| (k.to_string(), l.to_string())));
    out
}

/* ============================================================================================== */
/// Hints for hand-coded view mechanics (Enter/Tab/Esc/Backspace behaviour that
/// is not a registry action).
pub fn mechanic_hints(s: &AppState) -> &'static [(&'static str, &'static str)] {
    match s.active_view {
        View::ContextSwitcher => &[],
        View::ResourceBrowser => &[("Tab", "panes"), ("Esc", "back")],
        View::CostExplorer => match s.cost_view {
            CostView::Subscription(CostGrouping::ByService) => &[("Esc", "back")],
            CostView::Subscription(CostGrouping::ByResourceGroup) => &[("↵", "drill"), ("Esc", "back")],
            CostView::ResourceGroup(_) => &[("Bksp", "up"), ("Esc", "up")],
        },
        View::ActivityLog => &[("↵", "detail"), ("Esc", "back")],
        View::RunCommand => &[("Tab", "switch pane"), ("Esc", "back")],
        View::GlobalSearch | View::Help => &[("Esc", "back")],
    }
}

/* ============================================================================================== */
/*                                              Help                                              */
/* ============================================================================================== */

/// One titled block of the help screen.
#[derive(Debug, Clone)]
pub struct HelpSection {
    pub title: &'static str,
    pub entries: Vec<(String, &'static str)>,
}

/// Mechanics documented in the help screen's Navigation section.
const NAVIGATION_HELP: &[(&str, &str)] = &[
    ("↑/↓  j/k", "Navigate list"),
    ("Tab  ← →", "Switch pane"),
    ("Enter", "Select / open / default action"),
    ("Esc", "Clear search / close / back"),
    ("Backspace", "Up one level (cost explorer)"),
];

/* ============================================================================================== */
/// The help screen, generated from the registry: Global, one section per view
/// with view-scoped actions, target actions, then navigation mechanics.
pub fn help_sections() -> Vec<HelpSection> {
    let entry = |id: &ActionId| {
        let spec = id.spec();
        (key_label(&spec), spec.label)
    };
    let by_scope = |pred: &dyn Fn(Scope) -> bool| -> Vec<(String, &'static str)> {
        ActionId::ALL.iter().filter(|id| pred(id.spec().scope)).map(|id| entry(id)).collect()
    };

    let mut sections = vec![HelpSection {
        title: "Global",
        entries: by_scope(&|s| s == Scope::Global),
    }];
    for (view, title) in [
        (View::CostExplorer, "Cost explorer"),
        (View::ActivityLog, "Activity log"),
        (View::RunCommand, "Run command"),
    ] {
        sections.push(HelpSection {
            title,
            entries: by_scope(&|s| matches!(s, Scope::View(views) if views.contains(&view))),
        });
    }
    sections.push(HelpSection {
        title: "On selected item",
        entries: by_scope(&|s| matches!(s, Scope::Target(_))),
    });
    sections.push(HelpSection {
        title: "Navigation",
        entries: NAVIGATION_HELP.iter().map(|(k, l)| (k.to_string(), *l)).collect(),
    });
    sections
}


/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

/// `NavigateTo(view)`, or `None` when already there.
fn go(s: &AppState, view: View) -> Option<Command> {
    (s.active_view != view).then(|| Command::NavigateTo(view))
}

/* ============================================================================================== */
fn scope_rank(scope: Scope) -> u8 {
    match scope {
        Scope::View(_) => 0,
        Scope::Target(_) => 1,
        Scope::Global => 2,
    }
}

/* ============================================================================================== */
/// "[ / h" for keyed actions (primary plus alternates); "↵" for keyless default actions.
fn key_label(spec: &ActionSpec) -> String {
    match spec.key {
        Some(k) => std::iter::once(k.display)
            .chain(spec.alt_keys.iter().map(|a| a.display))
            .collect::<Vec<_>>()
            .join(" / "),
        None => "↵".to_string(),
    }
}



/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CostGrouping, CostView, Pane, RunCommandSession};
    use crate::command::Command;
    use crate::test_support::{
        ctx, global, key, key_mod, resource, resource_group, state_with_contexts, STORAGE_TYPE,
        VM_TYPE,
    };

    #[test]
    fn plain_letter_matches_exactly() {
        let q = KeyBinding::plain('q', "q");
        assert!(q.matches(&key(KeyCode::Char('q'))));
        assert!(!q.matches(&key_mod(KeyCode::Char('Q'), KeyModifiers::SHIFT)));
        assert!(!q.matches(&key_mod(KeyCode::Char('q'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn plain_symbol_ignores_shift() {
        let help = KeyBinding::plain('?', "?");
        assert!(help.matches(&key(KeyCode::Char('?'))));
        assert!(help.matches(&key_mod(KeyCode::Char('?'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn ctrl_chord_is_case_insensitive() {
        let g = KeyBinding::ctrl('g', "Ctrl+G");
        assert!(g.matches(&key_mod(KeyCode::Char('g'), KeyModifiers::CONTROL)));
        assert!(g.matches(&key_mod(KeyCode::Char('G'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)));
        assert!(!g.matches(&key(KeyCode::Char('g'))));
    }

    #[test]
    fn function_key_matches_code() {
        let f5 = KeyBinding { code: KeyCode::F(5), mods: KeyModifiers::NONE, display: "F5" };
        assert!(f5.matches(&key(KeyCode::F(5))));
        assert!(!f5.matches(&key(KeyCode::F(6))));
    }

    #[test]
    fn target_from_global_carries_subscription_and_kind() {
        let t = Target::from_global(&global("web-01", VM_TYPE, "sub-b"));
        assert_eq!(t.kind(), Some(TargetKind::Resource));
        assert_eq!(t.name(), "web-01");
        assert!(matches!(t, Target::Resource { ref subscription_id, .. } if subscription_id == "sub-b"));
        assert_eq!(Target::None.kind(), None);
    }

    /// Exhaustive: adding an `ActionId` variant fails to compile until it is
    /// listed here, then fails the test until it is added to `ALL`.
    fn ordinal(id: ActionId) -> usize {
        use ActionId::*;
        match id {
            GoContexts => 0,
            GoResources => 1,
            GoCost => 2,
            GoActivity => 3,
            GoGlobalSearch => 4,
            QuickSwitch => 5,
            Help => 6,
            Quit => 7,
            Refresh => 8,
            FocusSearch => 9,
            PrevPeriod => 10,
            NextPeriod => 11,
            ToggleCostGrouping => 12,
            BroadenActivityScope => 13,
            ToggleFailedOnly => 14,
            RunScript => 15,
            ActivityForTarget => 16,
            CostForResourceGroup => 17,
            RunCommand => 18,
            GoToResourceGroup => 19,
            SwitchToContext => 20,
        }
    }
    const VARIANT_COUNT: usize = 21;

    #[test]
    fn all_lists_every_variant_exactly_once() {
        assert_eq!(ActionId::ALL.len(), VARIANT_COUNT);
        let mut seen = [false; VARIANT_COUNT];
        for id in ActionId::ALL {
            let i = ordinal(*id);
            assert!(!seen[i], "{:?} listed twice", id);
            seen[i] = true;
        }
    }

    #[test]
    fn go_actions_are_none_in_their_own_view() {
        let pairs = [
            (View::ContextSwitcher, ActionId::GoContexts),
            (View::ResourceBrowser, ActionId::GoResources),
            (View::CostExplorer, ActionId::GoCost),
            (View::ActivityLog, ActionId::GoActivity),
            (View::GlobalSearch, ActionId::GoGlobalSearch),
        ];
        for (view, id) in pairs {
            let mut s = state_with_contexts(Some("sub-a"));
            s.active_view = view.clone();
            assert!(id.build(&s, &Target::None).is_none(), "{:?} in {:?}", id, view);
        }
        let s = state_with_contexts(Some("sub-a"));
        assert!(matches!(
            ActionId::GoCost.build(&s, &Target::None),
            Some(Command::NavigateTo(View::CostExplorer))
        ));
    }

    #[test]
    fn refresh_is_view_aware() {
        let mut s = state_with_contexts(Some("sub-a"));
        assert!(matches!(ActionId::Refresh.build(&s, &Target::None), Some(Command::RefreshContextList)));
        s.active_view = View::CostExplorer;
        assert!(matches!(ActionId::Refresh.build(&s, &Target::None), Some(Command::FetchCostSummary { .. })));
        s.active_view = View::Help;
        assert!(ActionId::Refresh.build(&s, &Target::None).is_none());
    }

    #[test]
    fn focus_search_is_view_aware() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::GlobalSearch;
        assert!(matches!(
            ActionId::FocusSearch.build(&s, &Target::None),
            Some(Command::SetGlobalSearchFocus(true))
        ));
        s.active_view = View::CostExplorer;
        assert!(ActionId::FocusSearch.build(&s, &Target::None).is_none());
    }

    #[test]
    fn cost_grouping_hidden_when_drilled_into_rg() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::CostExplorer;
        assert!(ActionId::ToggleCostGrouping.build(&s, &Target::None).is_some());
        s.cost_view = CostView::ResourceGroup("rg".into());
        assert!(ActionId::ToggleCostGrouping.build(&s, &Target::None).is_none());
    }

    #[test]
    fn default_action_by_target() {
        let vm = Target::from_global(&global("web-01", VM_TYPE, "sub-a"));
        let st = Target::from_global(&global("st01", STORAGE_TYPE, "sub-a"));
        assert_eq!(default_action(&vm), Some(ActionId::RunCommand));
        assert_eq!(default_action(&st), Some(ActionId::GoToResourceGroup));
        assert_eq!(default_action(&Target::Context(ctx("sub-a", "t1"))), Some(ActionId::SwitchToContext));
        assert_eq!(default_action(&Target::None), None);
    }

    #[test]
    fn run_command_only_for_vms_and_not_inside_run_command_view() {
        let mut s = state_with_contexts(Some("sub-a"));
        let vm = Target::from_global(&global("web-01", VM_TYPE, "sub-b"));
        let st = Target::from_global(&global("st01", STORAGE_TYPE, "sub-a"));
        assert!(matches!(
            ActionId::RunCommand.build(&s, &vm),
            Some(Command::OpenRunCommand { ref subscription_id, ref vm_name, .. })
                if subscription_id == "sub-b" && vm_name == "web-01"
        ));
        assert!(ActionId::RunCommand.build(&s, &st).is_none());
        s.active_view = View::RunCommand;
        assert!(ActionId::RunCommand.build(&s, &vm).is_none());
    }

    #[test]
    fn cost_and_go_to_rg_wrap_in_context() {
        let s = state_with_contexts(Some("sub-a"));
        let st = Target::from_global(&global("st01", STORAGE_TYPE, "sub-b"));
        match ActionId::CostForResourceGroup.build(&s, &st) {
            Some(Command::InContext { subscription_id, then }) => {
                assert_eq!(subscription_id, "sub-b");
                assert!(matches!(*then, Command::OpenResourceGroupCost { ref resource_group } if resource_group == "rg-app"));
            }
            other => panic!("expected InContext, got {:?}", other),
        }
        assert!(matches!(
            ActionId::GoToResourceGroup.build(&s, &st),
            Some(Command::InContext { then, .. }) if matches!(*then, Command::OpenResourceGroup(ref rg) if rg == "rg-app")
        ));
    }

    #[test]
    fn go_to_rg_hidden_in_resource_browser() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::ResourceBrowser;
        let st = Target::from_global(&global("st01", STORAGE_TYPE, "sub-a"));
        assert!(ActionId::GoToResourceGroup.build(&s, &st).is_none());
    }

    #[test]
    fn current_target_follows_view_and_pane() {
        let mut s = state_with_contexts(Some("sub-a"));
        assert!(matches!(current_target(&s), Target::Context(ref c) if c.subscription.id == "sub-a"));

        s.active_view = View::ResourceBrowser;
        s.resource_groups = vec![resource_group("rg-app")];
        s.resources = vec![resource("web-01", "rg-app", VM_TYPE)];
        s.resource_browser_focus = Pane::Left;
        assert!(matches!(current_target(&s), Target::ResourceGroup { ref name, .. } if name == "rg-app"));
        s.resource_browser_focus = Pane::Right;
        assert!(matches!(current_target(&s), Target::Resource { ref name, .. } if name == "web-01"));

        s.active_view = View::CostExplorer;
        assert_eq!(current_target(&s), Target::None);
        s.cost_view = CostView::Subscription(CostGrouping::ByResourceGroup);
        assert_eq!(current_target(&s), Target::None); // no summary loaded

        s.active_view = View::GlobalSearch;
        s.global_resources = vec![global("st01", STORAGE_TYPE, "sub-b")];
        assert!(matches!(current_target(&s), Target::Resource { ref subscription_id, .. } if subscription_id == "sub-b"));

        s.active_view = View::RunCommand;
        s.run_command = Some(RunCommandSession::new("sub-a".into(), "rg-app".into(), "vm-1".into()));
        assert!(matches!(current_target(&s), Target::Resource { ref resource_type, .. } if resource_type == VM_TYPE));
    }

        fn sample_target(kind: Option<TargetKind>) -> Target {
        match kind {
            None => Target::None,
            Some(TargetKind::Context) => Target::Context(ctx("sub-a", "t1")),
            Some(TargetKind::ResourceGroup) => Target::ResourceGroup {
                subscription_id: "sub-a".into(),
                name: "rg-app".into(),
            },
            Some(TargetKind::Resource) => Target::from_global(&global("web-01", VM_TYPE, "sub-a")),
        }
    }

    #[test]
    fn no_key_conflicts_within_any_view() {
        let reachable: &[(View, &[Option<TargetKind>])] = &[
            (View::ContextSwitcher, &[None, Some(TargetKind::Context)]),
            (View::ResourceBrowser, &[None, Some(TargetKind::ResourceGroup), Some(TargetKind::Resource)]),
            (View::CostExplorer, &[None, Some(TargetKind::ResourceGroup)]),
            (View::ActivityLog, &[None]),
            (View::GlobalSearch, &[None, Some(TargetKind::Resource)]),
            (View::RunCommand, &[Some(TargetKind::Resource)]),
            (View::Help, &[None]),
        ];
        for (view, kinds) in reachable {
            for kind in kinds.iter() {
                let mut s = state_with_contexts(Some("sub-a"));
                s.active_view = view.clone();
                let t = sample_target(*kind);
                let mut seen: Vec<(String, ActionId)> = Vec::new();
                for id in ordered_visible(&s, &t) {
                    let spec = id.spec();
                    for kb in spec.key.iter().chain(spec.alt_keys.iter()) {
                        let sig = format!("{:?}{:?}", kb.code, kb.mods);
                        if let Some((_, other)) = seen.iter().find(|(k, _)| *k == sig) {
                            panic!("{:?}: {:?} and {:?} both bind {}", view, other, id, kb.display);
                        }
                        seen.push((sig, id));
                    }
                }
            }
        }
    }

    #[test]
    fn no_action_binds_a_mechanic_key() {
        let mechanic = [
            KeyCode::Char('j'), KeyCode::Char('k'), KeyCode::Tab, KeyCode::Enter, KeyCode::Esc,
            KeyCode::Backspace, KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right,
        ];
        for id in ActionId::ALL {
            let spec = id.spec();
            for kb in spec.key.iter().chain(spec.alt_keys.iter()) {
                assert!(!mechanic.contains(&kb.code), "{:?} binds mechanic key {}", id, kb.display);
            }
        }
    }

    #[test]
    fn ordered_visible_puts_view_then_target_then_global() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::CostExplorer;
        let t = sample_target(Some(TargetKind::ResourceGroup));
        let rank = |id: &ActionId| match id.spec().scope {
            Scope::View(_) => 0,
            Scope::Target(_) => 1,
            Scope::Global => 2,
        };
        let ranks: Vec<u8> = ordered_visible(&s, &t).iter().map(rank).collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "{:?}", ranks);
        assert!(ranks.contains(&0) && ranks.contains(&1) && ranks.contains(&2));
    }

    #[test]
    fn resolve_key_global_digit_from_any_view() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::ActivityLog;
        assert!(matches!(
            resolve_key(key(KeyCode::Char('1')), &s),
            Some(Command::NavigateTo(View::ContextSwitcher))
        ));
    }

    #[test]
    fn resolve_key_ctrl_g_upper_and_lower() {
        let s = state_with_contexts(Some("sub-a"));
        for c in ['g', 'G'] {
            let cmd = resolve_key(key_mod(KeyCode::Char(c), KeyModifiers::CONTROL), &s);
            assert!(matches!(cmd, Some(Command::OpenModal(_))), "Ctrl+{c}");
        }
    }

    #[test]
    fn resolve_key_help_with_shift() {
        let s = state_with_contexts(Some("sub-a"));
        assert!(matches!(
            resolve_key(key_mod(KeyCode::Char('?'), KeyModifiers::SHIFT), &s),
            Some(Command::NavigateTo(View::Help))
        ));
    }

    #[test]
    fn resolve_key_target_action_uses_selection() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::ResourceBrowser;
        s.resource_groups = vec![resource_group("rg-app")];
        assert!(matches!(
            resolve_key(key(KeyCode::Char('a')), &s),
            Some(Command::OpenResourceActivity { scope: ActivityScope::ResourceGroup { .. } })
        ));
    }

    #[test]
    fn resolve_key_view_action_only_in_its_view() {
        let mut s = state_with_contexts(Some("sub-a"));
        assert!(resolve_key(key(KeyCode::Char('g')), &s).is_none());
        s.active_view = View::CostExplorer;
        assert!(matches!(resolve_key(key(KeyCode::Char('g')), &s), Some(Command::ToggleCostGrouping)));
        assert!(matches!(resolve_key(key(KeyCode::Char('h')), &s), Some(Command::FetchCostSummary { .. })));
    }

    #[test]
    fn default_command_in_global_search_vm_opens_run_command() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::GlobalSearch;
        s.global_resources = vec![global("web-01", VM_TYPE, "sub-a")];
        assert!(uses_default_enter(&s));
        assert!(matches!(default_command(&s), Some(Command::OpenRunCommand { .. })));
    }

        fn has_hint(hints: &[(String, String)], key: &str, label: &str) -> bool {
        hints.iter().any(|(k, l)| k == key && l == label)
    }

    #[test]
    fn hints_are_selection_aware_in_resource_browser() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::ResourceBrowser;
        s.resource_groups = vec![resource_group("rg-app")];
        s.resource_browser_focus = Pane::Right;

        s.resources = vec![resource("vm-1", "rg-app", VM_TYPE)];
        let vm_hints = hints_for(&s);
        assert!(has_hint(&vm_hints, "↵", "run command"), "{:?}", vm_hints);
        assert!(has_hint(&vm_hints, "a", "activity"));
        assert!(has_hint(&vm_hints, "Tab", "panes"));

        s.resources = vec![resource("st01", "rg-app", STORAGE_TYPE)];
        let st_hints = hints_for(&s);
        assert!(!st_hints.iter().any(|(_, l)| l == "run command"));
        assert!(!st_hints.iter().any(|(k, _)| k == "↵"), "go-to-RG is hidden in the browser");
    }

    #[test]
    fn hints_drop_inapplicable_actions() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::CostExplorer;
        assert!(has_hint(&hints_for(&s), "g", "grouping"));
        assert!(has_hint(&hints_for(&s), "[/]", "period"));
        s.cost_view = CostView::ResourceGroup("rg".into());
        let hints = hints_for(&s);
        assert!(!hints.iter().any(|(_, l)| l == "grouping"));
        assert!(has_hint(&hints, "Bksp", "up"));
    }

        #[test]
    fn help_lists_every_action_once() {
        let sections = help_sections();
        for id in ActionId::ALL {
            let label = id.spec().label;
            let hits = sections
                .iter()
                .flat_map(|s| s.entries.iter())
                .filter(|(_, l)| *l == label)
                .count();
            assert!(hits >= 1, "{:?} missing from help", id);
        }
    }

    #[test]
    fn help_shows_alternate_keys_and_default_enter() {
        let sections = help_sections();
        let entries: Vec<&(String, &str)> = sections.iter().flat_map(|s| s.entries.iter()).collect();
        assert!(entries.iter().any(|(k, l)| k == "[ / h" && *l == "Previous period"));
        assert!(entries.iter().any(|(k, l)| k == "↵" && *l == "Run command"));
        assert_eq!(sections.first().map(|s| s.title), Some("Global"));
        assert_eq!(sections.last().map(|s| s.title), Some("Navigation"));
    }

}
