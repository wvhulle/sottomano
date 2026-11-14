use crate::finished_commands::run_to_success;

pub async fn start_waydroid_session() -> Result<(), String> {
    run_to_success(
        "systemd-run",
        [
            "--user",
            "--collect",
            "--quiet",
            "--",
            "waydroid",
            "session",
            "start",
        ],
    )
    .await
}

pub async fn stop_waydroid_session() -> Result<(), String> {
    run_to_success("waydroid", ["session", "stop"]).await
}
