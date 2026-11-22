mod grouped_table;
pub use grouped_table::{
    ColumnHeading, GroupedTable, RecordCells, TableColumns, TableRow,
    grouped_table_view::{GroupedTableStyle, table_separator, text_grouped_table},
};
mod auto_width_table;
pub use auto_width_table::{
    AutoWidthColumn, AutoWidthTable, SpannableRow, auto_width_column,
    table_row_assembly::{
        auto_width_grid, auto_width_spanning_grid, auto_width_spanning_table, auto_width_table,
    },
};
