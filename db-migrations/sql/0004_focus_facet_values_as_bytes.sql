CREATE TABLE window_focus_span_facets_as_bytes (
    span_started_at TEXT NOT NULL
        REFERENCES window_focus_spans (started_at),
    facet TEXT NOT NULL,
    value BLOB NOT NULL,
    PRIMARY KEY (span_started_at, facet)
);

INSERT INTO window_focus_span_facets_as_bytes (span_started_at, facet, value)
SELECT span_started_at, facet, CAST(value AS BLOB)
FROM window_focus_span_facets;

DROP TABLE window_focus_span_facets;

ALTER TABLE window_focus_span_facets_as_bytes RENAME TO window_focus_span_facets;
