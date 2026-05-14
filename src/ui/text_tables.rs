use comfy_table::{presets::UTF8_FULL, Attribute, Cell, ContentArrangement, Table};

pub fn styled_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|h| Cell::new(h).add_attribute(Attribute::Bold)),
        );
    table
}

pub fn rule(width: usize) -> String {
    "═".repeat(width)
}
