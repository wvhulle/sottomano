use iced::Element;

use crate::popovers::{
    MenuRow, MenuRowContent,
    menu_row::{IconColumn, RowIcon},
};

pub enum MenuEntry<'a, SubmenuKey, Message> {
    Item(MenuItem<'a, Message>),
    Divider,
    Submenu(MenuSubmenu<'a, SubmenuKey, Message>),
}

impl<'a, SubmenuKey, Message> From<MenuItem<'a, Message>> for MenuEntry<'a, SubmenuKey, Message> {
    fn from(item: MenuItem<'a, Message>) -> Self {
        MenuEntry::Item(item)
    }
}

impl<'a, SubmenuKey, Message> From<MenuSubmenu<'a, SubmenuKey, Message>>
    for MenuEntry<'a, SubmenuKey, Message>
{
    fn from(submenu: MenuSubmenu<'a, SubmenuKey, Message>) -> Self {
        MenuEntry::Submenu(submenu)
    }
}

type RowWrapper<'a, Message> =
    Box<dyn for<'row> FnOnce(Element<'row, Message>) -> Element<'row, Message> + 'a>;

pub(super) enum RowWrapping<'a, Message> {
    Bare,
    Wrapped(RowWrapper<'a, Message>),
}

impl<'a, Message> RowWrapping<'a, Message> {
    pub(super) fn applied(self, row: Element<'a, Message>) -> Element<'a, Message> {
        match self {
            RowWrapping::Bare => row,
            RowWrapping::Wrapped(wrapper) => wrapper(row),
        }
    }
}

pub(super) enum BesideRow<'a, Message> {
    Nothing,
    Card(Element<'a, Message>),
}

pub struct MenuItem<'a, Message> {
    pub(super) row: MenuRow<'a, Message>,
    pub(super) beside: BesideRow<'a, Message>,
    pub(super) wrapping: RowWrapping<'a, Message>,
}

impl<'a, Message> MenuItem<'a, Message> {
    #[must_use]
    pub fn new(row: MenuRow<'a, Message>) -> Self {
        Self {
            row,
            beside: BesideRow::Nothing,
            wrapping: RowWrapping::Bare,
        }
    }

    #[must_use]
    pub fn beside(self, card: impl Into<Element<'a, Message>>) -> Self {
        Self {
            beside: BesideRow::Card(card.into()),
            ..self
        }
    }

    #[must_use]
    pub fn wrapped(
        self,
        wrapper: impl for<'row> FnOnce(Element<'row, Message>) -> Element<'row, Message> + 'a,
    ) -> Self {
        Self {
            wrapping: RowWrapping::Wrapped(Box::new(wrapper)),
            ..self
        }
    }
}

impl<'a, Message> From<MenuRow<'a, Message>> for MenuItem<'a, Message> {
    fn from(row: MenuRow<'a, Message>) -> Self {
        MenuItem::new(row)
    }
}

pub struct MenuSubmenu<'a, SubmenuKey, Message> {
    pub(super) key: SubmenuKey,
    pub(super) header: MenuRowContent<'a, Message>,
    pub(super) entries: Vec<MenuEntry<'a, SubmenuKey, Message>>,
}

impl<'a, SubmenuKey, Message> MenuSubmenu<'a, SubmenuKey, Message> {
    #[must_use]
    pub fn new(
        key: SubmenuKey,
        header: MenuRowContent<'a, Message>,
        entries: impl IntoIterator<Item = MenuEntry<'a, SubmenuKey, Message>>,
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

pub(super) fn icon_column_of<'row, 'a: 'row, Message: 'a>(
    contents: impl IntoIterator<Item = &'row MenuRowContent<'a, Message>>,
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
