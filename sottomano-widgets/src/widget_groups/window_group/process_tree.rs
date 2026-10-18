use std::{collections::BTreeSet, time::Duration};

use bytesize::ByteSize;
use iced::{
    Alignment, Color, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    mouse::Interaction,
    widget::{Row, Space, container, mouse_area, table},
};
use sottomano_config::{Config, severity::Severity};
use sottomano_sources::process::Process;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, ratio::percent},
};

use crate::{
    shell_message::Message,
    widget_groups::window_group::{WindowGroupMessage, window_list::WindowListMessage},
    widgets::{
        bar_widget::Glyph,
        palette_colors::PaletteColors,
        phosphor::{self, Phosphor},
        pressable_text::pressable_text,
        text::{FontFaces, bar_text, column_heading},
        transitioning_text::{TransitionStyle, TransitioningText},
        widget_state::WidgetState,
    },
};

const INDENT_PX: f32 = 12.0;
const NAME_WIDTH_PX: f32 = 220.0;
const COLUMN_GAP_PX: f32 = 12.0;
const ROW_GAP_PX: f32 = 3.0;
const CARET_GAP_PX: f32 = 2.0;
const MAX_CHILDREN: usize = 19;
const UNFOLDED_DEPTH_BY_DEFAULT: usize = 3;
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

#[derive(Clone, Copy)]
struct Tint {
    warning: Color,
    critical: Color,
}

impl Tint {
    fn or(self, level: Severity, calm: Color) -> Color {
        match level {
            Severity::Low | Severity::Normal => calm,
            Severity::Warning => self.warning,
            Severity::Critical => self.critical,
        }
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
    pub fn view(self) -> Element<'static, Message> {
        let mut branches = Vec::new();
        outline(self.root, 0, self.folding, &mut branches);

        let foreground = self.foreground();
        let flash = self.flash();
        let dim = self.dim();
        let tint = self.tint();
        let font = self.config.theme.font.clone();
        let monospace = self.config.theme.monospace_font.clone();
        let bold = FontFaces::from(&monospace).bold;
        let transition = self.transition;
        let copy_flash = self.copy_flash();

        table::table(
            [
                table::column(self.heading("Binary"), move |branch: Branch| {
                    let is_root = branch.depth == 0;
                    let caret_size = font.text_size_px;
                    let row = match branch.entry {
                        Entry::Listed(listing) => {
                            let level = listing.level();
                            let calm = if is_root { flash } else { foreground };
                            let color = tint.or(level, calm);
                            let name = bar_text(&monospace, listing.label()).color(color);
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
                                .push(caret(&listing, caret_size, tint.or(level, dim)))
                                .push(pressable_text(
                                    cell,
                                    listing.name,
                                    Message::CopyText,
                                    flash,
                                    copy_flash,
                                    Interaction::Copy,
                                ))
                        }
                        Entry::Elided { hidden } => Row::new()
                            .push(blank(caret_size))
                            .push(bar_text(&font, format!("+{hidden} more")).color(dim)),
                    };
                    let row = row.spacing(CARET_GAP_PX).align_y(Alignment::Center);
                    let indent = INDENT_PX * branch.depth as f32;
                    Element::from(container(row).padding(Padding::ZERO.left(indent)))
                })
                .width(Length::Fill)
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
        .width(Length::Fill)
        .separator(0.0)
        .padding_x(COLUMN_GAP_PX / 2.0)
        .padding_y(ROW_GAP_PX / 2.0)
        .into()
    }

    fn heading(&self, title: &str) -> Element<'static, Message> {
        column_heading(
            &self.config.theme.font,
            title.to_owned(),
            PaletteColors::from(&self.config.theme.palette).inactive,
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
        let font = self.config.theme.monospace_font.clone();
        let transition = self.transition;
        let copy_flash = self.copy_flash();
        table::column(self.heading(title), move |branch: Branch| {
            let Entry::Listed(listing) = branch.entry else {
                return blank(0.0);
            };
            let text = field(&listing);
            let color = tint.or(listing.level(), dim);
            let cell = TransitioningText::new(
                bar_text(&font, text.clone()).color(color),
                text.clone(),
                transition,
                &font,
                color,
            )
            .identified_by(&listing.pid);
            pressable_text(
                cell,
                text,
                Message::CopyText,
                flash,
                copy_flash,
                Interaction::Copy,
            )
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
    }

    fn tint(&self) -> Tint {
        let palette = PaletteColors::from(&self.config.theme.palette);
        Tint {
            warning: WidgetState::Warning.color(&palette),
            critical: WidgetState::Critical.color(&palette),
        }
    }

    fn foreground(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).bar_foreground
    }

    fn flash(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).success
    }

    fn copy_flash(&self) -> Duration {
        self.config.tooltips.copy_flash
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..PaletteColors::from(&self.config.theme.palette).bar_foreground
        }
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
