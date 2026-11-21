use iced::Element;

use crate::popovers::{
    MenuRow, MenuRowContent,
    menu_row::{IconColumn, RowIcon},
};

pub enum MenuEntry<'view, SubmenuKey, Message> {
    Item(MenuItem<'view, Message>),
    Divider,
    Submenu(MenuSubmenu<'view, SubmenuKey, Message>),
}

impl<'view, SubmenuKey, Message> From<MenuItem<'view, Message>>
    for MenuEntry<'view, SubmenuKey, Message>
{
    fn from(item: MenuItem<'view, Message>) -> Self {
        MenuEntry::Item(item)
    }
}

impl<'view, SubmenuKey, Message> From<MenuSubmenu<'view, SubmenuKey, Message>>
    for MenuEntry<'view, SubmenuKey, Message>
{
    fn from(submenu: MenuSubmenu<'view, SubmenuKey, Message>) -> Self {
        MenuEntry::Submenu(submenu)
    }
}

type RowWrapper<'view, Message> =
    Box<dyn for<'row> FnOnce(Element<'row, Message>) -> Element<'row, Message> + 'view>;

pub(super) enum RowWrapping<'view, Message> {
    Bare,
    Wrapped(RowWrapper<'view, Message>),
}

impl<'view, Message> RowWrapping<'view, Message> {
    pub(super) fn applied(self, row: Element<'view, Message>) -> Element<'view, Message> {
        match self {
            RowWrapping::Bare => row,
            RowWrapping::Wrapped(wrapper) => wrapper(row),
        }
    }
}

pub(super) enum BesideRow<'view, Message> {
    Nothing,
    Card(Element<'view, Message>),
}

pub struct MenuItem<'view, Message> {
    pub(super) row: MenuRow<'view, Message>,
    pub(super) beside: BesideRow<'view, Message>,
    pub(super) wrapping: RowWrapping<'view, Message>,
}

impl<'view, Message> MenuItem<'view, Message> {
    #[must_use]
    pub fn new(row: MenuRow<'view, Message>) -> Self {
        Self {
            row,
            beside: BesideRow::Nothing,
            wrapping: RowWrapping::Bare,
        }
    }

    #[must_use]
    pub fn beside(self, card: impl Into<Element<'view, Message>>) -> Self {
        Self {
            beside: BesideRow::Card(card.into()),
            ..self
        }
    }

    #[must_use]
    pub fn wrapped(
        self,
        wrapper: impl for<'row> FnOnce(Element<'row, Message>) -> Element<'row, Message> + 'view,
    ) -> Self {
        Self {
            wrapping: RowWrapping::Wrapped(Box::new(wrapper)),
            ..self
        }
    }
}

impl<'view, Message> From<MenuRow<'view, Message>> for MenuItem<'view, Message> {
    fn from(row: MenuRow<'view, Message>) -> Self {
        MenuItem::new(row)
    }
}

pub struct MenuSubmenu<'view, SubmenuKey, Message> {
    pub(super) key: SubmenuKey,
    pub(super) header: MenuRowContent<'view, Message>,
    pub(super) entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
}

impl<'view, SubmenuKey, Message> MenuSubmenu<'view, SubmenuKey, Message> {
    #[must_use]
    pub fn new(
        key: SubmenuKey,
        header: MenuRowContent<'view, Message>,
        entries: impl IntoIterator<Item = MenuEntry<'view, SubmenuKey, Message>>,
    ) -> Self {
        Self {
            key,
            header,
            entries: entries.into_iter().collect(),
        }
    }
}

pub(super) fn without_redundant_dividers<SubmenuKey, Message>(
    entries: Vec<MenuEntry<'_, SubmenuKey, Message>>,
) -> Vec<MenuEntry<'_, SubmenuKey, Message>> {
    let mut kept = entries.into_iter().fold(Vec::new(), |mut kept, entry| {
        let leading_divider = matches!(entry, MenuEntry::Divider)
            && kept
                .last()
                .is_none_or(|last| matches!(last, MenuEntry::Divider));
        if !leading_divider {
            kept.push(entry);
        }
        kept
    });
    if kept
        .last()
        .is_some_and(|last| matches!(last, MenuEntry::Divider))
    {
        kept.pop();
    }
    kept
}

pub(super) fn shared_icon_column<'row, 'view: 'row, Message: 'view>(
    contents: impl IntoIterator<Item = &'row MenuRowContent<'view, Message>>,
) -> IconColumn {
    if contents
        .into_iter()
        .any(|content| matches!(content.icon, RowIcon::Shown(_)))
    {
        IconColumn::Reserved
    } else {
        IconColumn::Omitted
    }
}
