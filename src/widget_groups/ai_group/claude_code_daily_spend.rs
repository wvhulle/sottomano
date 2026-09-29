use crate::{
    data::source::ai::claude::ClaudeCcusage,
    widget_groups::ai_group::spend_to_level::level_from_cost,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn state(&self) -> WidgetState {
        level_from_cost(self.today.cost).into()
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.today.active() {
            return TooltipBody::rows(vec![row("Usage", "no activity today")]);
        }
        TooltipBody::rows(vec![
            row("Today", format!("${:.2} USD", self.today.cost)),
            row("Tokens", self.today.tokens.to_string()),
            row("Models", self.today.models.clone()),
        ])
    }
}
