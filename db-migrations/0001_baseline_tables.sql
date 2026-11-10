DROP TABLE IF EXISTS group_menu_presentation;

CREATE TABLE IF NOT EXISTS rate_limit_backoff (
    source TEXT PRIMARY KEY,
    retry_after_unix INTEGER NOT NULL,
    attempts INTEGER NOT NULL,
    status INTEGER NOT NULL,
    message TEXT
);

CREATE TABLE IF NOT EXISTS launch_events (
    workspace TEXT NOT NULL,
    app_id TEXT NOT NULL,
    launched_at TEXT NOT NULL,
    PRIMARY KEY (workspace, app_id, launched_at)
);

CREATE TABLE IF NOT EXISTS battery_draw_model (
    battery TEXT NOT NULL,
    profile TEXT NOT NULL,
    base_w REAL NOT NULL,
    k_bright REAL NOT NULL,
    k_load REAL NOT NULL,
    PRIMARY KEY (battery, profile)
);

CREATE TABLE IF NOT EXISTS battery_draw_covariance (
    battery TEXT NOT NULL,
    profile TEXT NOT NULL,
    row_index INTEGER NOT NULL,
    column_index INTEGER NOT NULL,
    covariance REAL NOT NULL,
    PRIMARY KEY (battery, profile, row_index, column_index),
    CHECK (row_index <= column_index AND column_index < 3)
);

CREATE TABLE IF NOT EXISTS battery_charge_curve (
    battery TEXT NOT NULL,
    bin INTEGER NOT NULL,
    mean_w REAL NOT NULL,
    variance REAL NOT NULL,
    PRIMARY KEY (battery, bin)
);

CREATE TABLE IF NOT EXISTS global_application_menu (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS notification_sound (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS toast_spatial_layout (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces_in_sidebar (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS auto_collapse_workspaces (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS windows_grouped_by_location (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS frequent_apps_hide_open (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS category_apps_sorted_by_usage (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS compositor_driving_controllers (
    controller_key TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS dark_mode_schedule (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    armed INTEGER NOT NULL,
    sunrise_offset INTEGER NOT NULL,
    sunset_offset INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS metered_volume (
    day TEXT PRIMARY KEY,
    received_bytes INTEGER NOT NULL,
    sent_bytes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS window_focus_spans (
    started_at TEXT PRIMARY KEY,
    window_id INTEGER NOT NULL,
    ended_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS window_focus_span_facets (
    span_started_at TEXT NOT NULL
        REFERENCES window_focus_spans (started_at),
    facet TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (span_started_at, facet)
);

CREATE TABLE IF NOT EXISTS presence_spans (
    started_at TEXT PRIMARY KEY,
    ended_at TEXT NOT NULL,
    state TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_window_layout (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    corner_radius_px REAL NOT NULL CHECK (corner_radius_px >= 0),
    gaps_px REAL NOT NULL CHECK (gaps_px >= 0)
);

CREATE TABLE IF NOT EXISTS shell_blur_depth (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    depth TEXT NOT NULL CHECK (depth IN ('windows_behind', 'wallpaper_only'))
);

CREATE TABLE IF NOT EXISTS switch_heat_borders (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    shown INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS quiet_hours (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    follows_schedule INTEGER NOT NULL,
    start_minutes INTEGER NOT NULL,
    end_minutes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS quiet_hours_screen_off (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    after_seconds INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS working_days (
    days_from_monday INTEGER PRIMARY KEY CHECK (days_from_monday BETWEEN 0 AND 6),
    is_working INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS panel_layout (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    disconnected_groups INTEGER NOT NULL,
    auto_collapse_groups INTEGER NOT NULL,
    float_offset_px REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS idle_action_timeouts (
    idle_action TEXT PRIMARY KEY,
    after_seconds INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pinned_widget_groups (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, group_instance)
);

CREATE TABLE IF NOT EXISTS pinned_group_items (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL,
    item_key TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, group_instance, item_key)
);

CREATE TABLE IF NOT EXISTS group_menu_opening_choices (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    layout TEXT NOT NULL CHECK (layout IN ('cascading', 'accordion')),
    cascading_reveal_trigger TEXT NOT NULL CHECK (cascading_reveal_trigger IN ('hover', 'press')),
    accordion_open_limit TEXT NOT NULL CHECK (accordion_open_limit IN ('one', 'several'))
);

CREATE TABLE IF NOT EXISTS layer_appearance (
    layer TEXT PRIMARY KEY,
    opacity REAL NOT NULL,
    corner_radius_px REAL NOT NULL,
    blurred INTEGER NOT NULL,
    drop_shadow INTEGER NOT NULL,
    shadow_softness_px REAL NOT NULL,
    shadow_opacity REAL NOT NULL,
    outline INTEGER NOT NULL,
    outline_opacity REAL NOT NULL
);
