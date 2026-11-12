use std::time::{SystemTime, UNIX_EPOCH};

use sottomano_desktop_sources::desktop_windows_apps::dbusmenu_menu_bar_layout::{
    DbusMenuLocation, dbusmenu_proxy,
};
use zbus::zvariant::Value;

const CLICKED_EVENT: &str = "clicked";
const OPENED_EVENT: &str = "opened";

fn event_timestamp() -> u32 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since_epoch) => u32::try_from(since_epoch.as_secs())
            .expect("dbusmenu event timestamps are u32 seconds, which last until 2106"),
        Err(before_epoch) => {
            tracing::warn!("dbusmenu: the wall clock is before 1970: {before_epoch}");
            0
        }
    }
}

async fn send_menu_event(
    connection: &zbus::Connection,
    location: &DbusMenuLocation,
    item_id: i32,
    event_id: &str,
) -> zbus::Result<()> {
    dbusmenu_proxy(connection, location)
        .await?
        .event(item_id, event_id, &Value::I32(0), event_timestamp())
        .await
}

pub async fn click_dbusmenu_item(
    connection: zbus::Connection,
    location: DbusMenuLocation,
    item_id: i32,
) {
    if let Err(click_failure) =
        send_menu_event(&connection, &location, item_id, CLICKED_EVENT).await
    {
        tracing::warn!("global menu: click item {item_id}: {click_failure}");
    }
}

pub async fn open_dbusmenu_submenu(
    connection: zbus::Connection,
    location: DbusMenuLocation,
    item_id: i32,
) {
    let opened = async {
        let menu_proxy = dbusmenu_proxy(&connection, &location).await?;
        menu_proxy.about_to_show(item_id).await?;
        send_menu_event(&connection, &location, item_id, OPENED_EVENT).await
    };
    if let Err(open_failure) = opened.await {
        tracing::debug!("global menu: open submenu {item_id}: {open_failure}");
    }
}

pub async fn prepare_dbusmenu_submenus(
    connection: zbus::Connection,
    location: DbusMenuLocation,
    submenu_ids: Vec<i32>,
) {
    let menu_proxy = match dbusmenu_proxy(&connection, &location).await {
        Ok(menu_proxy) => menu_proxy,
        Err(proxy_failure) => {
            tracing::debug!("global menu: prepare submenus: {proxy_failure}");
            return;
        }
    };
    for submenu_id in submenu_ids {
        if let Err(prepare_failure) = menu_proxy.about_to_show(submenu_id).await {
            tracing::debug!("global menu: prepare submenu {submenu_id}: {prepare_failure}");
        }
    }
}
