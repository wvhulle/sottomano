UPDATE window_focus_span_facets
SET value = CAST(value AS BLOB)
WHERE typeof(value) = 'text';
