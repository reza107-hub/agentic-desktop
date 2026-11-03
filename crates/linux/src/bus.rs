//! Thin blocking client for the AT-SPI accessibility bus.

use agentic_desktop_core::{AppError, ErrorCode};
use serde::Serialize;
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::{DynamicType, OwnedObjectPath, OwnedValue, Type};

pub const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
pub const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
pub const ACTION: &str = "org.a11y.atspi.Action";
pub const COMPONENT: &str = "org.a11y.atspi.Component";
pub const TEXT: &str = "org.a11y.atspi.Text";
pub const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";

const CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// One accessible object: the app's bus name plus the object path.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub bus: String,
    pub path: String,
}

impl Target {
    pub fn new(bus: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            bus: bus.into(),
            path: path.into(),
        }
    }

    pub fn registry_root() -> Self {
        Self::new(REGISTRY_BUS, ROOT_PATH)
    }
}

pub struct Atspi {
    connection: Connection,
}

pub fn map_error(error: zbus::Error) -> AppError {
    let text = error.to_string();
    let lowered = text.to_lowercase();
    if lowered.contains("timed out") || lowered.contains("timeout") {
        return AppError::new(
            ErrorCode::Timeout,
            format!("The app did not answer in time: {text}"),
        );
    }
    if lowered.contains("serviceunknown")
        || lowered.contains("name has no owner")
        || lowered.contains("noreply")
    {
        return AppError::new(ErrorCode::AppNotFound, format!("The app went away: {text}"));
    }
    AppError::internal(format!("Accessibility bus error: {text}"))
}

impl Atspi {
    /// Find the accessibility bus through the session bus, then connect to it.
    pub fn connect() -> Result<Self, AppError> {
        let session = Connection::session().map_err(|e| {
            AppError::new(
                ErrorCode::PermDenied,
                format!("Cannot reach the session bus: {e}"),
            )
            .with_suggestion("Run from your desktop session, with DBUS_SESSION_BUS_ADDRESS set")
        })?;
        let reply = session
            .call_method(
                Some("org.a11y.Bus"),
                "/org/a11y/bus",
                Some("org.a11y.Bus"),
                "GetAddress",
                &(),
            )
            .map_err(|e| {
                AppError::new(
                    ErrorCode::PermDenied,
                    format!("Accessibility bus is not available: {e}"),
                )
                .with_suggestion(
                    "Install and start at-spi2-core, and log in to a graphical session",
                )
            })?;
        let address: String = reply
            .body()
            .deserialize()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let address: zbus::Address = address.as_str().try_into().map_err(map_error)?;
        let connection = Builder::address(address)
            .map_err(map_error)?
            .method_timeout(CALL_TIMEOUT)
            .build()
            .map_err(map_error)?;
        Ok(Self { connection })
    }

    pub fn call<B, R>(
        &self,
        target: &Target,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<R, AppError>
    where
        B: Serialize + DynamicType,
        R: for<'d> serde::Deserialize<'d> + Type,
    {
        let reply = self
            .connection
            .call_method(
                Some(target.bus.as_str()),
                target.path.as_str(),
                Some(interface),
                method,
                body,
            )
            .map_err(map_error)?;
        reply
            .body()
            .deserialize()
            .map_err(|e| AppError::internal(format!("Bad reply to {method}: {e}")))
    }

    fn property(
        &self,
        target: &Target,
        interface: &str,
        name: &str,
    ) -> Result<OwnedValue, AppError> {
        self.call(
            target,
            "org.freedesktop.DBus.Properties",
            "Get",
            &(interface, name),
        )
    }

    pub fn string_property(
        &self,
        target: &Target,
        interface: &str,
        name: &str,
    ) -> Result<String, AppError> {
        let value = self.property(target, interface, name)?;
        String::try_from(value)
            .map_err(|e| AppError::internal(format!("Property {name} is not text: {e}")))
    }

    pub fn int_property(
        &self,
        target: &Target,
        interface: &str,
        name: &str,
    ) -> Result<i32, AppError> {
        let value = self.property(target, interface, name)?;
        i32::try_from(value)
            .map_err(|e| AppError::internal(format!("Property {name} is not a number: {e}")))
    }

    pub fn children(&self, target: &Target) -> Result<Vec<Target>, AppError> {
        let raw: Vec<(String, OwnedObjectPath)> =
            self.call(target, ACCESSIBLE, "GetChildren", &())?;
        Ok(raw
            .into_iter()
            .map(|(bus, path)| Target::new(bus, path.as_str()))
            .collect())
    }

    /// Process id of a bus name, when the bus will tell us.
    pub fn process_id(&self, bus_name: &str) -> Option<u32> {
        let reply = self
            .connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetConnectionUnixProcessID",
                &(bus_name,),
            )
            .ok()?;
        reply.body().deserialize::<u32>().ok()
    }
}

impl Atspi {
    /// Every property of one interface in a single call.
    pub fn all_properties(
        &self,
        target: &Target,
        interface: &str,
    ) -> Result<std::collections::HashMap<String, OwnedValue>, AppError> {
        self.call(
            target,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &(interface,),
        )
    }
}

/// One row of `org.a11y.atspi.Cache.GetItems`, which describes a whole app in a single reply.
#[derive(Debug, Clone)]
pub struct CacheItem {
    pub target: Target,
    pub interfaces: Vec<String>,
    pub name: String,
    pub role: u32,
    pub description: String,
    pub states: Vec<u32>,
}

type RawCacheItem = (
    (String, OwnedObjectPath),
    (String, OwnedObjectPath),
    (String, OwnedObjectPath),
    i32,
    i32,
    Vec<String>,
    String,
    u32,
    String,
    Vec<u32>,
);

impl Atspi {
    pub fn cache_items(&self, app_bus: &str) -> Result<Vec<CacheItem>, AppError> {
        let target = Target::new(app_bus, "/org/a11y/atspi/cache");
        let raw: Vec<RawCacheItem> = self.call(&target, "org.a11y.atspi.Cache", "GetItems", &())?;
        Ok(raw
            .into_iter()
            .map(
                |(
                    me,
                    _app,
                    _parent,
                    _index,
                    _count,
                    interfaces,
                    name,
                    role,
                    description,
                    states,
                )| {
                    CacheItem {
                        target: Target::new(me.0, me.1.as_str()),
                        interfaces,
                        name,
                        role,
                        description,
                        states,
                    }
                },
            )
            .collect())
    }
}
