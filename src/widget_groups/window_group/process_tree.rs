use std::{iter, time::Duration};

use iced::{
    Color, Element, Font, Padding,
    alignment::{Horizontal, Vertical},
    font::Weight,
    widget::{container, table},
};

use crate::{
    app::Message,
    config::Config,
    data::source::process::Process,
    widgets::{
        copyable::copyable,
        text::{bar_text, column_heading},
    },
};

const INDENT_PX: f32 = 12.0;
const NAME_WIDTH_PX: f32 = 220.0;
const COLUMN_GAP_PX: f32 = 12.0;
const ROW_GAP_PX: f32 = 3.0;
const MAX_PROCESSES: usize = 20;
const DIM_ALPHA: f32 = 0.7;

#[derive(Clone)]
struct Branch {
    depth: usize,
    name: String,
    pid: String,
    memory: String,
    threads: String,
    age: String,
}

pub struct ProcessTree<'a> {
    root: &'a Process,
    config: &'a Config,
    opacity: f32,
}

impl<'a> ProcessTree<'a> {
    pub fn new(root: &'a Process, config: &'a Config) -> Self {
        Self {
            root,
            config,
            opacity: 1.0,
        }
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn view(self) -> Element<'static, Message> {
        let branches = if self.root.count() <= MAX_PROCESSES {
            let mut branches = Vec::new();
            flatten(self.root, 0, &mut branches);
            branches
        } else {
            top_level(self.root)
        };

        let foreground = self.foreground();
        let flash = self.flash();
        let font = self.config.font.clone();
        let bold = Font {
            weight: Weight::Bold,
            ..self.config.font.iced_font
        };

        table::table(
            [
                table::column(self.heading("Binary"), move |branch: Branch| {
                    let is_root = branch.depth == 0;
                    let text = branch.name.clone();
                    let name = bar_text(&font, branch.name).color(if is_root {
                        flash
                    } else {
                        foreground
                    });
                    let name = if is_root { name.font(bold) } else { name };
                    let indent = INDENT_PX * branch.depth as f32;
                    let cell = container(name).max_width(NAME_WIDTH_PX);
                    let cell = copyable(cell, text, Message::CopyText, flash);
                    Element::from(container(cell).padding(Padding::ZERO.left(indent)))
                })
                .align_y(Vertical::Top),
                self.number("PID", |branch| branch.pid),
                self.number("Memory", |branch| branch.memory),
                self.number("Threads", |branch| branch.threads),
                self.number("Age", |branch| branch.age),
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
        field: impl Fn(Branch) -> String + 'static,
    ) -> table::Column<'static, 'static, Branch, Message> {
        let dim = self.dim();
        let flash = self.flash();
        let font = self.config.font.clone();
        table::column(self.heading(title), move |branch: Branch| {
            let value = field(branch);
            let cell = bar_text(&font, value.clone()).color(dim);
            if value.is_empty() {
                return cell.into();
            }
            copyable(cell, value, Message::CopyText, flash)
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
    }

    fn foreground(&self) -> Color {
        self.config.palette.bar_foreground.scale_alpha(self.opacity)
    }

    fn flash(&self) -> Color {
        self.config.palette.focus.scale_alpha(self.opacity)
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..self.config.palette.bar_foreground
        }
        .scale_alpha(self.opacity)
    }
}

impl Branch {
    fn of(node: &Process, depth: usize) -> Self {
        Self {
            depth,
            name: node.name.clone(),
            pid: node.pid.to_string(),
            memory: node.memory.display().iec().to_string(),
            threads: format!("{} thr", node.threads),
            age: age(node.uptime),
        }
    }

    fn collapsed(node: &Process, depth: usize) -> Self {
        let branch = Self::of(node, depth);
        match node.count() - 1 {
            0 => branch,
            descendants => Self {
                name: format!("{} +{descendants}", branch.name),
                ..branch
            },
        }
    }
}

fn flatten(node: &Process, depth: usize, branches: &mut Vec<Branch>) {
    branches.push(Branch::of(node, depth));
    for child in &node.children {
        flatten(child, depth + 1, branches);
    }
}

fn top_level(root: &Process) -> Vec<Branch> {
    let shown_children = MAX_PROCESSES - 1;
    let hidden_children = root.children.len().saturating_sub(shown_children);
    iter::once(Branch::of(root, 0))
        .chain(
            root.children
                .iter()
                .take(shown_children)
                .map(|child| Branch::collapsed(child, 1)),
        )
        .chain((hidden_children > 0).then(|| elision(hidden_children)))
        .collect()
}

fn elision(hidden: usize) -> Branch {
    Branch {
        depth: 1,
        name: format!("+{hidden} more"),
        pid: String::new(),
        memory: String::new(),
        threads: String::new(),
        age: String::new(),
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
