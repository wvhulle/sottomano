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
    CompositorRulesFile::in_config_home(Path::new("/nonexistent"), Some(compiled_in_compositor()))
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
