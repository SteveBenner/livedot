use crate::control;
use crate::model::{Snapshot, Status};
use crate::monitor::Shared;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::object_server::SignalEmitter;

pub const BUS_NAME: &str = "io.livedot.Daemon";
pub const OBJECT_PATH: &str = "/io/livedot/Daemon";

pub struct DaemonInterface {
    shared: Arc<Shared>,
}

#[zbus::interface(name = "io.livedot.Daemon1")]
impl DaemonInterface {
    fn get_state(&self) -> String {
        serde_json::to_string(&self.shared.snapshot()).unwrap_or_default()
    }

    fn control(&self, id: &str, action: &str) -> zbus::fdo::Result<()> {
        control::request(&self.shared, id, action).map_err(zbus::fdo::Error::Failed)
    }

    fn prompt(&self, id: &str, text: &str) -> zbus::fdo::Result<()> {
        control::prompt(&self.shared, id, None, text).map_err(zbus::fdo::Error::Failed)
    }

    fn prompt_button(&self, id: &str, button: &str, text: &str) -> zbus::fdo::Result<()> {
        control::prompt(&self.shared, id, Some(button), text).map_err(zbus::fdo::Error::Failed)
    }

    fn item_action(&self, id: &str, item: &str, action: &str) -> zbus::fdo::Result<()> {
        control::item(&self.shared, id, item, action).map_err(zbus::fdo::Error::Failed)
    }

    fn set_shown(&self, id: &str, shown: bool) -> zbus::fdo::Result<()> {
        self.shared.set_shown(id, shown).map_err(zbus::fdo::Error::Failed)
    }

    fn set_icon_size(&self, percent: u32) -> zbus::fdo::Result<()> {
        self.shared.set_icon_size(percent).map_err(zbus::fdo::Error::Failed)
    }

    fn set_hide_healthy(&self, hide: bool) -> zbus::fdo::Result<()> {
        self.shared.set_hide_healthy(hide).map_err(zbus::fdo::Error::Failed)
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, state: &str) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "io.livedot.Daemon1",
    default_service = "io.livedot.Daemon",
    default_path = "/io/livedot/Daemon"
)]
pub trait Livedot {
    fn get_state(&self) -> zbus::Result<String>;
    fn control(&self, id: &str, action: &str) -> zbus::Result<()>;
    fn prompt(&self, id: &str, text: &str) -> zbus::Result<()>;
    fn prompt_button(&self, id: &str, button: &str, text: &str) -> zbus::Result<()>;
    fn item_action(&self, id: &str, item: &str, action: &str) -> zbus::Result<()>;
    fn set_shown(&self, id: &str, shown: bool) -> zbus::Result<()>;
    fn set_icon_size(&self, percent: u32) -> zbus::Result<()>;
    fn set_hide_healthy(&self, hide: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    fn changed(&self, state: String) -> zbus::Result<()>;
}

fn statuses(snapshot: &Snapshot) -> HashMap<String, (Status, String)> {
    snapshot.flat().into_iter().map(|view| (view.id.clone(), (view.status, view.detail.clone()))).collect()
}

pub fn serve(shared: Arc<Shared>, changes: Receiver<()>) -> Result<(), String> {
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.serve_at(OBJECT_PATH, DaemonInterface { shared: Arc::clone(&shared) }))
        .and_then(|b| b.build())
        .map_err(|e| format!("cannot reach the session bus: {e}"))?;
    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner) | Ok(RequestNameReply::AlreadyOwner) => {}
        Ok(_) | Err(zbus::Error::NameTaken) => {
            return Err(format!("another livedot daemon already owns {BUS_NAME}"));
        }
        Err(error) => return Err(format!("cannot own {BUS_NAME} on the session bus: {error}")),
    }
    shared.start();
    let interface = connection
        .object_server()
        .interface::<_, DaemonInterface>(OBJECT_PATH)
        .map_err(|e| format!("cannot reach the served interface: {e}"))?;
    let mut last_json = String::new();
    let mut last_statuses: HashMap<String, (Status, String)> = HashMap::new();
    loop {
        match changes.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => while changes.try_recv().is_ok() {},
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("monitor stopped".into()),
        }
        let snapshot = shared.snapshot();
        let current = statuses(&snapshot);
        for (id, (status, detail)) in &current {
            let previous = last_statuses.get(id).map(|(s, _)| *s);
            if previous != Some(*status) {
                eprintln!(
                    "livedot: {id}: {} -> {} ({detail})",
                    previous.map(Status::word).unwrap_or("unknown"),
                    status.word()
                );
            }
        }
        last_statuses = current;
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        if json != last_json {
            match zbus::block_on(DaemonInterface::changed(interface.signal_emitter(), &json)) {
                Ok(()) => {}
                Err(error @ zbus::Error::InputOutput(_)) => {
                    return Err(format!("lost the session bus: {error}"));
                }
                Err(error) => eprintln!("livedot: cannot emit Changed: {error}"),
            }
            last_json = json;
        }
    }
}

pub fn client() -> Result<LivedotProxyBlocking<'static>, String> {
    let connection =
        zbus::blocking::Connection::session().map_err(|e| format!("cannot reach the session bus: {e}"))?;
    LivedotProxyBlocking::new(&connection).map_err(|e| format!("cannot build the daemon proxy: {e}"))
}

pub fn fetch(proxy: &LivedotProxyBlocking<'_>) -> Result<Snapshot, String> {
    let json = proxy.get_state().map_err(|error| match &error {
        zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" => {
            "the livedot daemon is not running".to_string()
        }
        _ => format!("the livedot daemon is not answering: {error}"),
    })?;
    serde_json::from_str(&json).map_err(|e| format!("unreadable daemon state: {e}"))
}
