//! Windows default-handler registration for Markdown files.
//!
//! Mirrors lmdv's v0.8.0 plan: write the per-user ProgID, `.md` OpenWith and
//! Default Apps entries, then open the Default Apps page so the user confirms.
//! `UserChoice` is never touched.

use std::io;
use std::path::Path;

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

pub const PROG_ID: &str = "lmdv-tui.md";
pub const REGISTERED_NAME: &str = "lmdv-tui";

fn command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

pub fn is_registered() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!(r"Software\Classes\{PROG_ID}\shell\open\command"))
        .and_then(|key| key.get_value::<String, _>(""))
        .is_ok()
}

pub fn register() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let command = command(&exe);
    let icon = format!("{},0", exe.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (prog, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}"))?;
    prog.set_value("", &"Markdown document")?;
    prog.set_value("FriendlyTypeName", &"Markdown document")?;
    let (icon_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}\DefaultIcon"))?;
    icon_key.set_value("", &icon)?;
    let (open, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}\shell\open\command"))?;
    open.set_value("", &command)?;
    let (edit, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}\shell\edit\command"))?;
    edit.set_value("", &command)?;

    let (md, _) = hkcu.create_subkey(r"Software\Classes\.md")?;
    md.set_value("", &PROG_ID)?;
    md.set_value("Content Type", &"text/markdown")?;
    let (open_with, _) = hkcu.create_subkey(r"Software\Classes\.md\OpenWithProgids")?;
    open_with.set_value(PROG_ID, &"")?;

    let (app, _) = hkcu.create_subkey(r"Software\Classes\Applications\lmdv-tui.exe")?;
    app.set_value("FriendlyAppName", &"lmdv-tui")?;
    let (app_open, _) = hkcu.create_subkey(r"Software\Classes\Applications\lmdv-tui.exe\shell\open\command")?;
    app_open.set_value("", &command)?;
    let (types, _) = hkcu.create_subkey(r"Software\Classes\Applications\lmdv-tui.exe\SupportedTypes")?;
    types.set_value(".md", &"")?;

    let (registered, _) = hkcu.create_subkey(r"Software\RegisteredApplications")?;
    registered.set_value(REGISTERED_NAME, &r"Software\lmdv-tui\Capabilities")?;
    let (capabilities, _) = hkcu.create_subkey(r"Software\lmdv-tui\Capabilities")?;
    capabilities.set_value("ApplicationName", &"lmdv-tui")?;
    capabilities.set_value("ApplicationDescription", &"Markdown reader in the terminal")?;
    capabilities.set_value("ApplicationIcon", &icon)?;
    let (files, _) = hkcu.create_subkey(r"Software\lmdv-tui\Capabilities\FileAssociations")?;
    files.set_value(".md", &PROG_ID)?;
    Ok(())
}

pub fn open_default_apps() -> io::Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        let target: Vec<u16> = "ms-settings:defaultapps?registeredAppUser=lmdv-tui"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize > 32 {
            Ok(())
        } else {
            Err(io::Error::other("could not open Default Apps"))
        }
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::other("Default Apps is Windows-only"))
    }
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn command_quotes_the_exe_and_placeholder() {
        assert_eq!(
            command(std::path::Path::new(r"C:\Apps\lmdv-tui.exe")),
            r#""C:\Apps\lmdv-tui.exe" "%1""#
        );
    }
}
