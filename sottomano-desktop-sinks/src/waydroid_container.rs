use sottomano_desktop_sources::waydroid_container::WaydroidContainerManagerProxy;

pub async fn stop_waydroid_container() -> Result<(), String> {
    let stopped = async {
        WaydroidContainerManagerProxy::new(&zbus::Connection::system().await?)
            .await?
            .stop(false)
            .await
    };
    stopped
        .await
        .map_err(|failure: zbus::Error| format!("waydroid: {failure}"))
}
