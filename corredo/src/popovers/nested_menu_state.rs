#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NestedMenuLayout {
    #[default]
    Cascading,
    Accordion,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NestedMenuOpenTrigger {
    Hover,
    #[default]
    Press,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpenGroupLimit {
    One,
    #[default]
    Several,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SubmenuConnector {
    #[default]
    Hidden,
    Shown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NestedMenuPresentation {
    pub layout: NestedMenuLayout,
    pub open_trigger: NestedMenuOpenTrigger,
    pub accordion_open_limit: OpenGroupLimit,
    pub submenu_connector: SubmenuConnector,
}

impl NestedMenuPresentation {
    #[must_use]
    pub const fn open_group_limit(self) -> OpenGroupLimit {
        match self.layout {
            NestedMenuLayout::Cascading => OpenGroupLimit::One,
            NestedMenuLayout::Accordion => self.accordion_open_limit,
        }
    }

    #[must_use]
    pub const fn with_layout_toggled(self) -> Self {
        Self {
            layout: match self.layout {
                NestedMenuLayout::Cascading => NestedMenuLayout::Accordion,
                NestedMenuLayout::Accordion => NestedMenuLayout::Cascading,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_trigger_toggled(self) -> Self {
        Self {
            open_trigger: match self.open_trigger {
                NestedMenuOpenTrigger::Hover => NestedMenuOpenTrigger::Press,
                NestedMenuOpenTrigger::Press => NestedMenuOpenTrigger::Hover,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_submenu_connector_toggled(self) -> Self {
        Self {
            submenu_connector: match self.submenu_connector {
                SubmenuConnector::Hidden => SubmenuConnector::Shown,
                SubmenuConnector::Shown => SubmenuConnector::Hidden,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_limit_toggled(self) -> Self {
        Self {
            accordion_open_limit: match self.accordion_open_limit {
                OpenGroupLimit::One => OpenGroupLimit::Several,
                OpenGroupLimit::Several => OpenGroupLimit::One,
            },
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NestedMenuState<SubmenuKey> {
    pub open_submenus: Vec<SubmenuKey>,
    scrolled_px: Vec<f32>,
}

impl<SubmenuKey> Default for NestedMenuState<SubmenuKey> {
    fn default() -> Self {
        Self {
            open_submenus: Vec::new(),
            scrolled_px: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NestedMenuAction<SubmenuKey> {
    Opened(Vec<SubmenuKey>),
    Scrolled { depth: usize, scrolled_px: f32 },
}

impl<SubmenuKey: Clone + PartialEq> NestedMenuState<SubmenuKey> {
    pub fn perform(&mut self, action: NestedMenuAction<SubmenuKey>) {
        match action {
            NestedMenuAction::Opened(open_submenus) => {
                let kept_depths = self
                    .open_submenus
                    .iter()
                    .zip(&open_submenus)
                    .take_while(|(earlier, later)| earlier == later)
                    .count();
                self.scrolled_px.truncate(kept_depths + 1);
                self.open_submenus = open_submenus;
            }
            NestedMenuAction::Scrolled { depth, scrolled_px } => {
                if self.scrolled_px.len() <= depth {
                    self.scrolled_px.resize(depth + 1, 0.0);
                }
                self.scrolled_px[depth] = scrolled_px;
            }
        }
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn adopt(&mut self, presentation: NestedMenuPresentation) {
        if presentation.open_group_limit() == OpenGroupLimit::One {
            self.open_submenus.truncate(1);
        }
    }

    #[must_use]
    pub fn scrolled_px(&self, depth: usize) -> f32 {
        self.scrolled_px.get(depth).copied().unwrap_or_default()
    }

    pub(crate) fn toggling(
        &self,
        submenu_path: &[SubmenuKey],
        open_limit: OpenGroupLimit,
    ) -> NestedMenuAction<SubmenuKey> {
        let Some((toggled, ancestors)) = submenu_path.split_last() else {
            return NestedMenuAction::Opened(self.open_submenus.clone());
        };
        let open_submenus = if self.open_submenus.contains(toggled) {
            match open_limit {
                OpenGroupLimit::One => ancestors.to_vec(),
                OpenGroupLimit::Several => self
                    .open_submenus
                    .iter()
                    .filter(|open| *open != toggled)
                    .cloned()
                    .collect(),
            }
        } else {
            match open_limit {
                OpenGroupLimit::One => submenu_path.to_vec(),
                OpenGroupLimit::Several => self
                    .open_submenus
                    .iter()
                    .cloned()
                    .chain([toggled.clone()])
                    .collect(),
            }
        };
        NestedMenuAction::Opened(open_submenus)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NestedMenuAction, NestedMenuOpenTrigger, NestedMenuPresentation, NestedMenuState,
        OpenGroupLimit,
    };

    const NETWORKS: &str = "networks";
    const AVAILABLE: &str = "available";
    const VPN: &str = "vpn";

    type SubmenuOpenState = NestedMenuState<&'static str>;

    fn performed(
        state: SubmenuOpenState,
        action_of: impl Fn(&SubmenuOpenState) -> NestedMenuAction<&'static str>,
    ) -> SubmenuOpenState {
        let action = action_of(&state);
        let mut next = state;
        next.perform(action);
        next
    }

    #[test]
    fn revealing_a_nested_submenu_opens_its_whole_path() {
        let state = performed(SubmenuOpenState::default(), |_| {
            NestedMenuAction::Opened(vec![NETWORKS, AVAILABLE])
        });
        assert_eq!(state.open_submenus, [NETWORKS, AVAILABLE]);
    }

    #[test]
    fn revealing_a_sibling_branch_closes_the_previous_branch() {
        let state = performed(SubmenuOpenState::default(), |_| {
            NestedMenuAction::Opened(vec![NETWORKS, AVAILABLE])
        });
        let state = performed(state, |_| NestedMenuAction::Opened(vec![VPN]));
        assert_eq!(state.open_submenus, [VPN]);
    }

    #[test]
    fn toggling_with_one_open_keeps_only_the_toggled_path() {
        let state = performed(SubmenuOpenState::default(), |state| {
            state.toggling(&[NETWORKS], OpenGroupLimit::One)
        });
        let state = performed(state, |state| state.toggling(&[VPN], OpenGroupLimit::One));
        assert_eq!(state.open_submenus, [VPN]);
    }

    #[test]
    fn closing_a_nested_submenu_keeps_its_ancestors_open() {
        let state = performed(SubmenuOpenState::default(), |state| {
            state.toggling(&[NETWORKS, AVAILABLE], OpenGroupLimit::One)
        });
        let state = performed(state, |state| {
            state.toggling(&[NETWORKS, AVAILABLE], OpenGroupLimit::One)
        });
        assert_eq!(state.open_submenus, [NETWORKS]);
    }

    #[test]
    fn several_open_submenus_stay_open() {
        let state = performed(SubmenuOpenState::default(), |state| {
            state.toggling(&[NETWORKS], OpenGroupLimit::Several)
        });
        let state = performed(state, |state| {
            state.toggling(&[VPN], OpenGroupLimit::Several)
        });
        assert_eq!(state.open_submenus, [NETWORKS, VPN]);
        let state = performed(state, |state| {
            state.toggling(&[NETWORKS], OpenGroupLimit::Several)
        });
        assert_eq!(state.open_submenus, [VPN]);
    }

    #[test]
    fn adopting_a_limit_of_one_keeps_only_the_first_open_submenu() {
        let mut state = performed(SubmenuOpenState::default(), |state| {
            state.toggling(&[NETWORKS], OpenGroupLimit::Several)
        });
        state = performed(state, |state| {
            state.toggling(&[VPN], OpenGroupLimit::Several)
        });
        state.adopt(NestedMenuPresentation::default());
        assert_eq!(state.open_submenus, [NETWORKS]);
    }

    #[test]
    fn opening_another_branch_resets_the_scroll_of_deeper_panels() {
        let mut state = performed(SubmenuOpenState::default(), |_| {
            NestedMenuAction::Opened(vec![NETWORKS, AVAILABLE])
        });
        state.perform(NestedMenuAction::Scrolled {
            depth: 0,
            scrolled_px: 10.0,
        });
        state.perform(NestedMenuAction::Scrolled {
            depth: 2,
            scrolled_px: 40.0,
        });
        let same_branch = performed(state.clone(), |_| {
            NestedMenuAction::Opened(vec![NETWORKS, AVAILABLE])
        });
        assert!((same_branch.scrolled_px(2) - 40.0).abs() < f32::EPSILON);
        let other_branch = performed(state, |_| NestedMenuAction::Opened(vec![VPN]));
        assert!((other_branch.scrolled_px(0) - 10.0).abs() < f32::EPSILON);
        assert!(other_branch.scrolled_px(1).abs() < f32::EPSILON);
        assert!(other_branch.scrolled_px(2).abs() < f32::EPSILON);
    }

    #[test]
    fn switching_layouts_keeps_each_sub_choice() {
        let chosen = NestedMenuPresentation::default()
            .with_open_trigger_toggled()
            .with_open_limit_toggled();
        let round_trip = chosen.with_layout_toggled().with_layout_toggled();
        assert_eq!(round_trip, chosen);
        assert_eq!(chosen.open_trigger, NestedMenuOpenTrigger::Hover);
        assert_eq!(chosen.accordion_open_limit, OpenGroupLimit::One);
    }

    #[test]
    fn a_cascading_menu_opens_one_group_whatever_the_accordion_limit() {
        let several_stored = NestedMenuPresentation {
            accordion_open_limit: OpenGroupLimit::Several,
            ..NestedMenuPresentation::default()
        };
        assert_eq!(several_stored.open_group_limit(), OpenGroupLimit::One);
    }
}
