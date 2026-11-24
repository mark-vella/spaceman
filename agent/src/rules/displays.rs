//! `[[displays]]` as Settings › Layout edits it: one display's single-window width, changed in
//! the entry that applies to it so every other entry, its order and its comments stay put.

use spaceman::{DisplaySelector, config::Config};
use toml_edit::{ArrayOfTables, InlineTable, Item, Table, TableLike, Value, value};

use super::{Doc, push_inline_table, set_table_item};

const WIDTH: &str = "single_window_max_width";

impl Doc {
    /// The file as Spaceman reads it, while it is valid.
    pub fn config(&self) -> Option<Config> {
        Config::parse(&self.source()).ok()
    }

    /// Sets the single-window width on the display `selects` picks, in the `[[displays]]` entry
    /// that applies to it, or in a new entry for `monitor` when none does. `None` removes the
    /// width, and the entry with it once it overrides nothing else.
    pub fn set_single_width_on(
        &mut self,
        selects: impl Fn(&DisplaySelector) -> bool,
        monitor: &str,
        width: Option<i32>,
    ) {
        let index = self
            .config()
            .and_then(|config| config.display_override(selects).map(|(index, _)| index));
        match (index, width) {
            (Some(index), Some(width)) => {
                if let Some(entry) = self.display_entry(index) {
                    set_table_item(entry, WIDTH, value(i64::from(width)));
                }
            }
            (Some(index), None) => {
                let emptied = self.display_entry(index).is_some_and(|entry| {
                    entry.remove(WIDTH);
                    entry.iter().all(|(key, _)| key == "monitor")
                });
                if emptied {
                    self.remove_entry("displays", index);
                }
            }
            (None, Some(width)) => self.push_display(monitor, width),
            (None, None) => {}
        }
    }

    fn display_entry(&mut self, index: usize) -> Option<&mut dyn TableLike> {
        match self.doc.get_mut("displays")? {
            Item::ArrayOfTables(tables) => tables
                .get_mut(index)
                .map(|table| table as &mut dyn TableLike),
            Item::Value(Value::Array(entries)) => entries
                .get_mut(index)
                .and_then(Value::as_inline_table_mut)
                .map(|table| table as &mut dyn TableLike),
            _ => None,
        }
    }

    fn push_display(&mut self, monitor: &str, width: i32) {
        if let Some(Item::Value(Value::Array(entries))) = self.doc.get_mut("displays") {
            let mut table = InlineTable::new();
            table.insert("monitor", Value::from(monitor));
            table.insert(WIDTH, Value::from(i64::from(width)));
            push_inline_table(entries, table);
            return;
        }
        if !self
            .doc
            .get("displays")
            .is_some_and(Item::is_array_of_tables)
        {
            self.doc.remove("displays");
            self.doc
                .insert("displays", Item::ArrayOfTables(ArrayOfTables::new()));
        }
        if let Some(Item::ArrayOfTables(tables)) = self.doc.get_mut("displays") {
            let mut table = Table::new();
            table.insert("monitor", value(monitor));
            table.insert(WIDTH, value(i64::from(width)));
            tables.push(table);
        }
    }
}

#[cfg(test)]
mod tests {
    use spaceman::DisplaySelector;

    use crate::rules::Doc;

    const SOURCE: &str = r#"# Mine.
version = 1

[settings]
single_window_max_width = 1680

# The laptop fills.
[[displays]]
monitor = "Built-in Retina Display"
single_window_max_width = 0 # whole screen

# The office monitor, unplugged today.
[[displays]]
monitor = "LG UltraFine"
single_window_max_width = 1440
"#;

    fn named(name: &'static str) -> impl Fn(&DisplaySelector) -> bool {
        move |selector| selector.as_str() == name
    }

    #[test]
    fn edits_the_entry_that_applies_and_keeps_the_rest_of_the_file() {
        let mut doc = Doc::parse(SOURCE);

        doc.set_single_width_on(named("M34WQ"), "M34WQ", Some(2670));
        doc.set_single_width_on(
            named("Built-in Retina Display"),
            "Built-in Retina Display",
            Some(1920),
        );

        assert_eq!(
            doc.source(),
            r#"# Mine.
version = 1

[settings]
single_window_max_width = 1680

# The laptop fills.
[[displays]]
monitor = "Built-in Retina Display"
single_window_max_width = 1920 # whole screen

# The office monitor, unplugged today.
[[displays]]
monitor = "LG UltraFine"
single_window_max_width = 1440

[[displays]]
monitor = "M34WQ"
single_window_max_width = 2670
"#
        );
        assert!(doc.config().is_some());
    }

    #[test]
    fn clearing_a_display_removes_its_entry_and_nothing_else() {
        let mut doc = Doc::parse(SOURCE);

        doc.set_single_width_on(
            named("Built-in Retina Display"),
            "Built-in Retina Display",
            None,
        );
        doc.set_single_width_on(named("M34WQ"), "M34WQ", None);

        assert_eq!(
            doc.source(),
            r#"# Mine.
version = 1

[settings]
single_window_max_width = 1680

# The office monitor, unplugged today.
[[displays]]
monitor = "LG UltraFine"
single_window_max_width = 1440
"#
        );
    }

    #[test]
    fn an_inline_displays_array_stays_inline() {
        let mut doc = Doc::parse(
            "version = 1\ndisplays = [{ monitor = \"main\", single_window_max_width = 0 }]\n",
        );

        doc.set_single_width_on(
            |selector| *selector == DisplaySelector::Main,
            "M",
            Some(1440),
        );
        doc.set_single_width_on(named("M34WQ"), "M34WQ", Some(2670));

        let config = doc.config().unwrap();
        let entries: Vec<_> = config
            .display_overrides()
            .iter()
            .map(|entry| (entry.monitor.as_str(), entry.single_window_max_width))
            .collect();
        assert_eq!(entries, [("main", 1440), ("M34WQ", 2670)]);
        assert!(
            doc.source().starts_with("version = 1\ndisplays = ["),
            "{}",
            doc.source()
        );
    }
}
