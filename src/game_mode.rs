//! Small foreground-game classifier: no process scan, graphics injection or hooks.

pub fn automatic_enabled(requested: bool, game_active: bool) -> bool {
    requested && !game_active
}

pub fn is_game_executable(path: &str, extra_exes: &[String]) -> bool {
    let path = path.replace('/', "\\").to_lowercase();
    let name = path.rsplit('\\').next().unwrap_or("");
    if extra_exes
        .iter()
        .any(|extra| extra.eq_ignore_ascii_case(name))
    {
        return true;
    }
    // Launchers, crash reporters and tools are often installed alongside games.
    if matches!(
        name,
        "steam.exe"
            | "steamwebhelper.exe"
            | "epicgameslauncher.exe"
            | "galaxyclient.exe"
            | "xboxpcapp.exe"
            | "unitycrashhandler64.exe"
            | "unitycrashhandler32.exe"
            | "crashreportclient.exe"
            | "setup.exe"
            | "unins000.exe"
            | "blender.exe"
            | "wallpaper32.exe"
            | "wallpaper64.exe"
    ) {
        return false;
    }
    if matches!(
        name,
        "bomb rush cyberfunk.exe"
            | "undertale.exe"
            | "deltarune.exe"
            | "cs2.exe"
            | "dota2.exe"
            | "hl2.exe"
            | "tf_win64.exe"
            | "leagueoflegends.exe"
            | "valorant-win64-shipping.exe"
            | "fortniteclient-win64-shipping.exe"
            | "r5apex.exe"
            | "robloxplayerbeta.exe"
            | "minecraft.windows.exe"
            | "genshinimpact.exe"
            | "starrail.exe"
            | "zenlesszonezero.exe"
            | "eldenring.exe"
            | "darksoulsiii.exe"
            | "cyberpunk2077.exe"
            | "gta5.exe"
            | "rdr2.exe"
            | "witcher3.exe"
            | "terraria.exe"
            | "stardew valley.exe"
            | "hollow_knight.exe"
            | "hades.exe"
            | "hades2.exe"
    ) {
        return true;
    }
    name.ends_with("-win64-shipping.exe")
        || name.ends_with("-win32-shipping.exe")
        || [
            "\\steamapps\\common\\",
            "\\epic games\\",
            "\\epicgames\\",
            "\\xboxgames\\",
            "\\gog games\\",
            "\\gog galaxy\\games\\",
        ]
        .iter()
        .any(|folder| path.contains(folder))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn games_work_in_windowed_mode_and_portable_folders() {
        for path in [
            r"D:\Portable\Bomb Rush Cyberfunk.exe",
            r"C:\Games\DELTARUNE.EXE",
            r"D:\SteamLibrary\steamapps\common\New Game\game.exe",
            "D:/Epic Games/NewTitle/Binaries/game.exe",
            r"D:\XboxGames\NewTitle\Content\game.exe",
            r"D:\Portable\Title-Win64-Shipping.exe",
        ] {
            assert!(is_game_executable(path, &[]), "{path}");
        }
    }
    #[test]
    fn editors_launchers_and_similarly_named_folders_are_not_games() {
        for path in [
            r"C:\Windows\notepad.exe",
            r"C:\Apps\ChatGPT.exe",
            r"C:\Chrome\chrome.exe",
            r"D:\Steam\steam.exe",
            r"D:\Steam\steamapps\common\Blender\blender.exe",
            r"D:\Games\UnityCrashHandler64.exe",
            r"D:\steamapps\commonplace\editor.exe",
            r"D:\Epic Games Tools\editor.exe",
        ] {
            assert!(!is_game_executable(path, &[]), "{path}");
        }
    }
    #[test]
    fn custom_names_are_exact_and_case_insensitive() {
        let extra = vec!["MyGame.exe".into()];
        assert!(is_game_executable(r"D:\Portable\mygame.EXE", &extra));
        assert!(!is_game_executable(
            r"D:\Portable\mygame-helper.exe",
            &extra
        ));
    }
    #[test]
    fn leaving_game_restores_the_user_setting_instead_of_turning_auto_on() {
        for requested in [true, false] {
            assert_eq!(automatic_enabled(requested, false), requested);
            assert!(!automatic_enabled(requested, true));
            assert_eq!(automatic_enabled(requested, false), requested);
        }
    }
}
