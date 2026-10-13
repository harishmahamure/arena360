//! Windows shell launch semantics, matching an executable opened from Explorer.

#[cfg(any(windows, test))]
fn windows_command_line(args: &[String]) -> Result<String, String> {
    let mut quoted = Vec::with_capacity(args.len());
    for arg in args {
        if arg.contains('\0') {
            return Err("Launch arguments contain a NUL character".into());
        }
        // Windows argv parsing doubles backslashes before a quote and before
        // the closing quote, while leaving ordinary path separators intact.
        let mut output = String::from("\"");
        let mut slashes = 0;
        for ch in arg.chars() {
            if ch == '\\' {
                slashes += 1;
                continue;
            }
            output.extend(std::iter::repeat_n(
                '\\',
                if ch == '"' { slashes * 2 + 1 } else { slashes },
            ));
            output.push(ch);
            slashes = 0;
        }
        output.extend(std::iter::repeat_n('\\', slashes * 2));
        output.push('"');
        quoted.push(output);
    }
    Ok(quoted.join(" "))
}

#[cfg(windows)]
pub fn launch(path: &str, args: Option<&[String]>) -> Result<u32, String> {
    if path.contains('\0') {
        return Err("Executable path contains a NUL character".into());
    }
    // COM shell extensions require STA. Use a dedicated thread so the caller's
    // COM apartment and the kiosk's UI message loop are unaffected.
    let parameters = windows_command_line(args.unwrap_or_default())?;
    std::thread::scope(|scope| {
        scope
            .spawn(|| launch_on_shell_thread(path, &parameters))
            .join()
            .map_err(|_| "Windows shell launch thread failed".to_string())?
    })
}

#[cfg(windows)]
fn launch_on_shell_thread(path: &str, parameters: &str) -> Result<u32, String> {
    use std::path::Path;
    use std::time::Duration;
    use sysinfo::{ProcessesToUpdate, System};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
        SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    let _apartment = ComApartment;
    let wide = |value: &str| value.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let file = wide(path);
    let parameters = wide(parameters);
    let directory = wide(
        &Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy(),
    );
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // Shell execution honors Windows compatibility/elevation settings. Security
    // prompts remain enabled; cancellation is returned as a normal launch error.
    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| format!("Windows shell launch: {e}"))?;
    }
    if !info.hProcess.is_invalid() {
        let pid = unsafe { GetProcessId(info.hProcess) };
        unsafe {
            let _ = CloseHandle(info.hProcess);
        }
        if pid != 0 {
            return Ok(pid);
        }
    }
    // DDE/single-instance activation can succeed without creating a new process.
    // Adopt the exact executable in our Windows session for tracking/cleanup.
    let mut system = System::new();
    for _ in 0..20 {
        system.refresh_processes(ProcessesToUpdate::All, true);
        if let Some(pid) = matching_process_pids(&system, path).into_iter().next() {
            return Ok(pid);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Windows opened the application but did not return a trackable process".into())
}

#[cfg(windows)]
pub fn matching_process_pids(system: &sysinfo::System, executable_path: &str) -> Vec<u32> {
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(std::process::id(), &mut session) }.is_err() {
        return Vec::new();
    }
    let expected_name = std::path::Path::new(executable_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase());
    let mut pids: Vec<u32> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let path = process.exe()?;
            // Most running processes cannot match. Avoid filesystem
            // canonicalization for those on every tracking refresh.
            let name = path.file_name()?.to_string_lossy().to_lowercase();
            if expected_name.as_deref() != Some(name.as_str()) {
                return None;
            }
            if !crate::process::paths_match(&path.to_string_lossy(), executable_path) {
                return None;
            }
            let mut process_session = 0;
            if unsafe { ProcessIdToSessionId(pid.as_u32(), &mut process_session) }.is_err()
                || process_session != session
            {
                return None;
            }
            Some(pid.as_u32())
        })
        .collect();
    pids.sort_unstable();
    pids
}

#[cfg(test)]
mod tests {
    use super::windows_command_line;

    #[test]
    fn shell_arguments_preserve_empty_values_spaces_quotes_and_trailing_slashes() {
        let args = [
            "",
            "plain",
            "two words",
            "quote\"here",
            "C:\\Games\\",
            "--exec=launch D3",
        ]
        .map(str::to_string);
        assert_eq!(
            windows_command_line(&args).unwrap(),
            r#""" "plain" "two words" "quote\"here" "C:\Games\\" "--exec=launch D3""#
        );
        assert_eq!(windows_command_line(&[]).unwrap(), "");
    }

    #[cfg(windows)]
    #[test]
    fn shell_arguments_round_trip_through_windows_parser() {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        use windows::Win32::UI::Shell::CommandLineToArgvW;
        let args = [
            "",
            "space here",
            "C:\\Games\\",
            "backslash\\\"quote",
            "--exec=launch D3",
            "தமிழ்",
        ]
        .map(str::to_string);
        let command = format!("app.exe {}", windows_command_line(&args).unwrap());
        let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let mut count = 0;
            let argv = CommandLineToArgvW(PCWSTR(wide.as_ptr()), &mut count);
            assert!(!argv.is_null());
            let parsed: Vec<String> = std::slice::from_raw_parts(argv, count as usize)
                .iter()
                .skip(1)
                .map(|value| value.to_string().unwrap())
                .collect();
            let _ = LocalFree(HLOCAL(argv as *mut _));
            assert_eq!(parsed, args);
        }
    }

    #[test]
    fn shell_arguments_reject_nul() {
        assert!(windows_command_line(&["bad\0argument".into()]).is_err());
    }
}
