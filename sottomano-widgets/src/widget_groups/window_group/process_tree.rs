use std::{collections::BTreeSet, time::Duration};

use bytesize::ByteSize;
use iced::{
    Alignment, Color, Element, Padding, Theme,
    alignment::{Horizontal, Vertical},
    mouse::Interaction,
    widget::{Row, Space, container, mouse_area, text},
};
use sottomano_config::{Config, severity::Severity};
use sottomano_sources::desktop_windows_apps::process_tree_inspection::Process;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, ratio::percent},
};

use crate::{
    hover_cards::PopupMessage,
    widget_groups::window_group::{WindowGroupMessage, window_list::WindowListMessage},
    widgets::{
        configured_theme::{
            danger_color, secondary_text_color, success_color, success_shades, text_color,
            warning_color,
        },
        content_sized_table::{ContentSizedColumn, content_sized_column, content_sized_table},
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        phosphor::{self, Phosphor},
        pressable_text::pressable_text,
        text::{FontFaces, column_heading, panel_text},
        transitioning_text::{TransitionStyle, TransitioningText},
    },
};

const INDENT_PX: f32 = 12.0;
const NAME_WIDTH_PX: f32 = 220.0;
const COLUMN_GAP_PX: f32 = 12.0;
const ROW_GAP_PX: f32 = 3.0;
const CARET_GAP_PX: f32 = 2.0;
const MAX_CHILDREN: usize = 19;
const UNFOLDED_DEPTH_BY_DEFAULT: usize = 3;
const WARNING_CPU_PERCENT: f64 = 2.0;
const CRITICAL_CPU_PERCENT: f64 = 10.0;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folding {
    toggled: BTreeSet<i32>,
}

impl Folding {
    pub fn toggle(&mut self, pid: i32) {
        if !self.toggled.remove(&pid) {
            self.toggled.insert(pid);
        }
    }

    fn is_unfolded(&self, node: &Process, depth: usize) -> bool {
        let unfolded_by_default = depth < UNFOLDED_DEPTH_BY_DEFAULT;
        unfolded_by_default != self.toggled.contains(&node.pid)
    }
}

#[derive(Clone, Copy)]
enum Fold {
    Leaf,
    Folded { hidden: usize },
    Unfolded,
}

#[derive(Clone)]
struct Branch {
    depth: usize,
    entry: Entry,
}

#[derive(Clone)]
enum Entry {
    Listed(Listing),
    Elided { hidden: usize },
}

#[derive(Clone)]
struct Listing {
    name: String,
    fold: Fold,
    pid: i32,
    cpu: Ratio,
    memory: ByteSize,
    threads: u64,
    uptime: Duration,
}

impl Listing {
    fn level(&self) -> Severity {
        cpu_level(self.cpu)
    }

    fn label(&self) -> String {
        match self.fold {
            Fold::Folded { hidden } => format!("{} +{hidden}", self.name),
            Fold::Leaf | Fold::Unfolded => self.name.clone(),
        }
    }
}

fn severity_tint(
    level: Severity,
    calm: fn(&Theme) -> Color,
) -> impl Fn(&Theme) -> Color + Copy + 'static {
    move |theme| match level {
        Severity::Low | Severity::Normal => calm(theme),
        Severity::Warning => warning_color(theme),
        Severity::Critical => danger_color(theme),
    }
}

fn styled_by(color: impl Fn(&Theme) -> Color + 'static) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(theme)),
    }
}

fn cpu_text(share: Ratio) -> String {
    format!("{:.1}", share.into_format_args(percent, Abbreviation))
}

fn cpu_level(share: Ratio) -> Severity {
    if share >= Ratio::new::<percent>(CRITICAL_CPU_PERCENT) {
        Severity::Critical
    } else if share >= Ratio::new::<percent>(WARNING_CPU_PERCENT) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

pub struct ProcessTree<'a> {
    root: &'a Process,
    folding: &'a Folding,
    config: &'a Config,
    transition: TransitionStyle,
}

impl<'a> ProcessTree<'a> {
    #[must_use]
    pub fn new(
        root: &'a Process,
        folding: &'a Folding,
        config: &'a Config,
        transition: TransitionStyle,
    ) -> Self {
        Self {
            root,
            folding,
            config,
            transition,
        }
    }

    #[must_use]
    pub fn view(self) -> Element<'static, PopupMessage> {
        let mut branches = Vec::new();
        outline(self.root, 0, self.folding, &mut branches);

        let font = self.config.theme.font.clone();
        let monospace = self.config.theme.monospace_font.clone();
        let bold = FontFaces::from(&monospace).bold;
        let transition = self.transition;
        let copy_flash = self.copy_flash();
        let locale = self.config.calendar.locale;

        content_sized_table(
            [
                content_sized_column(self.heading("Binary"), move |branch: Branch| {
                    let is_root = branch.depth == 0;
                    let caret_size = font.text_size_px;
                    let row = match branch.entry {
                        Entry::Listed(listing) => {
                            let level = listing.level();
                            let calm = if is_root { success_color } else { text_color };
                            let color = severity_tint(level, calm);
                            let name =
                                panel_text(&monospace, listing.label()).style(styled_by(color));
                            let name = if is_root { name.font(bold) } else { name };
                            let cell = TransitioningText::new(
                                container(name).max_width(NAME_WIDTH_PX),
                                listing.label(),
                                transition,
                                &monospace,
                                color,
                            )
                            .identified_by(&listing.pid);
                            Row::new()
                                .push(caret(
                                    &listing,
                                    caret_size,
                                    severity_tint(level, secondary_text_color),
                                ))
                                .push(pressable_text(
                                    cell,
                                    listing.name,
                                    PopupMessage::CopyText,
                                    success_shades,
                                    copy_flash,
                                    Interaction::Copy,
                                ))
                        }
                        Entry::Elided { hidden } => Row::new().push(blank(caret_size)).push(
                            panel_text(&font, format!("+{hidden} more")).style(text::secondary),
                        ),
                    };
                    let row = row.spacing(CARET_GAP_PX).align_y(Alignment::Center);
                    let indent = INDENT_PX * branch.depth as f32;
                    Element::from(container(row).padding(Padding::ZERO.left(indent)))
                })
                .align_y(Vertical::Top),
                self.number("PID", |listing| listing.pid.to_string()),
                self.number("CPU", |listing| cpu_text(listing.cpu)),
                self.number("Memory", |listing| {
                    listing.memory.display().iec().to_string()
                }),
                self.number("Threads", |listing| format!("{} thr", listing.threads)),
                self.number("Age", move |listing| duration_text(listing.uptime, locale)),
            ],
            branches,
        )
        .padding_x(COLUMN_GAP_PX / 2.0)
        .padding_y(ROW_GAP_PX / 2.0)
        .into()
    }

    fn heading(&self, title: &str) -> Element<'static, PopupMessage> {
        column_heading(&self.config.theme.font, title.to_owned()).into()
    }

    fn number(
        &self,
        title: &str,
        field: impl Fn(&Listing) -> String + 'static,
    ) -> ContentSizedColumn<'static, Branch, PopupMessage> {
        let font = self.config.theme.monospace_font.clone();
        let transition = self.transition;
        let copy_flash = self.copy_flash();
        content_sized_column(self.heading(title), move |branch: Branch| {
            let Entry::Listed(listing) = branch.entry else {
                return blank(0.0);
            };
            let text = field(&listing);
            let color = severity_tint(listing.level(), secondary_text_color);
            let cell = TransitioningText::new(
                panel_text(&font, text.clone()).style(styled_by(color)),
                text.clone(),
                transition,
                &font,
                color,
            )
            .identified_by(&listing.pid);
            pressable_text(
                cell,
                text,
                PopupMessage::CopyText,
                success_shades,
                copy_flash,
                Interaction::Copy,
            )
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
    }

    fn copy_flash(&self) -> Duration {
        self.config.tooltips.copy_flash
    }
}

fn blank(size: f32) -> Element<'static, PopupMessage> {
    Space::new().width(size).height(size).into()
}

fn caret(
    listing: &Listing,
    size: f32,
    color: impl Fn(&Theme) -> Color + 'static,
) -> Element<'static, PopupMessage> {
    match listing.fold {
        Fold::Leaf => blank(size),
        Fold::Folded { .. } => fold_toggle(
            Phosphor::CaretRight(phosphor::Weight::Bold),
            listing.pid,
            size,
            color,
        ),
        Fold::Unfolded => fold_toggle(
            Phosphor::CaretDown(phosphor::Weight::Bold),
            listing.pid,
            size,
            color,
        ),
    }
}

fn fold_toggle(
    icon: Phosphor,
    pid: i32,
    size: f32,
    color: impl Fn(&Theme) -> Color + 'static,
) -> Element<'static, PopupMessage> {
    mouse_area(IconGlyph::from(icon).view(size, color))
        .on_press(PopupMessage::WindowGroup(WindowGroupMessage::WindowList(
            WindowListMessage::ProcessFoldToggled(pid),
        )))
        .interaction(Interaction::Pointer)
        .into()
}

impl Branch {
    fn of(node: &Process, depth: usize, fold: Fold) -> Self {
        Self {
            depth,
            entry: Entry::Listed(Listing {
                name: node.name.clone(),
                fold,
                pid: node.pid,
                cpu: node.cpu_total(),
                memory: node.memory,
                threads: node.threads,
                uptime: node.uptime,
            }),
        }
    }

    fn unfolded(node: &Process, depth: usize) -> Self {
        let fold = if node.children.is_empty() {
            Fold::Leaf
        } else {
            Fold::Unfolded
        };
        Self::of(node, depth, fold)
    }

    fn folded(node: &Process, depth: usize) -> Self {
        match node.count() - 1 {
            0 => Self::unfolded(node, depth),
            hidden => Self::of(node, depth, Fold::Folded { hidden }),
        }
    }
}

fn outline(node: &Process, depth: usize, folding: &Folding, branches: &mut Vec<Branch>) {
    if !folding.is_unfolded(node, depth) {
        branches.push(Branch::folded(node, depth));
        return;
    }
    branches.push(Branch::unfolded(node, depth));
    for child in node.children.iter().take(MAX_CHILDREN) {
        outline(child, depth + 1, folding, branches);
    }
    let hidden = node.children.len().saturating_sub(MAX_CHILDREN);
    if hidden > 0 {
        branches.push(Branch {
            depth: depth + 1,
            entry: Entry::Elided { hidden },
        });
    }
}
