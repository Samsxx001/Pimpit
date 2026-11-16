use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=data/dictionaries.b64");
    println!("cargo:rerun-if-changed=tools/Restore-Dictionaries.ps1");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    #[cfg(windows)]
    let status = {
        use std::os::windows::process::CommandExt;
        let powershell = PathBuf::from(env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        Command::new(powershell)
            .env_remove("PSModulePath")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(root.join("tools/Restore-Dictionaries.ps1"))
            .arg("-Destination")
            .arg(&output)
            .creation_flags(0x08000000)
            .status()
    };
    #[cfg(not(windows))]
    let status = {
        let decoded = Command::new("base64")
            .arg("--decode")
            .arg(root.join("data/dictionaries.b64"))
            .output()
            .expect("base64 is required");
        assert!(decoded.status.success(), "Invalid dictionary archive");
        let archive = output.join("dictionaries.zip");
        std::fs::write(&archive, decoded.stdout).expect("Cannot write dictionary archive");
        Command::new("unzip")
            .arg("-o")
            .arg(archive)
            .args(["english.txt", "thai.txt", "-d"])
            .arg(&output)
            .status()
    };
    assert!(
        status.expect("Cannot unpack build dictionaries").success(),
        "Dictionary extraction failed"
    );
    println!("cargo:rustc-env=PIMPIT_DICT_DIR={}", output.display());
}
