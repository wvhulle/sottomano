UPDATE pinned_instance_groups
SET group_family = 'steam_game'
WHERE group_family = 'steam_running_game';

UPDATE pinned_instance_group_items
SET group_family = 'steam_game'
WHERE group_family = 'steam_running_game';

DELETE FROM pinned_sole_groups
WHERE group_family = 'steam_library';

DELETE FROM pinned_sole_group_items
WHERE group_family = 'steam_library';
