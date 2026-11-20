INSERT OR IGNORE INTO pinned_widget_groups (menu_topic, group_family, group_instance)
SELECT DISTINCT menu_topic, group_family, 'chain'
FROM pinned_widget_groups
WHERE group_family = 'idle_timeouts'
  AND group_instance IN ('lock', 'screen_off', 'sleep', 'hibernate', 'power_down');

DELETE FROM pinned_widget_groups
WHERE group_family = 'idle_timeouts'
  AND group_instance IN ('lock', 'screen_off', 'sleep', 'hibernate', 'power_down');

DELETE FROM pinned_group_items
WHERE group_family = 'idle_timeouts'
  AND group_instance IN ('lock', 'screen_off', 'sleep', 'hibernate', 'power_down');
