#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

/// Maximum path length for Windows without long path support
pub const MAX_PATH_LENGTH_LEGACY: usize = 260;

/// Maximum path length for Windows with long path support enabled
pub const MAX_PATH_LENGTH_EXTENDED: usize = 32767;

/// Checks if Windows long path support is enabled via registry
/// Returns the appropriate maximum path length
pub fn get_max_path_length() -> usize {
    if is_long_path_enabled() {
        MAX_PATH_LENGTH_EXTENDED
    } else {
        MAX_PATH_LENGTH_LEGACY
    }
}

/// Checks Windows registry to see if long path support is enabled
/// Registry key: HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled
#[cfg(windows)]
fn is_long_path_enabled() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let filesystem_key = match hklm.open_subkey("SYSTEM\\CurrentControlSet\\Control\\FileSystem") {
        Ok(key) => key,
        Err(_) => return false,
    };

    // LongPathsEnabled is a DWORD value: 1 = enabled, 0 or missing = disabled
    match filesystem_key.get_value::<u32, _>("LongPathsEnabled") {
        Ok(value) => value == 1,
        Err(_) => false, // Key doesn't exist or can't be read = disabled
    }
}

/// Fallback for non-Windows platforms
#[cfg(not(windows))]
fn is_long_path_enabled() -> bool {
    false
}

/// Validates a path with consideration for Windows long path support
/// Returns a descriptive error message if the path is too long
pub fn validate_path_with_long_path_support(path: &str) -> Result<(), String> {
    let path_len = path.len();
    
    if path_len <= MAX_PATH_LENGTH_LEGACY {
        // Always OK
        return Ok(());
    }
    
    if path_len <= MAX_PATH_LENGTH_EXTENDED && is_long_path_enabled() {
        // OK if long paths are enabled
        return Ok(());
    }
    
    if is_long_path_enabled() {
        Err(format!(
            "Path exceeds maximum length of {} characters: {} (length: {})",
            MAX_PATH_LENGTH_EXTENDED, path, path_len
        ))
    } else {
        Err(format!(
            "Path exceeds Windows legacy limit of {} characters. Consider enabling long path support or use a shorter path: {} (length: {})",
            MAX_PATH_LENGTH_LEGACY, path, path_len
        ))
    }
}

/// Gets a human-readable description of the current path length limits
pub fn get_path_length_info() -> String {
    if is_long_path_enabled() {
        format!("Windows long path support enabled (max: {} characters)", MAX_PATH_LENGTH_EXTENDED)
    } else {
        format!("Windows legacy path limit (max: {} characters)", MAX_PATH_LENGTH_LEGACY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_length_constants() {
        assert_eq!(MAX_PATH_LENGTH_LEGACY, 260);
        assert_eq!(MAX_PATH_LENGTH_EXTENDED, 32767);
    }

    #[test]
    fn test_short_path_validation() {
        let short_path = "C:\\short\\path";
        assert!(validate_path_with_long_path_support(short_path).is_ok());
    }

    #[test]
    fn test_long_path_validation() {
        // Create a path that exceeds legacy limit but is within extended limit
        let long_path = "C:\\".to_string() + &"a".repeat(300);
        
        // This should either pass (if long paths enabled) or fail with legacy limit
        let result = validate_path_with_long_path_support(&long_path);
        
        if is_long_path_enabled() {
            assert!(result.is_ok(), "Long path should be valid when long path support is enabled");
        } else {
            assert!(result.is_err(), "Long path should be invalid when long path support is disabled");
            if let Err(msg) = result {
                assert!(msg.contains("legacy limit"));
                assert!(msg.contains("260"));
            }
        }
    }

    #[test]
    fn test_extremely_long_path_validation() {
        // Create a path that exceeds even the extended limit
        let extremely_long_path = "C:\\".to_string() + &"a".repeat(33000);
        
        let result = validate_path_with_long_path_support(&extremely_long_path);
        assert!(result.is_err(), "Extremely long path should always be invalid");
        
        if let Err(msg) = result {
            if is_long_path_enabled() {
                assert!(msg.contains("32767"));
            } else {
                assert!(msg.contains("260"));
            }
        }
    }

    #[test]
    fn test_path_length_info() {
        let info = get_path_length_info();
        assert!(!info.is_empty());
        assert!(info.contains("characters"));
    }
} 