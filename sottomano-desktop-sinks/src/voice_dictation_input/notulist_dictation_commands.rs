use notulist_client::{DictationCommand, send_dictation_command};

pub async fn send_logged_dictation_command(command: DictationCommand) {
    if let Err(dictation_failure) = send_dictation_command(command).await {
        tracing::warn!("notulist {command:?}: {dictation_failure}");
    }
}
