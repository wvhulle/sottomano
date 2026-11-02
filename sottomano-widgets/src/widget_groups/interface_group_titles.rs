use sottomano_sources::network_connectivity::interface_bus_attachment::InterfaceAttachment;

use crate::widget_groups::widget_group_identity::WidgetGroupTitle;

#[derive(Debug, Clone, Copy)]
pub struct AttachmentTitles {
    pub built_in: &'static str,
    pub usb: &'static str,
}

impl AttachmentTitles {
    fn base_title(self, attachment: InterfaceAttachment) -> &'static str {
        match attachment {
            InterfaceAttachment::BuiltIn => self.built_in,
            InterfaceAttachment::Usb => self.usb,
        }
    }

    pub fn group_titles(
        self,
        attachments: impl IntoIterator<Item = InterfaceAttachment>,
    ) -> Vec<WidgetGroupTitle> {
        let attachments: Vec<InterfaceAttachment> = attachments.into_iter().collect();
        attachments
            .iter()
            .enumerate()
            .map(|(position, attachment)| {
                let earlier_with_attachment = attachments[..position]
                    .iter()
                    .filter(|earlier_attachment| *earlier_attachment == attachment)
                    .count();
                WidgetGroupTitle::numbered_from_second(
                    self.base_title(*attachment),
                    earlier_with_attachment,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::network_connectivity::interface_bus_attachment::InterfaceAttachment::{
        self, BuiltIn, Usb,
    };

    use super::AttachmentTitles;

    const ETHERNET_TITLES: AttachmentTitles = AttachmentTitles {
        built_in: "Ethernet",
        usb: "USB Ethernet",
    };

    fn rendered_titles(attachments: impl IntoIterator<Item = InterfaceAttachment>) -> Vec<String> {
        ETHERNET_TITLES
            .group_titles(attachments)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn different_attachments_need_no_ordinal() {
        assert_eq!(
            rendered_titles([BuiltIn, Usb]),
            ["Ethernet", "USB Ethernet"]
        );
    }

    #[test]
    fn same_attachment_is_numbered_from_the_second() {
        assert_eq!(
            rendered_titles([Usb, BuiltIn, Usb, Usb, BuiltIn]),
            [
                "USB Ethernet",
                "Ethernet",
                "USB Ethernet 2",
                "USB Ethernet 3",
                "Ethernet 2"
            ]
        );
    }
}
