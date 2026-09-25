//! Action registry: the single table of named actions. Keybindings, hint bars,
//! the help screen, and the command palette all read from here.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::View;
use crate::domain::models::{AzureContext, GlobalResource};

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
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{global, key, key_mod, VM_TYPE};

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
}
