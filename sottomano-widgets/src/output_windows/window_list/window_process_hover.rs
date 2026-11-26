use iced::Task;
use sottomano_desktop_sources::desktop_windows_apps::{
    nix_package_facts::PackageWatch, process_tree_inspection::Watched,
};
use sottomano_toml_config::Config;

use super::{WindowList, WindowListMessage};
use crate::{
    clock::Clock,
    hover_cards::hover_card_body::Section,
    output_windows::{
        process_tree::process_tree_widget,
        window_hover_card::{inspection_sections, nix_package_facts_sections},
    },
};

#[derive(Debug, Clone, Copy, Default)]
pub(super) enum ProcessHover {
    #[default]
    Idle,
    Watching(u64),
}

impl WindowList {
    pub fn hover_process(
        &mut self,
        watched: Option<Watched>,
        config: &Config,
        clock: Clock,
    ) -> Task<WindowListMessage> {
        let Some(watched) = watched else {
            self.hovered_window = ProcessHover::Idle;
            self.process_tree.stop();
            self.inspection.stop();
            self.package_facts.stop();
            return Task::none();
        };
        self.hovered_window = ProcessHover::Watching(watched.window);
        Task::batch([
            self.process_tree.start_hover_fetch(
                watched.clone(),
                WindowListMessage::ProcessTreeScanned,
                config,
                clock,
            ),
            self.inspection.start_hover_fetch(
                watched.clone(),
                |fetched| WindowListMessage::ProcessInspected(Box::new(fetched)),
                config,
                clock,
            ),
            self.package_facts.start_hover_fetch(
                PackageWatch {
                    watched,
                    upstream_branch: config.output_windows.nixpkgs_branch.clone(),
                    nixos_flake: config.output_windows.nixos_flake.clone(),
                },
                |fetched| WindowListMessage::PackageFactsRead(Box::new(fetched)),
                config,
                clock,
            ),
        ])
    }

    #[must_use]
    pub fn process_sections(&self) -> Vec<Section> {
        let ProcessHover::Watching(window) = self.hovered_window else {
            return Vec::new();
        };
        [
            self.inspection.sections_described_by(inspection_sections),
            self.package_facts
                .sections_described_by(nix_package_facts_sections),
            self.process_tree.sections_described_by(|tree| {
                let kept = self
                    .window_processes
                    .window_terminal_pids(window)
                    .map_or_else(|| tree.clone(), |kept| tree.pruned_to(kept));
                vec![Section::new(
                    "Processes",
                    process_tree_widget(&kept, &self.folding),
                )]
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}
