//! SEC-006: least-privilege permissions for Ark-owned files and directories.
//!
//! Unix modes are explicit (`0700` directories, `0600` files). Windows does not have Unix
//! mode bits, so Ark installs a protected DACL containing one full-control ACE for the current
//! process user. Unsupported targets retain their platform defaults and are documented as such.

use crate::errors::AppError;
use std::path::Path;

pub fn harden_directory(path: &Path) -> Result<(), AppError> {
    harden(path, true)
}

pub fn harden_file(path: &Path) -> Result<(), AppError> {
    harden(path, false)
}

#[cfg(unix)]
fn harden(path: &Path, directory: bool) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if directory { 0o700 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|error| {
        AppError::new(
            "workspace_permissions_failed",
            format!(
                "Could not restrict permissions on {}: {error}",
                path.display()
            ),
        )
    })
}

#[cfg(windows)]
fn harden(path: &Path, directory: bool) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, GRANT_ACCESS, SE_FILE_OBJECT,
        TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION,
        OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            // SAFETY: this guard is constructed only from a successful `OpenProcessToken`.
            unsafe { CloseHandle(self.0) };
        }
    }

    let mut token = null_mut();
    // SAFETY: output points to a valid HANDLE slot and the pseudo process handle is valid.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(permission_error(path, std::io::Error::last_os_error()));
    }
    let token = HandleGuard(token);

    let mut required = 0u32;
    // SAFETY: the documented sizing call uses a null output buffer and zero size.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut required) };
    if required == 0 {
        return Err(permission_error(path, std::io::Error::last_os_error()));
    }
    let mut token_user_buffer = vec![0u8; required as usize];
    // SAFETY: the buffer has exactly the size returned by the preceding sizing call.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            token_user_buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    } == 0
    {
        return Err(permission_error(path, std::io::Error::last_os_error()));
    }
    // SAFETY: a successful TokenUser query returns a TOKEN_USER at the buffer start; its SID
    // remains valid until after SetEntriesInAclW returns because the buffer stays in scope.
    let user = unsafe { &*(token_user_buffer.as_ptr().cast::<TOKEN_USER>()) };
    let inheritance = if directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0x1000_0000, // GENERIC_ALL
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: user.User.Sid.cast(),
        },
    };
    let mut acl = null_mut();
    // SAFETY: `entry` and the SID it points to remain live for the call; the returned ACL is
    // owned by LocalAlloc and released through LocalFree below.
    let acl_status = unsafe { SetEntriesInAclW(1, &entry, null(), &mut acl) };
    if acl_status != ERROR_SUCCESS {
        return Err(permission_error(
            path,
            std::io::Error::from_raw_os_error(acl_status as i32),
        ));
    }

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL terminated; `acl` is a valid ACL returned above. The protected-DACL
    // flag removes inherited entries, leaving exactly the explicit current-user ACE.
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide.as_mut_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null(),
        )
    };
    // SAFETY: `acl` was allocated by SetEntriesInAclW and is released exactly once.
    unsafe { LocalFree(acl.cast()) };
    if status != ERROR_SUCCESS {
        return Err(permission_error(
            path,
            std::io::Error::from_raw_os_error(status as i32),
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn harden(_path: &Path, _directory: bool) -> Result<(), AppError> {
    Ok(())
}

#[cfg(windows)]
fn permission_error(path: &Path, error: std::io::Error) -> AppError {
    AppError::new(
        "workspace_permissions_failed",
        format!(
            "Could not restrict permissions on {}: {error}",
            path.display()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardens_new_directory_and_file_for_current_user() {
        let root = std::env::temp_dir().join(format!("ark-permissions-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("create root");
        let file = root.join("private.txt");
        std::fs::write(&file, "private").expect("create file");
        harden_directory(&root).expect("harden directory");
        harden_file(&file).expect("harden file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        std::fs::remove_file(file).expect("remove file");
        std::fs::remove_dir(root).expect("remove root");
    }
}
