use iced::Rectangle;
use iced_test::selector::{Candidate, Selector};
use sottomano_widgets::{hover_tooltips::HoverTarget, widget_groups::group_menus::PinnedEntry};

pub struct HoveredWidget(pub HoverTarget);

impl Selector for HoveredWidget {
    type Output = Rectangle;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Rectangle> {
        let Candidate::Custom { bounds, state, .. } = candidate else {
            return None;
        };
        (state.downcast_ref::<HoverTarget>() == Some(&self.0)).then_some(bounds)
    }

    fn description(&self) -> String {
        format!("the hover area of {:?}", self.0)
    }
}

pub struct MenuPinWidget(pub PinnedEntry);

impl Selector for MenuPinWidget {
    type Output = Rectangle;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Rectangle> {
        let Candidate::Custom { bounds, state, .. } = candidate else {
            return None;
        };
        (state.downcast_ref::<PinnedEntry>() == Some(&self.0)).then_some(bounds)
    }

    fn description(&self) -> String {
        format!("the menu pin of {:?}", self.0)
    }
}

pub struct AnyHoveredWidget;

impl Selector for AnyHoveredWidget {
    type Output = HoverTarget;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<HoverTarget> {
        let Candidate::Custom { state, .. } = candidate else {
            return None;
        };
        state.downcast_ref::<HoverTarget>().cloned()
    }

    fn description(&self) -> String {
        "every hover area".to_owned()
    }
}
