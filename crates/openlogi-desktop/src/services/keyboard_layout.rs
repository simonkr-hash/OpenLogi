//! The keyboard layout the user types with, for recording and showing
//! custom shortcuts. See `openlogi_core::binding::KeyLayout` for why the
//! settings app needs it and the agent does not.

#[cfg(target_os = "linux")]
mod linux;

/// Read the layout once, at startup, and install it for the whole process.
/// Where none can be read, shortcuts keep their US key names.
pub(crate) fn install() {
    #[cfg(target_os = "linux")]
    linux::install();
}
