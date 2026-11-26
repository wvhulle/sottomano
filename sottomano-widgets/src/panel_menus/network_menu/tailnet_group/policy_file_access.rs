use std::path::PathBuf;

use sottomano_desktop_sources::{
    SnapshotDelivery, network_connectivity::tailscale_policy_file::TailscalePolicyFile,
};

use super::policy_file_indicator::tailscale_policy_file_sections;
use crate::{
    clock::Clock, hover_cards::hover_card_body::Section,
    widgets::hover_fetched_snapshot::HoverFetchedSnapshot,
};

pub(super) enum PolicyFileAccess {
    NoApiToken,
    ApiToken {
        token_path: PathBuf,
        policy_file: Box<HoverFetchedSnapshot<TailscalePolicyFile>>,
    },
}

impl PolicyFileAccess {
    pub(super) fn stop_hover_fetch(&mut self) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.stop();
        }
    }

    pub(super) fn receive(
        &mut self,
        delivery: SnapshotDelivery<TailscalePolicyFile>,
        clock: Clock,
    ) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.receive(delivery, clock);
        }
    }

    pub(super) fn sections(&self) -> Vec<Section> {
        match self {
            PolicyFileAccess::NoApiToken => Vec::new(),
            PolicyFileAccess::ApiToken { policy_file, .. } => {
                policy_file.sections_described_by(tailscale_policy_file_sections)
            }
        }
    }
}
