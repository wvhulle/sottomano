INSERT INTO pinned_sole_groups (menu_topic, group_family)
VALUES
    ('agenda', 'clock'),
    ('agenda', 'notification'),
    ('session', 'tray')
ON CONFLICT DO NOTHING;
