use std::{collections::BTreeSet, time::Duration};

use bytesize::ByteSize;
use iced::{
    Alignment, Color, Element, Font, Padding,
    alignment::{Horizontal, Vertical},
    font::Weight,
    mouse::Interaction,
    widget::{Row, Space, container, mouse_area, table},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, ratio::percent},
};

use crate::{
    app::Message,
    config::Config,
    data::source::process::{Forest, Process},
    widget_groups::window_group::{WindowGroupMessage, window_list::WindowListMessage},
    widgets::{
        bar_widget::Glyph,
        copyable::copyable,
        phosphor::{self, Phosphor},
        text::{bar_text, column_heading},
        widget_state::Level,
    },
};

const INDENT_PX: f32 = 12.0;
const NAME_WIDTH_PX: f32 = 220.0;
const COLUMN_GAP_PX: f32 = 12.0;
const ROW_GAP_PX: f32 = 3.0;
const CARET_GAP_PX: f32 = 2.0;
const MAX_CHILDREN: usize = 19;
const DIM_ALPHA: f32 = 0.7;
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

    pub fn retain_live(&mut self, forest: &Forest) {
        self.toggled.retain(|&pid| forest.subtree(pid).is_some());
    }

    fn is_unfolded(&self, node: &Process, depth: usize) -> bool {
        let unfolded_by_default = depth == 0;
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
    fn level(&self) -> Level {
        cpu_level(self.cpu)
    }

    fn label(&self) -> String {
        match self.fold {
            Fold::Folded { hidden } => format!("{} +{hidden}", self.name),
            Fold::Leaf | Fold::Unfolded => self.name.clone(),
        }
    }
}

#[derive(Clone, Copy)]
struct Tint {
    warning: Color,
    critical: Color,
}

impl Tint {
    fn or(self, level: Level, calm: Color) -> Color {
        match level {
            Level::Ok => calm,
            Level::Warning => self.warning,
            Level::Critical => self.critical,
        }
    }
}

fn cpu_text(share: Ratio) -> String {
    format!("{:.1}", share.into_format_args(percent, Abbreviation))
}

fn cpu_level(share: Ratio) -> Level {
    if share >= Ratio::new::<percent>(CRITICAL_CPU_PERCENT) {
        Level::Critical
    } else if share >= Ratio::new::<percent>(WARNING_CPU_PERCENT) {
        Level::Warning
    } else {
        Level::Ok
    }
}

pub struct ProcessTree<'a> {
    root: &'a Process,
    folding: &'a Folding,
    config: &'a Config,
    opacity: f32,
}

impl<'a> ProcessTree<'a> {
    pub fn new(root: &'a Process, folding: &'a Folding, config: &'a Config) -> Self {
        Self {
            root,
            folding,
            config,
            opacity: 1.0,
        }
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn view(self) -> Element<'static, Message> {
        let mut branches = Vec::new();
        outline(self.root, 0, self.folding, &mut branches);

        let foreground = self.foreground();
        let flash = self.flash();
        let dim = self.dim();
        let tint = self.tint();
        let font = self.config.font.clone();
        let bold = Font {
            weight: Weight::Bold,
            ..self.config.font.iced_font
        };

        table::table(
            [
                table::column(self.heading("Binary"), move |branch: Branch| {
                    let is_root = branch.depth == 0;
                    let caret_size = font.text_size_px;
                    let row = match branch.entry {
                        Entry::Listed(listing) => {
                            let level = listing.level();
                            let calm = if is_root { flash } else { foreground };
                            let name = bar_text(&font, listing.label()).color(tint.or(level, calm));
                            let name = if is_root { name.font(bold) } else { name };
                            let cell = container(name).max_width(NAME_WIDTH_PX);
                            Row::new()
                                .push(caret(&listing, caret_size, tint.or(level, dim)))
                                .push(copyable(cell, listing.name, Message::CopyText, flash))
                        }
                        Entry::Elided { hidden } => Row::new()
                            .push(blank(caret_size))
                            .push(bar_text(&font, format!("+{hidden} more")).color(dim)),
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
                self.number("Age", |listing| age(listing.uptime)),
            ],
            branches,
        )
        .separator(0.0)
        .padding_x(COLUMN_GAP_PX / 2.0)
        .padding_y(ROW_GAP_PX / 2.0)
        .into()
    }

    fn heading(&self, title: &str) -> Element<'static, Message> {
        column_heading(
            &self.config.font,
            title.to_owned(),
            self.config.palette.inactive.scale_alpha(self.opacity),
        )
        .into()
    }

    fn number(
        &self,
        title: &str,
        field: impl Fn(&Listing) -> String + 'static,
    ) -> table::Column<'static, 'static, Branch, Message> {
        let dim = self.dim();
        let flash = self.flash();
        let tint = self.tint();
        let font = self.config.font.clone();
        table::column(self.heading(title), move |branch: Branch| {
            let Entry::Listed(listing) = branch.entry else {
                return blank(0.0);
            };
            let text = field(&listing);
            let cell = bar_text(&font, text.clone()).color(tint.or(listing.level(), dim));
            copyable(cell, text, Message::CopyText, flash)
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
    }

    fn tint(&self) -> Tint {
        let palette = &self.config.palette;
        Tint {
            warning: Level::Warning.color(palette).scale_alpha(self.opacity),
            critical: Level::Critical.color(palette).scale_alpha(self.opacity),
        }
    }

    fn foreground(&self) -> Color {
        self.config.palette.bar_foreground.scale_alpha(self.opacity)
    }

    fn flash(&self) -> Color {
        self.config.palette.success.scale_alpha(self.opacity)
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..self.config.palette.bar_foreground
        }
        .scale_alpha(self.opacity)
    }
}

fn blank(size: f32) -> Element<'static, Message> {
    Space::new().width(size).height(size).into()
}

fn caret(listing: &Listing, size: f32, color: Color) -> Element<'static, Message> {
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

fn fold_toggle(icon: Phosphor, pid: i32, size: f32, color: Color) -> Element<'static, Message> {
    mouse_area(Glyph::from(icon).view(size, color))
        .on_press(Message::WindowGroup(WindowGroupMessage::WindowList(
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

fn age(uptime: Duration) -> String {
    let secs = uptime.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}
