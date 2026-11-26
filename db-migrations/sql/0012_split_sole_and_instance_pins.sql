CREATE TABLE pinned_sole_groups (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family)
);

CREATE TABLE pinned_instance_groups (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL CHECK (group_instance <> ''),
    PRIMARY KEY (menu_topic, group_family, group_instance)
);

CREATE TABLE pinned_sole_group_items (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    item_key TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, item_key)
);

CREATE TABLE pinned_instance_group_items (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL CHECK (group_instance <> ''),
    item_key TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, group_instance, item_key)
);

INSERT INTO pinned_sole_groups (menu_topic, group_family)
SELECT menu_topic, group_family FROM pinned_widget_groups WHERE group_instance = '';

INSERT INTO pinned_instance_groups (menu_topic, group_family, group_instance)
SELECT menu_topic, group_family, group_instance FROM pinned_widget_groups
WHERE group_instance <> '';

INSERT INTO pinned_sole_group_items (menu_topic, group_family, item_key)
SELECT menu_topic, group_family, item_key FROM pinned_group_items WHERE group_instance = '';

INSERT INTO pinned_instance_group_items (menu_topic, group_family, group_instance, item_key)
SELECT menu_topic, group_family, group_instance, item_key FROM pinned_group_items
WHERE group_instance <> '';

DROP TABLE pinned_widget_groups;

DROP TABLE pinned_group_items;
