use std::{env, fs, process};

use super::{
    FlatpakBranch, FlatpakRuntime, InstanceInfo, parsed_instance_info, running_sandboxes_in,
};

#[test]
fn the_info_file_names_the_app_runtime_and_branch() {
    let info_text = [
        "[Application]",
        "name=org.mozilla.firefox",
        "runtime=runtime/org.freedesktop.Platform/x86_64/24.08",
        "",
        "[Instance]",
        "instance-id=1849302754",
        "branch=stable",
        "arch=x86_64",
    ]
    .join("\n");
    let info_text = info_text.as_str();
    assert_eq!(
        parsed_instance_info(info_text),
        Some(InstanceInfo {
            application_id: "org.mozilla.firefox".to_owned(),
            runtime_reference: FlatpakRuntime::Reference(
                "runtime/org.freedesktop.Platform/x86_64/24.08".to_owned()
            ),
            branch: FlatpakBranch::Named("stable".to_owned()),
        })
    );
}

#[test]
fn an_instance_whose_process_has_exited_is_left_out() {
    let scratch = env::temp_dir().join(format!("sottomano-flatpak-{}", process::id()));
    let proc_root = scratch.join("proc");
    let own_pid = process::id();
    fs::create_dir_all(proc_root.join(own_pid.to_string())).unwrap();
    for (instance_id, sandbox_pid) in [("1849302754", own_pid), ("2210043981", 4_000_000)] {
        let instance_directory = scratch.join("instances").join(instance_id);
        fs::create_dir_all(&instance_directory).unwrap();
        fs::write(
            instance_directory.join("info"),
            "[Application]\nname=org.gnome.Calculator\n",
        )
        .unwrap();
        fs::write(instance_directory.join("pid"), sandbox_pid.to_string()).unwrap();
    }
    let listed = running_sandboxes_in(&scratch.join("instances"), &proc_root);
    fs::remove_dir_all(&scratch).unwrap();
    assert_eq!(listed.details.len(), 1);
    assert_eq!(listed.details[0].listing.instance_id, "1849302754");
}
