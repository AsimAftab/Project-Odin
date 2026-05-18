use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Color, ContentArrangement, Table};

pub fn styled_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers.iter().map(|h| {
            Cell::new(h)
                .fg(Color::Yellow)
                .add_attribute(Attribute::Bold)
        }));
    table
}

pub fn rule(width: usize) -> String {
    "─".repeat(width)
}
