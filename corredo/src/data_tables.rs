mod grouped_table;
pub use grouped_table::{
    ColumnHeading, GroupedTable, GroupedTableStyle, RecordCells, TableColumns, TableRow,
    table_separator, text_grouped_table,
};
mod auto_width_table;
pub use auto_width_table::{
    AutoWidthColumn, AutoWidthTable, SpannableRow, auto_width_column, auto_width_grid,
    auto_width_spanning_grid, auto_width_spanning_table, auto_width_table,
};
