INSERT OR IGNORE INTO pinned_sole_groups (menu_topic, group_family)
SELECT 'settings', group_family FROM pinned_sole_groups
WHERE menu_topic IN ('appearance', 'behaviour');

INSERT OR IGNORE INTO pinned_instance_groups (menu_topic, group_family, group_instance)
SELECT 'settings', group_family, group_instance FROM pinned_instance_groups
WHERE menu_topic IN ('appearance', 'behaviour');

INSERT OR IGNORE INTO pinned_sole_group_items (menu_topic, group_family, item_key)
SELECT 'settings', group_family, item_key FROM pinned_sole_group_items
WHERE menu_topic IN ('appearance', 'behaviour');

INSERT OR IGNORE INTO pinned_instance_group_items (menu_topic, group_family, group_instance, item_key)
SELECT 'settings', group_family, group_instance, item_key FROM pinned_instance_group_items
WHERE menu_topic IN ('appearance', 'behaviour');

DELETE FROM pinned_sole_groups WHERE menu_topic IN ('appearance', 'behaviour');

DELETE FROM pinned_instance_groups WHERE menu_topic IN ('appearance', 'behaviour');

DELETE FROM pinned_sole_group_items WHERE menu_topic IN ('appearance', 'behaviour');

DELETE FROM pinned_instance_group_items WHERE menu_topic IN ('appearance', 'behaviour');
