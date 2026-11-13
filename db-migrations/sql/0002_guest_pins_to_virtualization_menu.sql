UPDATE pinned_widget_groups
SET menu_topic = 'virtualization'
WHERE menu_topic = 'services'
    AND group_family IN ('virtual_machines', 'nspawn_containers', 'podman_engines');

UPDATE pinned_group_items
SET menu_topic = 'virtualization'
WHERE menu_topic = 'services'
    AND group_family IN ('virtual_machines', 'nspawn_containers', 'podman_engines');
