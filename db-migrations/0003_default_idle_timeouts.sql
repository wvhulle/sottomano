INSERT INTO idle_action_timeouts (idle_action, after_seconds)
SELECT seeded.idle_action, seeded.after_seconds
FROM (
    SELECT 'lock_user' AS idle_action, 300 AS after_seconds
    UNION ALL SELECT 'screen_off', 300
    UNION ALL SELECT 'sleep', 1200
) AS seeded
WHERE NOT EXISTS (SELECT 1 FROM idle_action_timeouts);
