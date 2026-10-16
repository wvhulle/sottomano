use iced::Element;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget, monitor_surfaces::Position},
    readme_showcase::compute_fixture,
    widget_groups::compute_group::ComputeGroup,
    widgets::tooltip_body::Tooltip,
};

pub enum ShowcasedGroup {
    Compute(ComputeGroup),
}

impl ShowcasedGroup {
    pub fn view<'a>(&'a self, config: &'a Config) -> Option<Element<'a, Message>> {
        match self {
            ShowcasedGroup::Compute(group) => group
                .group_content()
                .map(Message::Compute)
                .view(config, Position::Top),
        }
    }

    pub fn update(&mut self, message: Message) {
        match self {
            ShowcasedGroup::Compute(group) => {
                if let Message::Compute(message) = message {
                    let _ = group.update(message);
                }
            }
        }
    }

    pub fn hover(&mut self, target: &HoverTarget, config: &Config) {
        match self {
            ShowcasedGroup::Compute(group) => {
                let _ = group.hover(Some(target), config);
                if let HoverTarget::Compute(target) = target {
                    compute_fixture::on_hover_readings(*target)
                        .into_iter()
                        .for_each(|reading| {
                            let _ = group.update(reading);
                        });
                }
            }
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match self {
            ShowcasedGroup::Compute(group) => {
                let HoverTarget::Compute(target) = target else {
                    return None;
                };
                group.tooltip_for(*target)
            }
        }
    }
}
