//! Data for the system tray context menu (com.canonical.dbusmenu).

use std::collections::HashMap;

use gtk::glib::Variant;
use gtk::prelude::*;

/// One menu item. id 0 is reserved for the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: i32,
        label: String,
        enabled: bool,
    },
    Separator {
        id: i32,
    },
}

impl MenuEntry {
    pub fn id(&self) -> i32 {
        match self {
            Self::Item { id, .. } | Self::Separator { id } => *id,
        }
    }
}

/// dbusmenu labels mark mnemonics with `_`; a `_` in regular text must be written as `__`.
fn escape_label(label: &str) -> String {
    label.replace('_', "__")
}

/// Properties of one item (a{sv}).
pub fn properties(entry: &MenuEntry) -> HashMap<String, Variant> {
    let mut properties = HashMap::new();
    match entry {
        MenuEntry::Item { label, enabled, .. } => {
            properties.insert("label".to_owned(), escape_label(label).to_variant());
            properties.insert("enabled".to_owned(), enabled.to_variant());
        }
        MenuEntry::Separator { .. } => {
            properties.insert("type".to_owned(), "separator".to_variant());
        }
    }
    properties.insert("visible".to_owned(), true.to_variant());
    properties
}

/// The whole menu (`(ia{sv}av)` returned by GetLayout): all items under root node 0.
pub fn layout(entries: &[MenuEntry]) -> Variant {
    let children: Vec<Variant> = entries
        .iter()
        .map(|entry| {
            (entry.id(), properties(entry), Vec::<Variant>::new())
                .to_variant()
                .to_variant()
        })
        .collect();
    let root = HashMap::from([("children-display".to_owned(), "submenu".to_variant())]);
    (0i32, root, children).to_variant()
}

/// GetLayout's return value `(u(ia{sv}av))`: menu revision and the whole menu.
pub fn get_layout_reply(revision: u32, entries: &[MenuEntry]) -> Variant {
    // Can't use `(revision, layout).to_variant()`: a layout that's already a Variant would be wrapped again as `v`.
    Variant::tuple_from_iter([revision.to_variant(), layout(entries)])
}

/// `a(ia{sv})` returned by GetGroupProperties; returns everything when `ids` is empty. Unknown ids are skipped.
pub fn group_properties(entries: &[MenuEntry], ids: &[i32]) -> Variant {
    entries
        .iter()
        .filter(|entry| ids.is_empty() || ids.contains(&entry.id()))
        .map(|entry| (entry.id(), properties(entry)))
        .collect::<Vec<_>>()
        .to_variant()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<MenuEntry> {
        vec![
            MenuEntry::Item {
                id: 1,
                label: "Open Main Window".into(),
                enabled: true,
            },
            MenuEntry::Item {
                id: 2,
                label: "Settings…".into(),
                enabled: false,
            },
            MenuEntry::Separator { id: 3 },
            MenuEntry::Item {
                id: 4,
                label: "Quit".into(),
                enabled: true,
            },
        ]
    }

    #[test]
    fn layout_has_the_dbusmenu_signature() {
        let layout = layout(&entries());
        assert_eq!(layout.type_().as_str(), "(ia{sv}av)");
        let (id, properties, children): (i32, HashMap<String, Variant>, Vec<Variant>) =
            layout.get().unwrap();
        assert_eq!(id, 0);
        assert_eq!(
            properties["children-display"].get::<String>().as_deref(),
            Some("submenu")
        );
        assert_eq!(children.len(), 4);
        let first: (i32, HashMap<String, Variant>, Vec<Variant>) =
            children[0].as_variant().unwrap().get().unwrap();
        assert_eq!(first.0, 1);
        assert_eq!(
            first.1["label"].get::<String>().as_deref(),
            Some("Open Main Window")
        );
        assert!(first.2.is_empty());
    }

    #[test]
    fn get_layout_reply_matches_the_method_signature() {
        let reply = get_layout_reply(7, &entries());
        assert_eq!(reply.type_().as_str(), "(u(ia{sv}av))");
        assert_eq!(reply.child_value(0).get::<u32>(), Some(7));
    }

    #[test]
    fn item_properties() {
        let items = entries();
        let disabled = properties(&items[1]);
        assert_eq!(disabled["enabled"].get::<bool>(), Some(false));
        assert_eq!(disabled["visible"].get::<bool>(), Some(true));
        let separator = properties(&items[2]);
        assert_eq!(
            separator["type"].get::<String>().as_deref(),
            Some("separator")
        );
        assert!(!separator.contains_key("label"));
    }

    #[test]
    fn labels_escape_underscores() {
        let item = MenuEntry::Item {
            id: 1,
            label: "pppoe_out".into(),
            enabled: true,
        };
        assert_eq!(
            properties(&item)["label"].get::<String>().as_deref(),
            Some("pppoe__out")
        );
    }

    #[test]
    fn group_properties_filter_by_id() {
        let all = group_properties(&entries(), &[]);
        assert_eq!(all.type_().as_str(), "a(ia{sv})");
        assert_eq!(all.n_children(), 4);
        let some: Vec<(i32, HashMap<String, Variant>)> =
            group_properties(&entries(), &[4, 99]).get().unwrap();
        assert_eq!(some.len(), 1);
        assert_eq!(some[0].0, 4);
    }
}
