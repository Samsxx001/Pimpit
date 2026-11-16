use std::{fs, os::windows::process::CommandExt, process::Command};

pub const VERSION_CODE: u32 = version_code(env!("CARGO_PKG_VERSION"));
const fn version_code(version: &str) -> u32 {
    let bytes = version.as_bytes();
    let mut code = 0;
    let mut part = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.' {
            code = code * 100 + part;
            part = 0;
        } else {
            part = part * 10 + (bytes[i] - b'0') as u32;
        }
        i += 1;
    }
    code * 100 + part
}

// This helper exists only while checking/installing an update. No idle polling,
// HTTP client, or extra resident thread is needed by the keyboard process.
pub fn launch(repository: &str) -> Result<(), String> {
    if !valid_repository(repository) {
        return Err("ยังไม่ได้ตั้งค่า GitHub สำหรับอัปเดต\nใส่ update_repo=ชื่อผู้ใช้/ชื่อโปรเจกต์ ใน pimpit.ini หรือใช้รุ่นที่เผยแพร่ผ่าน GitHub Releases".into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("Pimpit-update-{}-{stamp}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let script = directory.join("Update.ps1");
    let content = format!(
        "\u{feff}{}",
        include_str!("../tools/Update.ps1").trim_start_matches('\u{feff}')
    );
    fs::write(&script, content).map_err(|e| e.to_string())?;
    let powershell = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .ok_or("SystemRoot is not set")?
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let result = Command::new(powershell)
        .env_remove("PSModulePath")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-Repository")
        .arg(repository)
        .arg("-CurrentVersion")
        .arg(env!("CARGO_PKG_VERSION"))
        .arg("-Executable")
        .arg(executable)
        .arg("-ParentProcessId")
        .arg(std::process::id().to_string())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(script);
            let _ = fs::remove_dir(directory);
            Err(error.to_string())
        }
    }
}

fn valid_repository(value: &str) -> bool {
    let parts: Vec<_> = value.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && *part != "."
                && *part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn repository_is_a_slug_not_an_arbitrary_url_or_command() {
        for good in ["owner/Pimpit", "user.name/pimpit"] {
            assert!(super::valid_repository(good));
        }
        for bad in [
            "",
            "owner",
            "../repo",
            "o/r/extra",
            "https://github.com/o/r",
            "o/r;calc",
            "o/r\n",
        ] {
            assert!(!super::valid_repository(bad));
        }
    }
}
