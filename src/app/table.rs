use eframe::egui;
use egui_extras::{Column as TableColumn, TableBuilder};

/// One column in a list table.
#[derive(Clone, Copy)]
pub(super) struct Column {
    header: &'static str,
}

impl Column {
    pub(super) const fn new(header: &'static str) -> Self {
        Self { header }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    id: &str,
    columns: &[Column],
    rows: usize,
    mut cell: impl FnMut(&mut egui::Ui, usize, usize),
) {
    if rows == 0 || columns.is_empty() {
        return;
    }

    let width = ui.available_width();
    if width.is_finite() {
        ui.set_min_width(width);
    }

    let row_height = ui.spacing().interact_size.y;
    let mut table = TableBuilder::new(ui)
        .id_salt(id)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .vscroll(false)
        .min_scrolled_height(0.0)
        .auto_shrink([false, true]);
    let last = columns.len() - 1;
    for index in 0..columns.len() {
        // The last column takes whatever width is left, so the table edge
        // stays on the screen. The columns before it can be dragged.
        let spec = if index == last {
            TableColumn::remainder().at_least(40.0).resizable(false)
        } else {
            TableColumn::auto().resizable(true)
        };
        table = table.column(spec);
    }

    let column_count = columns.len();
    table
        .header(row_height, |mut header| {
            for column in columns {
                header.col(|ui| {
                    ui.strong(column.header);
                });
            }
        })
        .body(|mut body| {
            for row in 0..rows {
                body.row(row_height, |mut table_row| {
                    for col in 0..column_count {
                        table_row.col(|ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                            cell(ui, row, col);
                        });
                    }
                });
            }
        });
}
