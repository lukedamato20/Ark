//! Windows process-window policy for Ark-owned helpers. Release and debug GUI runs capture
//! stdout/stderr through Ark diagnostics instead of flashing a console window. Development's
//! `dev.bat` remains a visible shell and is intentionally outside this policy.

#[cfg(windows)]
pub fn hide_std_process_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_std_process_window(_command: &mut std::process::Command) {}

pub fn hide_tokio_process_window(command: &mut tokio::process::Command) {
    hide_std_process_window(command.as_std_mut());
}

#[cfg(windows)]
pub fn isolate_std_process_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}
