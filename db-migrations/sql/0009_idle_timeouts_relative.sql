CREATE TABLE IF NOT EXISTS idle_action_offsets (
    idle_action TEXT PRIMARY KEY,
    after_seconds INTEGER NOT NULL
);

INSERT INTO idle_action_offsets (idle_action, after_seconds)
SELECT
    stage.idle_action,
    MAX(
        0,
        stage.after_seconds - COALESCE(
            (
                SELECT MAX(earlier.after_seconds)
                FROM idle_action_timeouts AS earlier
                WHERE
                    CASE earlier.idle_action
                        WHEN 'lock_user' THEN 0
                        WHEN 'screen_off' THEN 1
                        WHEN 'sleep' THEN 2
                        WHEN 'hibernate' THEN 3
                        WHEN 'power_down' THEN 4
                    END
                    < CASE stage.idle_action
                        WHEN 'lock_user' THEN 0
                        WHEN 'screen_off' THEN 1
                        WHEN 'sleep' THEN 2
                        WHEN 'hibernate' THEN 3
                        WHEN 'power_down' THEN 4
                    END
            ),
            0
        )
    )
FROM idle_action_timeouts AS stage;

DROP TABLE idle_action_timeouts;
