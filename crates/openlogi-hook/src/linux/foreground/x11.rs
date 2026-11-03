//! The X11 frontmost source: `_NET_ACTIVE_WINDOW` on the root window, then
//! the focused window's `WM_CLASS`. It is the whole answer on an X11 session
//! and the XWayland fallback on every other.
//!
//! Either value changes the answer. The root announces a focus change, but only
//! the window itself announces a `WM_CLASS` rewrite, so the observer also
//! selects `PropertyChange` on the focused window. LibreOffice's `gen` backend
//! maps its frame with a provisional class and later sets the module class
//! (`libreoffice-calc`, …) on that same focused window (#2078).

use std::os::fd::AsRawFd;
use std::time::Instant;

use tracing::debug;
use x11rb::connection::Connection as _;
use x11rb::properties::WmClass;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, PropertyNotifyEvent,
    Window,
};
use x11rb::rust_connection::RustConnection;

use super::{
    FrontmostSource, PollResult, PublishAppId, RECONNECT_DELAY, StopToken, poll_source_or_stop,
    wine,
};

#[cfg(test)]
mod tests;

/// The WM_CLASS class component is the shared foreground/pointer profile ID,
/// except for a Wine window whose class only names its launcher's shared prefix:
/// that one is identified by its executable (see [`wine`]).
pub(in crate::linux) fn window_app_id(conn: &RustConnection, window: Window) -> Option<String> {
    let class = window_class(conn, window)?;
    Some(wine_executable(conn, window, &class).unwrap_or(class))
}

fn window_class(conn: &RustConnection, window: Window) -> Option<String> {
    let wm = WmClass::get(conn, window).ok()?.reply_unchecked().ok()??;
    std::str::from_utf8(wm.class())
        .ok()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The executable behind a window of a generic Wine `class`, from the window's
/// `_NET_WM_PID`.
fn wine_executable(conn: &RustConnection, window: Window, class: &str) -> Option<String> {
    if !wine::is_generic_class(class) {
        return None;
    }
    let net_wm_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let pid = conn
        .get_property(false, window, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()?;
    wine::executable_of(pid)
}

/// The identifier of the XWayland window a Wayland-native backend reported only
/// by its generic Wine `class`, which carries no pid to look up.
///
/// Every client of that class is resolved; when they are one executable the
/// answer is unambiguous, and when they differ the focused window decides.
/// `None` leaves the reported class in place.
pub(super) fn xwayland_wine_app_id(class: &str) -> Option<String> {
    if !wine::is_generic_class(class) {
        return None;
    }
    X11Source::connect()?.generic_wine_window_id()
}

/// The identifier of the XWayland window of generic Wine `class` whose title is
/// `title`, for a Wayland-native backend that reports the focused toplevel's
/// title next to its class.
///
/// `_NET_ACTIVE_WINDOW` only follows X11 clients: right after focus moves
/// between native windows and Wine windows it still names the previous one, so
/// with several Wine programs open (a launcher, the game, a desktop window) the
/// focused toplevel's own title is the dependable pointer. `None` when the
/// title is empty or does not single out one executable.
pub(super) fn xwayland_wine_app_id_for_title(class: &str, title: &str) -> Option<String> {
    if !wine::is_generic_class(class) || title.is_empty() {
        return None;
    }
    X11Source::connect()?.generic_wine_window_id_for_title(title)
}

/// The identifier of the one executable behind the `candidates` (identifier and
/// title per window) whose title is `title`.
fn id_for_title(candidates: &[(String, String)], title: &str) -> Option<String> {
    let mut matching = candidates
        .iter()
        .filter(|(_, candidate)| candidate == title)
        .map(|(id, _)| id);
    let first = matching.next()?;
    matching.all(|id| id == first).then(|| first.clone())
}

/// What one `PropertyNotify` means for the published application id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyChange {
    /// The root's `_NET_ACTIVE_WINDOW` changed: another window may have focus.
    Focus,
    /// The focused window rewrote its own `WM_CLASS` and kept focus.
    Class,
    /// Anything else: a title change on the focused window, a class change on
    /// a window that has lost focus, any other root property.
    Unrelated,
}

impl PropertyChange {
    fn of(
        event: &PropertyNotifyEvent,
        root: Window,
        net_active_window: Atom,
        focused: Option<Window>,
    ) -> Self {
        if event.window == root && event.atom == net_active_window {
            Self::Focus
        } else if Some(event.window) == focused && event.atom == Atom::from(AtomEnum::WM_CLASS) {
            Self::Class
        } else {
            Self::Unrelated
        }
    }
}

/// Frontmost backend backed by X11 `_NET_ACTIVE_WINDOW` + `WM_CLASS`.
///
/// Works on an X11 session, and on a Wayland session for XWayland windows;
/// native Wayland windows are invisible through this path and yield `None`.
pub(in crate::linux) struct X11Source {
    pub(in crate::linux) conn: RustConnection,
    pub(in crate::linux) root: Window,
    net_active_window: Atom,
}

impl X11Source {
    /// Connect to the X server and resolve the `_NET_ACTIVE_WINDOW` atom.
    /// Returns `None` when no X display is reachable (a Wayland session without
    /// XWayland, or `$DISPLAY` unset).
    pub(in crate::linux) fn connect() -> Option<Self> {
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| debug!("X11 not available, frontmost will return None: {e}"))
            .ok()?;
        let root = conn.setup().roots[screen_num].root;
        Self::on_connection(conn, root)
    }

    /// Resolve the `_NET_ACTIVE_WINDOW` atom on an open connection whose
    /// screen root is `root`.
    fn on_connection(conn: RustConnection, root: Window) -> Option<Self> {
        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        Some(Self {
            conn,
            root,
            net_active_window,
        })
    }

    fn subscribe(&self) -> bool {
        self.set_event_mask(EventMask::PROPERTY_CHANGE)
            .map_err(|e| debug!("frontmost: failed to subscribe to X11 root changes: {e}"))
            .is_ok()
    }

    fn unsubscribe(&self) {
        if let Err(error) = self.set_event_mask(EventMask::NO_EVENT) {
            debug!("frontmost: failed to unsubscribe from X11 root changes: {error}");
        }
    }

    fn set_event_mask(&self, event_mask: EventMask) -> Result<(), String> {
        let cookie = self
            .conn
            .change_window_attributes(
                self.root,
                &ChangeWindowAttributesAux::new().event_mask(event_mask),
            )
            .map_err(|error| error.to_string())?;
        cookie.check().map_err(|error| error.to_string())?;
        self.conn.flush().map_err(|error| error.to_string())
    }

    /// The window `_NET_ACTIVE_WINDOW` names, or `None` when nothing has focus.
    fn active_window(&self) -> Option<Window> {
        let window: Window = self
            .conn
            .get_property(
                false,
                self.root,
                self.net_active_window,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .next()?;
        // `None` (0) means no focus. The root never carries an application's
        // `WM_CLASS`, and watching it would let `watch` clear the root's own
        // `_NET_ACTIVE_WINDOW` subscription.
        (window != x11rb::NONE && window != self.root).then_some(window)
    }

    fn generic_wine_window_id(&self) -> Option<String> {
        let client_list = self
            .conn
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let windows: Vec<Window> = self
            .conn
            .get_property(
                false,
                self.root,
                client_list,
                AtomEnum::WINDOW,
                0,
                u32::MAX / 4,
            )
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .collect();
        let resolved: Vec<(Window, String)> = windows
            .into_iter()
            .filter(|window| {
                window_class(&self.conn, *window)
                    .is_some_and(|class| wine::is_generic_class(&class))
            })
            .filter_map(|window| Some((window, window_app_id(&self.conn, window)?)))
            .filter(|(_, id)| !wine::is_generic_class(id))
            .collect();
        let (_, first) = resolved.first()?;
        if resolved.iter().all(|(_, id)| id == first) {
            return Some(first.clone());
        }
        let active = self.active_window()?;
        resolved
            .into_iter()
            .find_map(|(window, id)| (window == active).then_some(id))
    }

    /// The identifier of the generic-class Wine window titled `title`, see
    /// [`xwayland_wine_app_id_for_title`].
    fn generic_wine_window_id_for_title(&self, title: &str) -> Option<String> {
        let client_list = self
            .conn
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let windows: Vec<Window> = self
            .conn
            .get_property(
                false,
                self.root,
                client_list,
                AtomEnum::WINDOW,
                0,
                u32::MAX / 4,
            )
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .collect();
        let candidates: Vec<(String, String)> = windows
            .into_iter()
            .filter(|window| {
                window_class(&self.conn, *window)
                    .is_some_and(|class| wine::is_generic_class(&class))
            })
            .filter_map(|window| {
                let id = window_app_id(&self.conn, window)?;
                let name = self.window_title(window)?;
                (!wine::is_generic_class(&id)).then_some((id, name))
            })
            .collect();
        id_for_title(&candidates, title)
    }

    /// The window's title: `_NET_WM_NAME` when set, else the legacy `WM_NAME`.
    fn window_title(&self, window: Window) -> Option<String> {
        let read = |property: Atom, kind: Atom| -> Option<String> {
            let reply = self
                .conn
                .get_property(false, window, property, kind, 0, 256)
                .ok()?
                .reply()
                .ok()?;
            String::from_utf8(reply.value)
                .ok()
                .filter(|s| !s.is_empty())
        };
        let net_wm_name = self
            .conn
            .intern_atom(false, b"_NET_WM_NAME")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let utf8_string = self
            .conn
            .intern_atom(false, b"UTF8_STRING")
            .ok()?
            .reply()
            .ok()?
            .atom;
        read(net_wm_name, utf8_string)
            .or_else(|| read(AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
    }

    /// Move the `WM_CLASS` watch to the focused window, then read its class.
    ///
    /// The selection is requested before the class read on the same
    /// connection, and the server handles one client's requests in order, so a
    /// rewrite after the read queues a `PropertyNotify` instead of going unseen.
    fn refocus(&self, focused: &mut Option<Window>) -> Option<String> {
        let window = self.active_window();
        self.watch(focused, window);
        window.and_then(|window| window_app_id(&self.conn, window))
    }

    /// Move this client's `PropertyChange` selection from `*focused` to `next`.
    ///
    /// Event selections are per client, so this never disturbs the masks the
    /// application or the window manager installed. Errors are discarded: the
    /// window can be destroyed between the focus change and these requests,
    /// and the class read then fails on its own.
    fn watch(&self, focused: &mut Option<Window>, next: Option<Window>) {
        if *focused == next {
            return;
        }
        let selections = [
            (focused.take(), EventMask::NO_EVENT),
            (next, EventMask::PROPERTY_CHANGE),
        ];
        for (window, event_mask) in selections {
            let Some(window) = window else {
                continue;
            };
            match self.conn.change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new().event_mask(event_mask),
            ) {
                Ok(cookie) => cookie.ignore_error(),
                Err(error) => {
                    debug!("frontmost: failed to select property changes on {window:#x}: {error}");
                }
            }
        }
        *focused = next;
        if let Err(error) = self.conn.flush() {
            debug!("frontmost: failed to flush X11 window selections: {error}");
        }
    }

    fn run_until_stopped_or_disconnected(
        &mut self,
        stop: &StopToken,
        publish: &PublishAppId,
    ) -> bool {
        if !self.subscribe() {
            return false;
        }

        // The root event mask is installed before this read, so a focus change
        // can only be represented by the snapshot, a queued PropertyNotify, or
        // both. `focused` is the one window whose own changes are selected.
        let mut focused = None;
        publish(self.refocus(&mut focused));

        loop {
            let mut changed = false;
            loop {
                match self.conn.poll_for_event() {
                    Ok(Some(Event::PropertyNotify(event))) => {
                        changed |=
                            PropertyChange::of(&event, self.root, self.net_active_window, focused)
                                != PropertyChange::Unrelated;
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) => {
                        debug!("frontmost: X11 connection lost: {error}");
                        return false;
                    }
                }
            }
            if changed {
                publish(self.refocus(&mut focused));
            }

            // Every refocus is a round trip, and x11rb queues the events that
            // arrive with a reply without leaving the socket readable. After
            // one, only check for a stop request and drain again; block on the
            // socket once a drain found nothing to act on.
            let deadline = changed.then(Instant::now);
            match poll_source_or_stop(
                Some(self.conn.stream().as_raw_fd()),
                stop.wake_fd(),
                deadline,
            ) {
                PollResult::StopRequested => {
                    self.watch(&mut focused, None);
                    self.unsubscribe();
                    return true;
                }
                PollResult::SourceReady | PollResult::DeadlineReached => {}
                PollResult::Error => return false,
            }
        }
    }
}

impl FrontmostSource for X11Source {
    fn frontmost_app_id(&mut self) -> Option<String> {
        self.active_window()
            .and_then(|window| window_app_id(&self.conn, window))
    }

    fn observe(
        mut self: Box<Self>,
        stop: StopToken,
        publish: PublishAppId,
    ) -> Box<dyn FrontmostSource> {
        loop {
            if self.run_until_stopped_or_disconnected(&stop, &publish) {
                return self;
            }
            publish(None);
            if stop.wait_timeout(RECONNECT_DELAY) {
                return self;
            }
            if let Some(reconnected) = Self::connect() {
                debug!("frontmost: X11 connection restored");
                *self = reconnected;
            }
        }
    }

    fn name(&self) -> &'static str {
        "x11"
    }
}
