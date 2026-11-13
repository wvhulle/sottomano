CREATE TABLE IF NOT EXISTS window_titles_capped (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    enabled INTEGER NOT NULL
);
