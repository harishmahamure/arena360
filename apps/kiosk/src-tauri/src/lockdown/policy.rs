//! Per-user Windows policies complement the hook, including Task Manager
//! launched from the secure Ctrl+Alt+Delete screen. Save the original values
//! before changing them so staff setup, exit, and recovery preserve IT policy.

#[cfg(windows)]
mod win {
    use serde::{Deserialize, Serialize};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY,
        REG_DWORD, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE,
    };

    const POLICIES: [(&str, &str); 2] = [
        (
            "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
            "DisableTaskMgr",
        ),
        (
            "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer",
            "NoWinKeys",
        ),
    ];
    const BACKUP: (&str, &str) = ("Software\\Arena360\\Lockdown", "OriginalPolicies");

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Value {
        kind: u32,
        bytes: Vec<u8>,
    }

    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    fn read(path: &str, name: &str) -> Result<Option<Value>, String> {
        let path = wide(path);
        let name = wide(name);
        unsafe {
            let mut key = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if result == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            result.ok().map_err(|e| e.to_string())?;
            let key = Key(key);
            let mut kind = REG_VALUE_TYPE::default();
            let mut size = 0;
            let result = RegQueryValueExW(
                key.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            );
            if result == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            result.ok().map_err(|e| e.to_string())?;
            let mut bytes = vec![0; size as usize];
            RegQueryValueExW(
                key.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&mut size),
            )
            .ok()
            .map_err(|e| e.to_string())?;
            bytes.truncate(size as usize);
            Ok(Some(Value {
                kind: kind.0,
                bytes,
            }))
        }
    }

    fn write(path: &str, name: &str, value: Option<&Value>) -> Result<(), String> {
        let path = wide(path);
        let name = wide(name);
        unsafe {
            let mut key = HKEY::default();
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
            .ok()
            .map_err(|e| e.to_string())?;
            let key = Key(key);
            let result = if let Some(value) = value {
                RegSetValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    0,
                    REG_VALUE_TYPE(value.kind),
                    Some(&value.bytes),
                )
            } else {
                RegDeleteValueW(key.0, PCWSTR(name.as_ptr()))
            };
            if value.is_none() && result == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            result.ok().map_err(|e| e.to_string())
        }
    }

    // Fixed policy locations are never read from the snapshot. A crash/relaunch
    // reuses the first snapshot rather than saving our own restrictions as IT's.
    fn apply(policies: &[(&str, &str); 2], backup: (&str, &str)) -> Result<(), String> {
        if read(backup.0, backup.1)?.is_none() {
            let original = [
                read(policies[0].0, policies[0].1)?,
                read(policies[1].0, policies[1].1)?,
            ];
            let bytes = serde_json::to_vec(&original).map_err(|e| e.to_string())?;
            write(
                backup.0,
                backup.1,
                Some(&Value {
                    kind: REG_BINARY.0,
                    bytes,
                }),
            )?;
        } else {
            // Validate the existing backup before modifying any policy.
            original_values(backup)?;
        }
        let enabled = Value {
            kind: REG_DWORD.0,
            bytes: 1u32.to_le_bytes().to_vec(),
        };
        for (path, name) in policies {
            write(path, name, Some(&enabled)).map_err(|e| format!("{name}: {e}"))?;
        }
        Ok(())
    }

    fn original_values(backup: (&str, &str)) -> Result<Option<[Option<Value>; 2]>, String> {
        read(backup.0, backup.1)?
            .map(|value| {
                if value.kind != REG_BINARY.0 {
                    return Err("Invalid lockdown policy backup type".into());
                }
                serde_json::from_slice(&value.bytes).map_err(|e| e.to_string())
            })
            .transpose()
    }

    fn restore(policies: &[(&str, &str); 2], backup: (&str, &str)) -> Result<(), String> {
        let Some(original) = original_values(backup)? else {
            return Ok(());
        };
        for ((path, name), value) in policies.iter().zip(original.iter()) {
            // Policy application may have been denied by IT ACLs. Skip values
            // we never changed, so that failure cannot prevent staff setup.
            if read(path, name)?.as_ref() != value.as_ref() {
                write(path, name, value.as_ref()).map_err(|e| format!("restore {name}: {e}"))?;
            }
        }
        // Retain the snapshot until every value has been restored successfully.
        write(backup.0, backup.1, None)
    }

    pub fn lock() -> Result<(), String> {
        apply(&POLICIES, BACKUP)
    }
    pub fn unlock() -> Result<(), String> {
        restore(&POLICIES, BACKUP)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn relaunch_restores_original_values_and_absent_values() {
            // Isolated HKCU test key: never modify the actual Windows policies.
            let root = format!("Software\\Arena360\\PolicyTest-{}", std::process::id());
            let backup_path = format!("{root}\\Backup");
            let policies = [(root.as_str(), "Existing"), (root.as_str(), "Absent")];
            let backup = (backup_path.as_str(), "Snapshot");
            let original = Value {
                kind: REG_DWORD.0,
                bytes: 0u32.to_le_bytes().to_vec(),
            };
            write(&root, "Existing", Some(&original)).unwrap();
            write(&root, "Absent", None).unwrap();
            apply(&policies, backup).unwrap();
            assert_eq!(
                read(&root, "Existing").unwrap().unwrap().bytes,
                1u32.to_le_bytes()
            );
            assert!(read(&root, "Absent").unwrap().is_some());
            apply(&policies, backup).unwrap(); // Simulate recovery after abrupt exit.
            restore(&policies, backup).unwrap();
            assert_eq!(read(&root, "Existing").unwrap(), Some(original));
            assert_eq!(read(&root, "Absent").unwrap(), None);
            assert_eq!(read(backup.0, backup.1).unwrap(), None);
            restore(&policies, backup).unwrap(); // Relaxation/exit is idempotent.
            write(&root, "Existing", None).unwrap();
        }
    }
}

pub fn apply_locked() -> Result<(), String> {
    #[cfg(windows)]
    {
        win::lock()
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

pub fn restore() -> Result<(), String> {
    #[cfg(windows)]
    {
        win::unlock()
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}
