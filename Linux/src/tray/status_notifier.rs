//! D-Bus services for StatusNotifierItem and dbusmenu (all on the GTK main thread, using gio::DBusConnection).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib::{Variant, variant::ObjectPath};
use gtk::prelude::*;
use gtk::{gio, glib};

use super::menu::{self, MenuEntry};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";
const MENU_INTERFACE: &str = "com.canonical.dbusmenu";
const WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";

const INTROSPECTION: &str = r#"
<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="WindowId" type="i" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconThemePath" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <property name="XAyatanaLabel" type="s" access="read"/>
    <property name="XAyatanaLabelGuide" type="s" access="read"/>
    <method name="ContextMenu">
      <arg name="x" type="i" direction="in"/>
      <arg name="y" type="i" direction="in"/>
    </method>
    <method name="Activate">
      <arg name="x" type="i" direction="in"/>
      <arg name="y" type="i" direction="in"/>
    </method>
    <method name="SecondaryActivate">
      <arg name="x" type="i" direction="in"/>
      <arg name="y" type="i" direction="in"/>
    </method>
    <method name="Scroll">
      <arg name="delta" type="i" direction="in"/>
      <arg name="orientation" type="s" direction="in"/>
    </method>
    <signal name="NewTitle"/>
    <signal name="NewIcon"/>
    <signal name="NewToolTip"/>
    <signal name="NewStatus">
      <arg name="status" type="s"/>
    </signal>
    <signal name="XAyatanaNewLabel">
      <arg name="label" type="s"/>
      <arg name="guide" type="s"/>
    </signal>
  </interface>
  <interface name="com.canonical.dbusmenu">
    <property name="Version" type="u" access="read"/>
    <property name="TextDirection" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconThemePath" type="as" access="read"/>
    <method name="GetLayout">
      <arg name="parentId" type="i" direction="in"/>
      <arg name="recursionDepth" type="i" direction="in"/>
      <arg name="propertyNames" type="as" direction="in"/>
      <arg name="revision" type="u" direction="out"/>
      <arg name="layout" type="(ia{sv}av)" direction="out"/>
    </method>
    <method name="GetGroupProperties">
      <arg name="ids" type="ai" direction="in"/>
      <arg name="propertyNames" type="as" direction="in"/>
      <arg name="properties" type="a(ia{sv})" direction="out"/>
    </method>
    <method name="GetProperty">
      <arg name="id" type="i" direction="in"/>
      <arg name="name" type="s" direction="in"/>
      <arg name="value" type="v" direction="out"/>
    </method>
    <method name="Event">
      <arg name="id" type="i" direction="in"/>
      <arg name="eventId" type="s" direction="in"/>
      <arg name="data" type="v" direction="in"/>
      <arg name="timestamp" type="u" direction="in"/>
    </method>
    <method name="EventGroup">
      <arg name="events" type="a(isvu)" direction="in"/>
      <arg name="idErrors" type="ai" direction="out"/>
    </method>
    <method name="AboutToShow">
      <arg name="id" type="i" direction="in"/>
      <arg name="needUpdate" type="b" direction="out"/>
    </method>
    <method name="AboutToShowGroup">
      <arg name="ids" type="ai" direction="in"/>
      <arg name="updatesNeeded" type="ai" direction="out"/>
      <arg name="idErrors" type="ai" direction="out"/>
    </method>
    <signal name="ItemsPropertiesUpdated">
      <arg name="updatedProps" type="a(ia{sv})"/>
      <arg name="removedProps" type="a(ias)"/>
    </signal>
    <signal name="LayoutUpdated">
      <arg name="revision" type="u"/>
      <arg name="parent" type="i"/>
    </signal>
  </interface>
</node>
"#;

/// What the system tray shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrayContent {
    pub icon_name: String,
    /// Text in the top bar (on desktops that support XAyatanaLabel, e.g. Ubuntu).
    pub label: String,
    /// The text at its widest, so the top bar reserves a fixed width and doesn't jump when numbers change.
    pub label_guide: String,
    pub tooltip_title: String,
    pub tooltip_body: String,
    pub menu: Vec<MenuEntry>,
}

/// System tray events.
pub trait TrayHandler {
    /// The icon was clicked.
    fn activate(&self);
    /// A menu item was chosen.
    fn menu_item(&self, id: i32);
    /// The system tray service (StatusNotifierWatcher) appeared or disappeared.
    fn availability_changed(&self, available: bool);
}

struct Inner {
    connection: gio::DBusConnection,
    bus_name: String,
    content: RefCell<TrayContent>,
    menu_revision: Cell<u32>,
    handler: Box<dyn TrayHandler>,
    registrations: RefCell<Vec<gio::RegistrationId>>,
    owner: RefCell<Option<gio::OwnerId>>,
    /// Stops watching the system tray service. gio has another `WatcherId` with the same name that shadows the type returned by `bus_watch_name`,
    /// so the type name can't be written and it's stored as a closure.
    unwatch: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// System tray item. Removed from D-Bus on drop.
pub struct StatusNotifier {
    inner: Rc<Inner>,
}

impl StatusNotifier {
    /// Connects to the session bus, registers the objects, and registers with the system tray service when it appears.
    pub async fn start(
        content: TrayContent,
        handler: Box<dyn TrayHandler>,
    ) -> Result<Self, glib::Error> {
        let connection = gio::bus_get_future(gio::BusType::Session).await?;
        let bus_name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        let inner = Rc::new(Inner {
            connection: connection.clone(),
            bus_name: bus_name.clone(),
            content: RefCell::new(content),
            menu_revision: Cell::new(1),
            handler,
            registrations: RefCell::default(),
            owner: RefCell::default(),
            unwatch: RefCell::default(),
        });

        let node = gio::DBusNodeInfo::for_xml(INTROSPECTION)?;
        let item_info = node
            .lookup_interface(ITEM_INTERFACE)
            .expect("interface in introspection XML");
        let menu_info = node
            .lookup_interface(MENU_INTERFACE)
            .expect("interface in introspection XML");

        let weak = Rc::downgrade(&inner);
        let weak_property = weak.clone();
        let item = connection
            .register_object(ITEM_PATH, &item_info)
            .method_call(move |_, _, _, _, method, _, invocation| {
                if let Some(inner) = weak.upgrade() {
                    // Left and middle click both open the popup; the host shows the right-click menu from the Menu property.
                    if method == "Activate" || method == "SecondaryActivate" {
                        inner.handler.activate();
                    }
                }
                invocation.return_value(None);
            })
            .property(move |_, _, _, _, property| match weak_property.upgrade() {
                Some(inner) => inner.item_property(property),
                None => "".to_variant(),
            })
            .build()?;

        let weak = Rc::downgrade(&inner);
        let weak_property = weak.clone();
        let menu = connection
            .register_object(MENU_PATH, &menu_info)
            .method_call(
                move |_, _, _, _, method, parameters, invocation| match weak.upgrade() {
                    Some(inner) => inner.menu_method(method, &parameters, invocation),
                    None => invocation.return_value(None),
                },
            )
            .property(move |_, _, _, _, property| match weak_property.upgrade() {
                Some(_) => menu_property(property),
                None => "".to_variant(),
            })
            .build()?;
        inner.registrations.replace(vec![item, menu]);

        let owner = gio::bus_own_name_on_connection(
            &connection,
            &bus_name,
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, name| glib::g_warning!("anemobit", "Lost the D-Bus name {name}"),
        );
        inner.owner.replace(Some(owner));

        let weak = Rc::downgrade(&inner);
        let weak_vanished = weak.clone();
        let watcher = gio::bus_watch_name_on_connection(
            &connection,
            WATCHER_NAME,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.register_with_watcher();
                }
            },
            move |_, _| {
                if let Some(inner) = weak_vanished.upgrade() {
                    inner.handler.availability_changed(false);
                }
            },
        );
        inner
            .unwatch
            .replace(Some(Box::new(move || gio::bus_unwatch_name(watcher))));

        Ok(StatusNotifier { inner })
    }

    /// Updates the displayed contents, emitting signals only for parts that changed.
    pub fn update(&self, content: TrayContent) {
        let inner = &self.inner;
        let previous = inner.content.replace(content.clone());
        if previous.label != content.label || previous.label_guide != content.label_guide {
            inner.emit(
                ITEM_PATH,
                ITEM_INTERFACE,
                "XAyatanaNewLabel",
                Some((content.label, content.label_guide).to_variant()),
            );
        }
        if previous.tooltip_title != content.tooltip_title
            || previous.tooltip_body != content.tooltip_body
        {
            inner.emit(ITEM_PATH, ITEM_INTERFACE, "NewToolTip", None);
        }
        if previous.icon_name != content.icon_name {
            inner.emit(ITEM_PATH, ITEM_INTERFACE, "NewIcon", None);
        }
        if previous.menu != content.menu {
            let revision = inner.menu_revision.get() + 1;
            inner.menu_revision.set(revision);
            inner.emit(
                MENU_PATH,
                MENU_INTERFACE,
                "LayoutUpdated",
                Some((revision, 0i32).to_variant()),
            );
        }
    }
}

impl Drop for StatusNotifier {
    fn drop(&mut self) {
        let inner = &self.inner;
        if let Some(unwatch) = inner.unwatch.take() {
            unwatch();
        }
        if let Some(owner) = inner.owner.take() {
            gio::bus_unown_name(owner);
        }
        for registration in inner.registrations.take() {
            let _ = inner.connection.unregister_object(registration);
        }
    }
}

impl Inner {
    fn emit(&self, path: &str, interface: &str, signal: &str, parameters: Option<Variant>) {
        if let Err(error) =
            self.connection
                .emit_signal(None, path, interface, signal, parameters.as_ref())
        {
            glib::g_warning!("anemobit", "Could not emit {signal}: {error}");
        }
    }

    /// Registers with the system tray service; the result (whether there's a system tray) goes to the handler.
    fn register_with_watcher(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.connection.call(
            Some(WATCHER_NAME),
            "/StatusNotifierWatcher",
            WATCHER_NAME,
            "RegisterStatusNotifierItem",
            Some(&(self.bus_name.as_str(),).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
            move |result| {
                let Some(inner) = weak.upgrade() else { return };
                if let Err(error) = &result {
                    glib::g_warning!("anemobit", "Could not register the tray icon: {error}");
                }
                inner.handler.availability_changed(result.is_ok());
            },
        );
    }

    fn item_property(&self, property: &str) -> Variant {
        let content = self.content.borrow();
        let no_pixmap: Vec<(i32, i32, Vec<u8>)> = Vec::new();
        match property {
            "Category" => "ApplicationStatus".to_variant(),
            "Id" => "anemobit".to_variant(),
            "Title" => "Anemobit".to_variant(),
            "Status" => "Active".to_variant(),
            "WindowId" => 0i32.to_variant(),
            "IconName" => content.icon_name.to_variant(),
            // The development build has no installed icons, so tell the system tray to look in data/icons in the source.
            "IconThemePath" => crate::config::ICON_SEARCH_PATH.to_variant(),
            "IconPixmap" => no_pixmap.to_variant(),
            "ToolTip" => (
                content.icon_name.as_str(),
                no_pixmap,
                content.tooltip_title.as_str(),
                content.tooltip_body.as_str(),
            )
                .to_variant(),
            "ItemIsMenu" => false.to_variant(),
            "Menu" => ObjectPath::try_from(MENU_PATH)
                .expect("valid object path")
                .to_variant(),
            "XAyatanaLabel" => content.label.to_variant(),
            "XAyatanaLabelGuide" => content.label_guide.to_variant(),
            _ => "".to_variant(),
        }
    }

    fn menu_method(
        &self,
        method: &str,
        parameters: &Variant,
        invocation: gio::DBusMethodInvocation,
    ) {
        let entries = self.content.borrow().menu.clone();
        match method {
            "GetLayout" => {
                invocation.return_value(Some(&menu::get_layout_reply(
                    self.menu_revision.get(),
                    &entries,
                )));
            }
            "GetGroupProperties" => {
                let ids: Vec<i32> = parameters.child_value(0).get().unwrap_or_default();
                let result = Variant::tuple_from_iter([menu::group_properties(&entries, &ids)]);
                invocation.return_value(Some(&result));
            }
            "GetProperty" => {
                let (id, name): (i32, String) = parameters.get().unwrap_or_default();
                let value = entries
                    .iter()
                    .find(|entry| entry.id() == id)
                    .and_then(|entry| menu::properties(entry).remove(&name))
                    .unwrap_or_else(|| "".to_variant());
                invocation.return_value(Some(&Variant::tuple_from_iter([Variant::from_variant(
                    &value,
                )])));
            }
            "Event" => {
                let id: i32 = parameters.child_value(0).get().unwrap_or_default();
                let event: String = parameters.child_value(1).get().unwrap_or_default();
                invocation.return_value(None);
                if event == "clicked" {
                    self.handler.menu_item(id);
                }
            }
            "EventGroup" => {
                let events: Vec<(i32, String, Variant, u32)> =
                    parameters.child_value(0).get().unwrap_or_default();
                invocation.return_value(Some(&(Vec::<i32>::new(),).to_variant()));
                for (id, event, _, _) in events {
                    if event == "clicked" {
                        self.handler.menu_item(id);
                    }
                }
            }
            "AboutToShow" => invocation.return_value(Some(&(false,).to_variant())),
            "AboutToShowGroup" => {
                invocation.return_value(Some(&(Vec::<i32>::new(), Vec::<i32>::new()).to_variant()));
            }
            _ => invocation.return_value(None),
        }
    }
}

fn menu_property(property: &str) -> Variant {
    match property {
        "Version" => 3u32.to_variant(),
        "TextDirection" => "ltr".to_variant(),
        "Status" => "normal".to_variant(),
        "IconThemePath" => Vec::<String>::new().to_variant(),
        _ => "".to_variant(),
    }
}
