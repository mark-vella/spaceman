//! `[settings.gaps]` and `settings.single_window_max_width`, edited in place like the rest of
//! the file. A value at its default `0` leaves the file, and so does a table it empties.

use spaceman::spacing::{SingleWidth, Spacing, SpacingKey};
use toml_edit::{Item, Table};

use super::Doc;

impl Doc {
    /// The spacing the file asks for; a missing or out-of-range key reads as `0`.
    pub fn spacing(&self) -> Spacing {
        let settings = self.doc.get("settings").and_then(Item::as_table_like);
        let gaps = settings
            .and_then(|table| table.get("gaps"))
            .and_then(Item::as_table_like);
        let mut spacing = Spacing::NONE;
        for key in SpacingKey::ALL {
            let checked = gaps
                .and_then(|table| table.get(key.gap_name()))
                .and_then(Item::as_integer)
                .and_then(|raw| key.check(raw).ok())
                .unwrap_or(0);
            key.set(&mut spacing, checked);
        }
        spacing.single_window_max_width = settings
            .and_then(|table| table.get(SingleWidth::KEY))
            .and_then(|item| match item.as_value()? {
                toml_edit::Value::Integer(points) => SingleWidth::from_points(*points.value()).ok(),
                toml_edit::Value::String(text) => SingleWidth::from_text(text.value()).ok(),
                _ => None,
            })
            .unwrap_or_default();
        spacing
    }

    /// Writes every spacing key that differs from what the file holds, removing the ones at `0`.
    pub fn set_spacing(&mut self, spacing: Spacing) {
        let current = self.spacing();
        for key in SpacingKey::ALL {
            let value = key.get(&spacing);
            if key.get(&current) == value && self.has_spacing_key(key) == (value != 0) {
                continue;
            }
            let item = (value != 0).then(|| toml_edit::value(i64::from(value)));
            self.set_gap(key.gap_name(), item);
        }
        let width = spacing.single_window_max_width;
        if current.single_window_max_width != width || self.has_single_width() == width.is_off() {
            let item = match width {
                SingleWidth::Off => None,
                SingleWidth::Points(points) => Some(toml_edit::value(i64::from(points.get()))),
                SingleWidth::Percent(_) => Some(toml_edit::value(width.to_string())),
            };
            self.set_setting(SingleWidth::KEY, item);
        }
    }

    fn has_spacing_key(&self, key: SpacingKey) -> bool {
        self.doc
            .get("settings")
            .and_then(Item::as_table_like)
            .and_then(|table| table.get("gaps"))
            .and_then(Item::as_table_like)
            .is_some_and(|gaps| gaps.contains_key(key.gap_name()))
    }

    fn has_single_width(&self) -> bool {
        self.doc
            .get("settings")
            .and_then(Item::as_table_like)
            .is_some_and(|table| table.contains_key(SingleWidth::KEY))
    }

    /// Sets `name` in `[settings.gaps]`, or removes it (and the tables it empties) for `None`.
    fn set_gap(&mut self, name: &str, item: Option<Item>) {
        let Some(item) = item else {
            let Some(settings) = self.doc.get_mut("settings").and_then(Item::as_table_mut) else {
                return;
            };
            let emptied = settings
                .get_mut("gaps")
                .and_then(Item::as_table_mut)
                .is_some_and(|gaps| {
                    gaps.remove(name);
                    gaps.is_empty()
                });
            if emptied {
                settings.remove("gaps");
            }
            if settings.is_empty() {
                self.doc.remove("settings");
            }
            return;
        };
        let settings = self.doc.entry("settings").or_insert_with(|| {
            // Holding only `[settings.gaps]`, the parent needs no header of its own.
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        });
        if !settings.is_table() {
            *settings = Item::Table(Table::new());
        }
        let Some(settings) = settings.as_table_mut() else {
            return;
        };
        let position = settings.position();
        let gaps = settings
            .entry("gaps")
            .or_insert_with(|| Item::Table(Table::new()));
        if !gaps.is_table() {
            *gaps = Item::Table(Table::new());
        }
        if let Some(gaps) = gaps.as_table_mut() {
            // Sharing `[settings]`'s place draws it right after its parent, not at the file's end.
            if let (None, Some(position)) = (gaps.position(), position) {
                gaps.set_position(position);
            }
            super::set_table_item(gaps, name, item);
        }
    }
}
