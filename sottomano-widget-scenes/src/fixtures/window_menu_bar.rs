use sottomano_desktop_sources::desktop_windows_apps::{
    application_menu_nodes::{
        MenuItemIcon, MenuItemSensitivity, MenuKind, MenuLabel, MenuNode, ToggleCheck, ToggleMark,
    },
    dbusmenu_menu_bar_layout::DbusMenuLocation,
    focused_window_menu_bar::{FocusedMenuBar, MenuActivationRoute, MenuBarMatch, ShownMenuBar},
};
use sottomano_widgets::{
    cascading_menus::window_menu_bar::{WindowMenuBar, WindowMenuBarMessage},
    icon_catalog::drawable_icon_catalog::IconCatalog,
};
use zbus::{names::OwnedBusName, zvariant::OwnedObjectPath};

use crate::showcased_group::{ShowcasedGroup, ShowcasedWindowMenuBar};

pub const FILE_MENU_ID: i32 = 1;
pub const FORMAT_MENU_ID: i32 = 5;

struct MenuEntryIds {
    next_id: i32,
}

impl MenuEntryIds {
    fn next_entry_id(&mut self) -> i32 {
        self.next_id += 1;
        self.next_id
    }
}

fn labelled_node(id: i32, label: &str, kind: MenuKind, children: Vec<MenuNode>) -> MenuNode {
    MenuNode {
        id,
        label: MenuLabel::Labelled(label.to_owned()),
        sensitivity: MenuItemSensitivity::Enabled,
        kind,
        icon: MenuItemIcon::NoIcon,
        children,
    }
}

fn plain_menu_items(entry_ids: &mut MenuEntryIds, labels: &[&str]) -> Vec<MenuNode> {
    labels
        .iter()
        .map(|label| {
            labelled_node(
                entry_ids.next_entry_id(),
                label,
                MenuKind::Standard,
                Vec::new(),
            )
        })
        .collect()
}

fn separator_entry(entry_ids: &mut MenuEntryIds) -> MenuNode {
    MenuNode {
        label: MenuLabel::Unlabelled,
        ..labelled_node(
            entry_ids.next_entry_id(),
            "",
            MenuKind::Separator,
            Vec::new(),
        )
    }
}

fn checked_toggle(entry_ids: &mut MenuEntryIds, label: &str, check: ToggleCheck) -> MenuNode {
    labelled_node(
        entry_ids.next_entry_id(),
        label,
        MenuKind::Toggle {
            mark: ToggleMark::Checkbox,
            check,
        },
        Vec::new(),
    )
}

fn top_menu(id: i32, label: &str, children: Vec<MenuNode>) -> MenuNode {
    labelled_node(id, label, MenuKind::Submenu, children)
}

fn nested_submenu(entry_ids: &mut MenuEntryIds, label: &str, children: Vec<MenuNode>) -> MenuNode {
    labelled_node(
        entry_ids.next_entry_id(),
        label,
        MenuKind::Submenu,
        children,
    )
}

fn office_top_menus() -> Vec<MenuNode> {
    let mut entry_ids = MenuEntryIds { next_id: 100 };
    let recent_documents = plain_menu_items(
        &mut entry_ids,
        &[
            "quarterly-report.odt",
            "budget-2027.ods",
            "meeting-notes.odt",
        ],
    );
    let mut file_children = plain_menu_items(&mut entry_ids, &["New", "Open…"]);
    file_children.push(nested_submenu(
        &mut entry_ids,
        "Recent Documents",
        recent_documents,
    ));
    file_children.push(separator_entry(&mut entry_ids));
    file_children.extend(plain_menu_items(
        &mut entry_ids,
        &["Save", "Save As…", "Export as PDF…"],
    ));
    file_children.push(MenuNode {
        sensitivity: MenuItemSensitivity::Disabled,
        ..labelled_node(
            entry_ids.next_entry_id(),
            "Print…",
            MenuKind::Standard,
            Vec::new(),
        )
    });
    file_children.push(separator_entry(&mut entry_ids));
    file_children.extend(plain_menu_items(&mut entry_ids, &["Quit"]));
    let edit_children = plain_menu_items(
        &mut entry_ids,
        &["Undo", "Redo", "Cut", "Copy", "Paste", "Find & Replace…"],
    );
    let view_children = vec![
        checked_toggle(&mut entry_ids, "Ruler", ToggleCheck::Checked),
        checked_toggle(&mut entry_ids, "Status Bar", ToggleCheck::Checked),
        checked_toggle(&mut entry_ids, "Sidebar", ToggleCheck::Unchecked),
    ];
    let insert_children = plain_menu_items(&mut entry_ids, &["Image…", "Table…", "Page Break"]);
    let text_styles = plain_menu_items(
        &mut entry_ids,
        &["Bold", "Italic", "Underline", "Strikethrough"],
    );
    let mut format_children = vec![nested_submenu(&mut entry_ids, "Text", text_styles)];
    format_children.extend(plain_menu_items(
        &mut entry_ids,
        &["Paragraph…", "Character…"],
    ));
    let tools_children = plain_menu_items(&mut entry_ids, &["Spelling…", "Word Count", "Options…"]);
    vec![
        top_menu(FILE_MENU_ID, "File", file_children),
        top_menu(2, "Edit", edit_children),
        top_menu(3, "View", view_children),
        top_menu(4, "Insert", insert_children),
        top_menu(FORMAT_MENU_ID, "Format", format_children),
        top_menu(6, "Tools", tools_children),
    ]
}

fn office_menu_location() -> DbusMenuLocation {
    DbusMenuLocation {
        bus_name: OwnedBusName::try_from(":1.52").expect("static bus name"),
        menu_path: OwnedObjectPath::try_from("/MenuBar/1").expect("static object path"),
    }
}

pub fn group() -> ShowcasedGroup {
    let mut window_menu_bar = WindowMenuBar::default();
    let _ = window_menu_bar.update(WindowMenuBarMessage::MenuBarChanged(FocusedMenuBar::Shown(
        ShownMenuBar {
            top_menus: office_top_menus(),
            activation_route: MenuActivationRoute::DbusMenu(office_menu_location()),
            menu_bar_match: MenuBarMatch::ExactWindow,
        },
    )));
    ShowcasedGroup::WindowMenuBar(Box::new(ShowcasedWindowMenuBar {
        group: window_menu_bar,
        icon_catalog: IconCatalog::default(),
    }))
}
