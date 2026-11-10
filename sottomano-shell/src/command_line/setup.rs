use std::{env, error::Error, fmt::Write, fs, io, path::PathBuf, process};

use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub struct SetupArguments {
    #[arg(long, help = "Print what would be done instead of doing it.")]
    print: bool,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        help = "Leave this step out; repeatable or comma-separated."
    )]
    skip: Vec<SetupStepName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SetupStepName {
    #[value(name = "runtime-tools")]
    RuntimeTools,
    Rapl,
    #[value(name = "amd-uncore")]
    AmdUncore,
    Journal,
}

#[derive(Debug, Subcommand)]
pub enum DistroSetupCommand {
    #[command(about = "Ubuntu: the sudo group and apt runtime tools.")]
    Ubuntu(SetupArguments),
    #[command(about = "Fedora: the wheel group, dnf runtime tools and tuned-ppd.")]
    Fedora(SetupArguments),
    #[command(
        about = "Any other distribution: an auto-detected admin group and no packages; the \n                 runtime tools come from its own package manager."
    )]
    Linux(SetupArguments),
}

impl DistroSetupCommand {
    fn arguments(&self) -> &SetupArguments {
        match self {
            DistroSetupCommand::Ubuntu(arguments)
            | DistroSetupCommand::Fedora(arguments)
            | DistroSetupCommand::Linux(arguments) => arguments,
        }
    }

    fn distro_setup(&self) -> DistroSetup {
        match self {
            DistroSetupCommand::Ubuntu(_) => DistroSetup {
                admin_group: "sudo".to_string(),
                runtime_tools: Some(
                    "apt install -y iproute2 network-manager modemmanager bluez inxi pipewire \
                     wireplumber wireguard-tools xdg-utils wl-clipboard fwupd upower \
                     power-profiles-daemon",
                ),
            },
            DistroSetupCommand::Fedora(_) => DistroSetup {
                admin_group: "wheel".to_string(),
                runtime_tools: Some(
                    "dnf install -y iproute NetworkManager ModemManager bluez inxi pipewire \
                     wireplumber wireguard-tools xdg-utils wl-clipboard fwupd upower tuned-ppd",
                ),
            },
            DistroSetupCommand::Linux(_) => DistroSetup {
                admin_group: auto_detected_admin_group(),
                runtime_tools: None,
            },
        }
    }
}

enum SetupStep {
    WriteFile {
        path: PathBuf,
        contents: String,
    },
    RunCommand {
        summary: String,
        argv: Vec<String>,
        failure: CommandFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandFailure {
    Fatal,
    AppliesAtNextBoot,
}

fn run_command_step(summary: &str, argv: &[&str], failure: CommandFailure) -> SetupStep {
    SetupStep::RunCommand {
        summary: summary.to_string(),
        argv: argv
            .iter()
            .map(|argument| (**argument).to_string())
            .collect(),
        failure,
    }
}

struct DistroSetup {
    admin_group: String,
    runtime_tools: Option<&'static str>,
}

fn auto_detected_admin_group() -> String {
    let group_database = fs::read_to_string("/etc/group").unwrap_or_default();
    if group_database
        .lines()
        .any(|group_line| group_line.starts_with("wheel:"))
    {
        "wheel".to_string()
    } else {
        "sudo".to_string()
    }
}

fn udev_rules(admin_group: &str, skipped: &[SetupStepName]) -> String {
    let mut rules = String::new();
    if !skipped.contains(&SetupStepName::Rapl) {
        rules.push_str(
            "ACTION==\"add\", SUBSYSTEM=\"powercap\", KERNEL=\"intel-rapl:*\", RUN+=\"/bin/chmod \
             0444 /sys%p/energy_uj\"\n",
        );
    }
    writeln!(
        rules,
        "ACTION==\"add|bind\", SUBSYSTEM=\"platform\", DRIVER=\"amd_x3d_vcache\", \
         RUN+=\"/bin/chgrp {admin_group} /sys%p/amd_x3d_mode\", RUN+=\"/bin/chmod 0664 \
         /sys%p/amd_x3d_mode\""
    )
    .expect("writing to a String cannot fail");
    rules
}

fn login_user() -> Option<String> {
    env::var("SUDO_USER")
        .or_else(|_| env::var("USER"))
        .ok()
        .filter(|user| user != "root")
}

fn setup_steps(distro_setup: &DistroSetup, skipped: &[SetupStepName]) -> Vec<SetupStep> {
    let amd_uncore = !skipped.contains(&SetupStepName::AmdUncore);
    let mut modules = "drivetemp\n".to_string();
    if amd_uncore {
        modules.push_str("amd_uncore\n");
    }
    let mut steps = vec![
        SetupStep::WriteFile {
            path: PathBuf::from("/etc/modules-load.d/sottomano.conf"),
            contents: modules,
        },
        run_command_step(
            "load drivetemp now",
            &["modprobe", "drivetemp"],
            CommandFailure::AppliesAtNextBoot,
        ),
    ];
    if amd_uncore {
        steps.push(run_command_step(
            "load amd_uncore now",
            &["modprobe", "amd_uncore"],
            CommandFailure::AppliesAtNextBoot,
        ));
        steps.push(SetupStep::WriteFile {
            path: PathBuf::from("/etc/sysctl.d/90-sottomano.conf"),
            contents: "kernel.perf_event_paranoid = 0\n".to_string(),
        });
    }
    if let Some(runtime_tools) = distro_setup.runtime_tools
        && !skipped.contains(&SetupStepName::RuntimeTools)
    {
        let runtime_tools_argv = runtime_tools
            .split_whitespace()
            .map(str::to_string)
            .collect();
        steps.push(SetupStep::RunCommand {
            summary: "install the tools and services the widgets call".to_string(),
            argv: runtime_tools_argv,
            failure: CommandFailure::Fatal,
        });
    }
    steps.push(SetupStep::WriteFile {
        path: PathBuf::from("/etc/udev/rules.d/90-sottomano.rules"),
        contents: udev_rules(&distro_setup.admin_group, skipped),
    });
    steps.push(run_command_step(
        "reload the udev rules and trigger them",
        &["udevadm", "control", "--reload"],
        CommandFailure::AppliesAtNextBoot,
    ));
    steps.push(run_command_step(
        "apply the new udev rules to present devices",
        &["udevadm", "trigger"],
        CommandFailure::AppliesAtNextBoot,
    ));
    if !skipped.contains(&SetupStepName::Journal) {
        if let Some(user) = login_user() {
            steps.push(SetupStep::RunCommand {
                summary: format!(
                    "add {user} to the systemd-journal group (takes effect at the next login)"
                ),
                argv: vec![
                    "usermod".to_string(),
                    "-aG".to_string(),
                    "systemd-journal".to_string(),
                    user,
                ],
                failure: CommandFailure::Fatal,
            });
        } else {
            println!(
                "# skipping the journal group: no invoking user found in SUDO_USER or USER; pass \
                 --skip journal to silence this note"
            );
            println!();
        }
    }
    steps
}

fn print_steps(steps: &[SetupStep]) {
    for step in steps {
        match step {
            SetupStep::WriteFile { path, contents } => {
                println!("# write {}", path.display());
                println!("{}", contents.trim_end());
                println!();
            }
            SetupStep::RunCommand { summary, argv, .. } => {
                println!("# {summary}");
                println!("$ {}", argv.join(" "));
                println!();
            }
        }
    }
}

fn apply_step(step: &SetupStep) -> Result<(), Box<dyn Error>> {
    match step {
        SetupStep::WriteFile { path, contents } => {
            if let Ok(existing_contents) = fs::read_to_string(path)
                && existing_contents == *contents
            {
                println!("{} already matches; left as is", path.display());
                return Ok(());
            }
            if let Some(parent_directory) = path.parent() {
                fs::create_dir_all(parent_directory)?;
            }
            fs::write(path, contents).map_err(|write_error| {
                if write_error.kind() == io::ErrorKind::PermissionDenied {
                    format!(
                        "writing {} needs root; rerun under sudo or pass --print first",
                        path.display()
                    )
                } else {
                    write_error.to_string()
                }
            })?;
            println!("wrote {}", path.display());
        }
        SetupStep::RunCommand {
            summary,
            argv,
            failure,
        } => {
            let [program, arguments @ ..] = argv.as_slice() else {
                return Err("empty command".into());
            };
            println!("{summary}");
            let command_result = process::Command::new(program).args(arguments).status();
            let command_status = match command_result {
                Ok(command_status) => command_status,
                Err(spawn_error) => match failure {
                    CommandFailure::Fatal => {
                        return Err(format!("running `{argv:?}` failed: {spawn_error}").into());
                    }
                    CommandFailure::AppliesAtNextBoot => {
                        println!(
                            "warning: `{}` failed ({spawn_error}); the written files apply at the \
                             next boot",
                            argv.join(" ")
                        );
                        return Ok(());
                    }
                },
            };
            if !command_status.success() {
                match failure {
                    CommandFailure::Fatal => {
                        return Err(
                            format!("`{}` failed with {command_status}", argv.join(" ")).into()
                        );
                    }
                    CommandFailure::AppliesAtNextBoot => {
                        println!(
                            "warning: `{}` failed with {command_status}; the written files apply \
                             at the next boot",
                            argv.join(" ")
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn run(distro: &DistroSetupCommand) -> Result<(), Box<dyn Error>> {
    let arguments = distro.arguments();
    let distro_setup = distro.distro_setup();
    let steps = setup_steps(&distro_setup, &arguments.skip);
    if !skipped(arguments, SetupStepName::Rapl) {
        println!(
            "# note: the RAPL energy counters become world-readable, which is the PLATYPUS power \
             side channel; skip it with `--skip rapl` on machines shared with untrusted users"
        );
        println!();
    }
    if arguments.print {
        print_steps(&steps);
        return Ok(());
    }
    for step in &steps {
        apply_step(step)?;
    }
    println!("sottomano system setup done");
    Ok(())
}

fn skipped(arguments: &SetupArguments, step: SetupStepName) -> bool {
    arguments.skip.contains(&step)
}
