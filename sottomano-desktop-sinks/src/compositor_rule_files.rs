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
pub struct UnwritableRules<'a> {
    pub path: &'a Path,
    pub kind: io::ErrorKind,
}

#[derive(Debug, Clone)]
struct RulesTarget {
    compositor: CompositorKind,
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompositorRulesFile {
    pub config_home: PathBuf,
    target: Option<RulesTarget>,
    readiness: StylingReadiness,
    written: Option<String>,
    last_write_failure: Option<io::ErrorKind>,
    write_slot: WriteSlot,
    switch_heat: SwitchHeat,
}

impl CompositorRulesFile {
    #[must_use]
    pub fn in_config_home(config_home: &Path, compositor: Option<CompositorKind>) -> Self {
        Self {
            config_home: config_home.to_path_buf(),
            target: compositor.map(|compositor| RulesTarget {
                compositor,
                path: config_home
                    .join("sottomano")
                    .join(compositor.rules_file_name()),
            }),
            readiness: StylingReadiness::AwaitingStyling,
            written: None,
            last_write_failure: None,
            write_slot: WriteSlot::Idle,
            switch_heat: SwitchHeat::default(),
        }
    }

    #[must_use]
    pub fn compositor(&self) -> Option<CompositorKind> {
        self.target.as_ref().map(|target| target.compositor)
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.target.as_ref().map(|target| target.path.as_path())
    }

    #[must_use]
    pub fn unwritable(&self) -> Option<UnwritableRules<'_>> {
        Some(UnwritableRules {
            path: self.path()?,
            kind: self.last_write_failure?,
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
        let Some(target) = &self.target else {
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
                self.written = Some(finished_rules);
                self.last_write_failure = None;
            }
            Err(kind) => self.last_write_failure = Some(kind),
        }
        waiting_rules.map_or(SaveStep::NothingInFlight, |rules| self.start_write(rules))
    }

    fn start_write(&mut self, rules: String) -> SaveStep {
        let Some(target) = &self.target else {
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
            WriteSlot::Idle => self.written.as_deref(),
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

#[cfg(test)]
mod tests {
    use std::{env, fs, io, path::Path, process};

    use sottomano_compositor_model::{
        compositor_styling_model::{
            AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, SwitchHeat,
        },
        compositor_window_facts::WindowAppId,
    };
    use sottomano_compositors::compositor_connection::CompositorKind;

    use super::{CompositorRulesFile, RulesSaveError, RulesWrite, SaveStep, replace};

    fn styled() -> CompositorStyling {
        CompositorStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: AppWindowLayout {
                window_corner_radius_px: 7.5,
                gaps_px: 5.0,
            },
        }
    }

    fn heated_terminal() -> SwitchHeat {
        SwitchHeat::Shown {
            focus_ring_color: "#95e6cb".to_owned(),
            neutral_border_color: "#0b0e14".to_owned(),
            windows: HeatedWindow::titled(
                WindowAppId::Reported("kitty".to_owned()),
                "nvim",
                "#ffb454".to_owned(),
            )
            .into_iter()
            .collect(),
        }
    }

    fn compiled_in_compositor() -> CompositorKind {
        *CompositorKind::COMPILED_IN
            .first()
            .expect("the tests build with at least one compositor")
    }

    fn compiled_in_rules_file() -> CompositorRulesFile {
        CompositorRulesFile::in_config_home(
            Path::new("/nonexistent"),
            Some(compiled_in_compositor()),
        )
    }

    fn rules_file_after_first_write() -> CompositorRulesFile {
        let mut rules = compiled_in_rules_file();
        let _ = rules.styling_loaded(CompositorStyling::default());
        rules.finish_write(Ok(()));
        rules
    }

    #[test]
    fn saving_before_the_styling_is_loaded_is_refused() {
        assert!(matches!(
            compiled_in_rules_file().save(styled()),
            Err(RulesSaveError::StylingNotLoaded)
        ));
    }

    #[test]
    fn without_a_compositor_nothing_is_written() {
        let mut rules = CompositorRulesFile::in_config_home(Path::new("/nonexistent"), None);
        assert!(matches!(
            rules.styling_loaded(styled()),
            Ok(SaveStep::NothingInFlight)
        ));
    }

    #[test]
    fn an_unchanged_heat_is_not_rewritten() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(
            rules.reheat(styled(), heated_terminal()),
            Ok(SaveStep::StartWrite(_))
        ));
        rules.finish_write(Ok(()));
        assert!(matches!(
            rules.reheat(styled(), heated_terminal()),
            Ok(SaveStep::NothingInFlight)
        ));
    }

    #[test]
    fn only_the_newest_rules_follow_a_write_in_flight() {
        let mut rules = rules_file_after_first_write();
        let first_look = styled();
        let second_look = CompositorStyling {
            blur_depth: ShellBlurDepth::WindowsBehind,
            ..first_look
        };
        let third_look = CompositorStyling {
            layout: AppWindowLayout {
                gaps_px: 9.0,
                ..first_look.layout
            },
            ..first_look
        };
        assert!(matches!(
            rules.save(first_look),
            Ok(SaveStep::StartWrite(_))
        ));
        assert!(matches!(
            rules.save(second_look),
            Ok(SaveStep::WriteInFlight)
        ));
        assert!(matches!(
            rules.save(third_look),
            Ok(SaveStep::WriteInFlight)
        ));
        let SaveStep::StartWrite(RulesWrite {
            rules: next_rules, ..
        }) = rules.finish_write(Ok(()))
        else {
            panic!("the waiting rules should start writing");
        };
        assert_eq!(
            Ok(next_rules),
            compiled_in_compositor().rendered_rules(third_look, &SwitchHeat::Hidden)
        );
        assert!(matches!(
            rules.finish_write(Ok(())),
            SaveStep::NothingInFlight
        ));
    }

    #[test]
    fn rules_matching_the_write_in_flight_start_nothing() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
        assert!(matches!(rules.save(styled()), Ok(SaveStep::WriteInFlight)));
        assert!(matches!(
            rules.finish_write(Ok(())),
            SaveStep::NothingInFlight
        ));
    }

    #[test]
    fn a_failed_write_is_reported_and_retried_by_an_identical_save() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
        assert!(matches!(
            rules.finish_write(Err(io::ErrorKind::PermissionDenied)),
            SaveStep::NothingInFlight
        ));
        assert_eq!(
            rules.unwritable().map(|unwritable| unwritable.kind),
            Some(io::ErrorKind::PermissionDenied)
        );
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
    }

    #[tokio::test]
    async fn a_rules_write_replaces_the_file_without_a_staged_leftover() {
        let rules_directory =
            env::temp_dir().join(format!("sottomano-compositor-rules-{}", process::id()));
        let rules_path = rules_directory.join("rules.conf");
        replace(&rules_path, "layout {\n    gaps 5.0\n}\n")
            .await
            .unwrap();
        assert!(
            fs::read_to_string(&rules_path)
                .unwrap()
                .contains("gaps 5.0")
        );
        assert!(!rules_directory.join("rules.conf.staged").exists());
        fs::remove_dir_all(&rules_directory).unwrap();
    }
}
