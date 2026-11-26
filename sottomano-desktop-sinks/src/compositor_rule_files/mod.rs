use std::{
    error::Error,
    fmt, io, mem,
    path::{Path, PathBuf},
};

use sottomano_compositor_model::{
    compositor_requests::CompositorRequest,
    compositor_styling_model::{CompositorStyling, SwitchHeat},
};
use sottomano_compositors::compositor_connection::{CompositorKind, send_compositor_request};
use tokio::fs;

#[cfg(test)]
mod compositor_rule_files_tests;

#[derive(Debug, Clone)]
enum WriteSlot {
    Idle,
    Writing { rules: String },
    WritingThenNext { rules: String, next_rules: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StylingReadiness {
    AwaitingStyling,
    Loaded,
}

#[derive(Debug)]
pub struct RulesWrite {
    path: PathBuf,
    rules: String,
}

impl RulesWrite {
    pub async fn run(self) -> Result<(), io::ErrorKind> {
        replace(&self.path, &self.rules)
            .await
            .map_err(|write_error| write_error.kind())?;
        if let Err(reload_failure) = send_compositor_request(CompositorRequest::ReloadConfig).await
        {
            tracing::warn!("reloading the compositor rules: {reload_failure}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum SaveStep {
    NothingInFlight,
    WriteInFlight,
    StartWrite(RulesWrite),
}

#[derive(Debug)]
pub enum RulesSaveError {
    StylingNotLoaded,
    Rendering(String),
}

impl Error for RulesSaveError {}

impl fmt::Display for RulesSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StylingNotLoaded => write!(formatter, "the stored styling is not loaded yet"),
            Self::Rendering(render_error) => write!(formatter, "rendering failed: {render_error}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnwritableRules<'path> {
    pub path: &'path Path,
    pub kind: io::ErrorKind,
}

#[derive(Debug, Clone)]
struct RulesTarget {
    compositor: CompositorKind,
    path: PathBuf,
}

#[derive(Debug, Clone)]
enum RulesDestination {
    NoCompositor,
    Compositor(RulesTarget),
}

#[derive(Debug, Clone)]
enum WrittenRules {
    NothingWritten,
    Written(String),
}

#[derive(Debug, Clone, Copy)]
enum WriteHealth {
    Writable,
    Unwritable(io::ErrorKind),
}

#[derive(Debug, Clone)]
pub struct CompositorRulesFile {
    pub config_home: PathBuf,
    destination: RulesDestination,
    readiness: StylingReadiness,
    written: WrittenRules,
    write_health: WriteHealth,
    write_slot: WriteSlot,
    switch_heat: SwitchHeat,
}

impl CompositorRulesFile {
    #[must_use]
    pub fn in_config_home(config_home: &Path, compositor: Option<CompositorKind>) -> Self {
        Self {
            config_home: config_home.to_path_buf(),
            destination: match compositor {
                Some(compositor) => RulesDestination::Compositor(RulesTarget {
                    compositor,
                    path: config_home
                        .join("sottomano")
                        .join(compositor.rules_file_name()),
                }),
                None => RulesDestination::NoCompositor,
            },
            readiness: StylingReadiness::AwaitingStyling,
            written: WrittenRules::NothingWritten,
            write_health: WriteHealth::Writable,
            write_slot: WriteSlot::Idle,
            switch_heat: SwitchHeat::default(),
        }
    }

    #[must_use]
    pub fn compositor(&self) -> Option<CompositorKind> {
        self.target().map(|target| target.compositor)
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.target().map(|target| target.path.as_path())
    }

    fn target(&self) -> Option<&RulesTarget> {
        match &self.destination {
            RulesDestination::Compositor(target) => Some(target),
            RulesDestination::NoCompositor => None,
        }
    }

    #[must_use]
    pub fn unwritable(&self) -> Option<UnwritableRules<'_>> {
        let WriteHealth::Unwritable(kind) = self.write_health else {
            return None;
        };
        Some(UnwritableRules {
            path: self.path()?,
            kind,
        })
    }

    pub fn styling_loaded(
        &mut self,
        styling: CompositorStyling,
    ) -> Result<SaveStep, RulesSaveError> {
        self.readiness = StylingReadiness::Loaded;
        self.save(styling)
    }

    pub fn reheat(
        &mut self,
        styling: CompositorStyling,
        switch_heat: SwitchHeat,
    ) -> Result<SaveStep, RulesSaveError> {
        if switch_heat == self.switch_heat {
            return Ok(self.current_step());
        }
        self.switch_heat = switch_heat;
        self.save(styling)
    }

    pub fn save(&mut self, styling: CompositorStyling) -> Result<SaveStep, RulesSaveError> {
        if self.readiness == StylingReadiness::AwaitingStyling {
            return Err(RulesSaveError::StylingNotLoaded);
        }
        let RulesDestination::Compositor(target) = &self.destination else {
            return Ok(SaveStep::NothingInFlight);
        };
        let rules = target
            .compositor
            .rendered_rules(styling, &self.switch_heat)
            .map_err(RulesSaveError::Rendering)?;
        if self.newest_rules() == Some(rules.as_str()) {
            return Ok(self.current_step());
        }
        Ok(match mem::replace(&mut self.write_slot, WriteSlot::Idle) {
            WriteSlot::Idle => self.start_write(rules),
            WriteSlot::Writing { rules: in_flight }
            | WriteSlot::WritingThenNext {
                rules: in_flight, ..
            } => {
                self.write_slot = WriteSlot::WritingThenNext {
                    rules: in_flight,
                    next_rules: rules,
                };
                SaveStep::WriteInFlight
            }
        })
    }

    pub fn finish_write(&mut self, written: Result<(), io::ErrorKind>) -> SaveStep {
        let (finished_rules, waiting_rules) =
            match mem::replace(&mut self.write_slot, WriteSlot::Idle) {
                WriteSlot::Idle => return SaveStep::NothingInFlight,
                WriteSlot::Writing { rules } => (rules, None),
                WriteSlot::WritingThenNext { rules, next_rules } => (rules, Some(next_rules)),
            };
        match written {
            Ok(()) => {
                self.written = WrittenRules::Written(finished_rules);
                self.write_health = WriteHealth::Writable;
            }
            Err(kind) => self.write_health = WriteHealth::Unwritable(kind),
        }
        waiting_rules.map_or(SaveStep::NothingInFlight, |rules| self.start_write(rules))
    }

    fn start_write(&mut self, rules: String) -> SaveStep {
        let RulesDestination::Compositor(target) = &self.destination else {
            return SaveStep::NothingInFlight;
        };
        let path = target.path.clone();
        self.write_slot = WriteSlot::Writing {
            rules: rules.clone(),
        };
        SaveStep::StartWrite(RulesWrite { path, rules })
    }

    fn current_step(&self) -> SaveStep {
        match self.write_slot {
            WriteSlot::Idle => SaveStep::NothingInFlight,
            WriteSlot::Writing { .. } | WriteSlot::WritingThenNext { .. } => {
                SaveStep::WriteInFlight
            }
        }
    }

    fn newest_rules(&self) -> Option<&str> {
        match &self.write_slot {
            WriteSlot::WritingThenNext { next_rules, .. } => Some(next_rules),
            WriteSlot::Writing { rules } => Some(rules),
            WriteSlot::Idle => match &self.written {
                WrittenRules::Written(rules) => Some(rules),
                WrittenRules::NothingWritten => None,
            },
        }
    }
}

async fn replace(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).await?;
    }
    let mut staged_name = path.file_name().unwrap_or_default().to_owned();
    staged_name.push(".staged");
    let staged = path.with_file_name(staged_name);
    fs::write(&staged, contents).await?;
    fs::rename(&staged, path).await
}
