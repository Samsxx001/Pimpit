#![allow(unsafe_op_in_unsafe_fn)]
use rightlang_rust::game_mode::{automatic_enabled, is_game_executable};
use rightlang_rust::{Language, automatic, automatic_live, convert};
use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    System::{LibraryLoader::*, Memory::*, RemoteDesktop::*, SystemInformation::*, Threading::*},
    UI::{Input::KeyboardAndMouse::*, Input::*, Shell::*, WindowsAndMessaging::*},
};

const TRAY: u32 = WM_APP + 1;
const STATUS: u32 = WM_APP + 50;
const TAG: usize = 0x524c5253;
const PAUSE_ID: i32 = 1;
const MANUAL_ID: i32 = 2;
const UNDO_ID: i32 = 3;
const REPAIR_ID: i32 = 4;
static TX: OnceLock<SyncSender<Event>> = OnceLock::new();
static PAUSED: AtomicBool = AtomicBool::new(false);
static AUTO: AtomicBool = AtomicBool::new(true);
static GAME_ACTIVE: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
static RAW_OK: AtomicBool = AtomicBool::new(false);
static APP_WINDOW: AtomicUsize = AtomicUsize::new(0);
static RAW_EVENTS: AtomicU64 = AtomicU64::new(0);
static MANUAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static LAST_RESULT: AtomicU32 = AtomicU32::new(0);
static REGISTERED: AtomicU32 = AtomicU32::new(0);
static LAST_KEY_TIME: AtomicU32 = AtomicU32::new(0);
const SHIFT_MANUAL_ID: i32 = 5;
static RAW_SEEN: AtomicU64 = AtomicU64::new(0);
static EPOCH: AtomicU64 = AtomicU64::new(1);
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static REPAIRS: AtomicU64 = AtomicU64::new(0);
static CONVERSIONS: AtomicU64 = AtomicU64::new(0);
static AUTO_CONVERSIONS: AtomicU64 = AtomicU64::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);
static OVERFLOWS: AtomicU64 = AtomicU64::new(0);
static HOTKEY_ERRORS: AtomicU32 = AtomicU32::new(0);
static SETTINGS: OnceLock<Settings> = OnceLock::new();
static STATUS_VIEW: AtomicUsize = AtomicUsize::new(0);
static LAST_REFRESH: AtomicU64 = AtomicU64::new(0);
static REGISTRATION_CALLS: AtomicU64 = AtomicU64::new(0);
static DEVICE_EVENTS: AtomicU64 = AtomicU64::new(0);

unsafe fn startup_stage(stage: u32) {
    let pointer = STATUS_VIEW.load(Acquire) as *const AtomicU32;
    if !pointer.is_null() {
        (*pointer.add(10)).store(stage, Release);
    }
}

unsafe fn publish_status() {
    let pointer = STATUS_VIEW.load(Acquire) as *const AtomicU32;
    if pointer.is_null() {
        return;
    }
    let view = std::slice::from_raw_parts(pointer, 16);
    for (index, value) in [
        crate::updater::VERSION_CODE,
        GetCurrentProcessId(),
        RAW_EVENTS.load(Relaxed) as u32,
        MANUAL_REQUESTS.load(Relaxed) as u32,
        CONVERSIONS.load(Relaxed) as u32,
        LAST_RESULT.load(Relaxed),
        REGISTERED.load(Relaxed),
        RAW_OK.load(Relaxed) as u32,
        FAILURES.load(Relaxed) as u32,
        GetTickCount64() as u32,
    ]
    .into_iter()
    .enumerate()
    {
        view[index].store(value, Release);
    }
    view[11].store(AUTO.load(Relaxed) as u32, Release);
    view[12].store(AUTO_CONVERSIONS.load(Relaxed) as u32, Release);
    view[13].store(GAME_ACTIVE.load(Relaxed) as u32, Release);
    view[14].store(automatic_allowed() as u32, Release);
    if view[15].swap(0, AcqRel) == 1 {
        DestroyWindow(APP_WINDOW.load(Acquire) as HWND);
    }
}

#[derive(Clone)]
struct Settings {
    switch_layout: bool,
    auto_delay_ms: u64,
    exclude: Vec<String>,
    pause_auto_in_games: bool,
    game_exes: Vec<String>,
    update_repo: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            switch_layout: true,
            auto_delay_ms: 120,
            pause_auto_in_games: true,
            game_exes: Vec::new(),
            update_repo: option_env!("RIGHTLANG_UPDATE_REPO")
                .unwrap_or("Samsxx001/rightlang-rust")
                .into(),
            exclude: vec![
                "keepass.exe".into(),
                "keepassxc.exe".into(),
                "1password.exe".into(),
                "bitwarden.exe".into(),
            ],
        }
    }
}
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn app_dir() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .into()
}
fn settings() -> &'static Settings {
    SETTINGS.get().unwrap()
}
fn read_settings() -> Settings {
    let mut cfg = Settings::default();
    if let Ok(text) = std::fs::read_to_string(app_dir().join("rightlang.ini")) {
        for line in text
            .lines()
            .filter(|l| !l.trim_start().starts_with(['#', ';']))
        {
            if let Some((key, val)) = line.split_once('=') {
                match key.trim() {
                    "auto_correct" => AUTO.store(val.trim() != "false", Relaxed),
                    "update_repo" if !val.trim().is_empty() => cfg.update_repo = val.trim().into(),
                    "switch_layout" => cfg.switch_layout = val.trim() != "false",
                    "pause_auto_in_games" => cfg.pause_auto_in_games = val.trim() != "false",
                    "game_exes" => cfg.game_exes.extend(
                        val.split(',')
                            .map(|v| v.trim().to_lowercase())
                            .filter(|v| !v.is_empty()),
                    ),
                    "auto_delay_ms" => {
                        if let Ok(value) = val.trim().parse::<u64>() {
                            cfg.auto_delay_ms = value.clamp(60, 2000);
                        }
                    }
                    "exclude" => cfg.exclude.extend(
                        val.split(',')
                            .map(|v| v.trim().to_ascii_lowercase())
                            .filter(|v| !v.is_empty()),
                    ),
                    _ => {}
                }
            }
        }
    }
    cfg
}

#[derive(Clone, Copy)]
struct Key {
    vk: u32,
    scan: u32,
    mods: u8,
    caps: bool,
    layout: usize,
    hwnd: usize,
    focused: usize,
    seq: u64,
    epoch: u64,
}
#[derive(Clone, Copy)]
enum Event {
    Key(Key),
    Manual { hwnd: usize, seq: u64, epoch: u64 },
    Undo { hwnd: usize, seq: u64, epoch: u64 },
}
fn queue(event: Event) {
    if TX.get().is_some_and(|tx| tx.try_send(event).is_err()) {
        OVERFLOWS.fetch_add(1, Relaxed);
        EPOCH.fetch_add(1, SeqCst);
    }
}
struct InputState {
    keys: [bool; 256],
    caps: bool,
}
thread_local! {
    static KEYS: RefCell<InputState> = const { RefCell::new(InputState { keys: [false; 256], caps: false }) };
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
    static GAME_CACHE: RefCell<(usize, u32, u64)> = const { RefCell::new((0, 0, 0)) };
}

fn automatic_allowed() -> bool {
    !PAUSED.load(Relaxed) && automatic_enabled(AUTO.load(Relaxed), GAME_ACTIVE.load(Relaxed))
}

unsafe fn set_game_mode(game: bool) {
    if GAME_ACTIVE.swap(game, AcqRel) != game {
        EPOCH.fetch_add(1, SeqCst);
        let app = APP_WINDOW.load(Acquire) as HWND;
        if !app.is_null() {
            tray(app, NIM_MODIFY);
        }
    }
}

unsafe fn refresh_game_mode(hwnd: HWND, force: bool) {
    let cfg = settings();
    let now = GetTickCount64();
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let check = GAME_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !force
            && cache.0 == hwnd as usize
            && cache.1 == pid
            && now.saturating_sub(cache.2) < 1000
        {
            return false;
        }
        *cache = (hwnd as usize, pid, now);
        true
    });
    if !check {
        return;
    }
    let mut game = false;
    if cfg.pause_auto_in_games && !hwnd.is_null() {
        game = executable_path(hwnd as usize)
            .is_some_and(|path| is_game_executable(&path, &cfg.game_exes));
        if !game && foreground_covers_monitor(hwnd) {
            let mut state = QUNS_ACCEPTS_NOTIFICATIONS;
            game = SHQueryUserNotificationState(&mut state) >= 0
                && state == QUNS_RUNNING_D3D_FULL_SCREEN;
        }
    }
    // Cancel both an armed live edit and a word captured before switching apps.
    set_game_mode(game);
}

unsafe fn foreground_covers_monitor(hwnd: HWND) -> bool {
    let mut rect: RECT = zeroed();
    let mut monitor: MONITORINFO = zeroed();
    monitor.cbSize = size_of::<MONITORINFO>() as u32;
    GetWindowRect(hwnd, &mut rect) != 0
        && GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        ) != 0
        && rect.left <= monitor.rcMonitor.left
        && rect.top <= monitor.rcMonitor.top
        && rect.right >= monitor.rcMonitor.right
        && rect.bottom >= monitor.rcMonitor.bottom
}
#[derive(Clone, Copy)]
struct Pending {
    id: i32,
    hwnd: usize,
    focused: usize,
    epoch: u64,
    time: u32,
}
fn modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5b | 0x5c | 0xa0..=0xa5)
}
fn shortcut(vk: u32, mods: u8, registered: u32) -> Option<i32> {
    let id = match (vk, mods) {
        (0x08, 1) => SHIFT_MANUAL_ID,
        (0x20, 6) => MANUAL_ID,
        (0x5a, 6) => UNDO_ID,
        (0x50, 6) => PAUSE_ID,
        (0x52, 6) => REPAIR_ID,
        _ => return None,
    };
    (registered & (1 << id) != 0).then_some(id)
}
fn later(a: u32, b: u32) -> bool {
    a.wrapping_sub(b) as i32 > 0
}
fn normalize_raw(raw: &RAWKEYBOARD, time: u32) -> Option<(KBDLLHOOKSTRUCT, bool)> {
    if raw.VKey == 0xff || raw.ExtraInformation as usize == TAG {
        return None;
    }
    let extended = raw.Flags & RI_KEY_E0 as u16 != 0;
    let vk = match raw.VKey {
        VK_SHIFT => {
            if raw.MakeCode == 0x36 {
                VK_RSHIFT
            } else {
                VK_LSHIFT
            }
        }
        VK_CONTROL => {
            if extended {
                VK_RCONTROL
            } else {
                VK_LCONTROL
            }
        }
        VK_MENU => {
            if extended {
                VK_RMENU
            } else {
                VK_LMENU
            }
        }
        v => v,
    };
    Some((
        KBDLLHOOKSTRUCT {
            vkCode: vk as u32,
            scanCode: raw.MakeCode as u32,
            flags: 0,
            time,
            dwExtraInfo: 0,
        },
        raw.Flags & RI_KEY_BREAK as u16 == 0,
    ))
}
unsafe fn initialize_keys() {
    KEYS.with(|cell| {
        let mut state = cell.borrow_mut();
        for (i, key) in state.keys.iter_mut().enumerate() {
            *key = GetAsyncKeyState(i as i32) < 0;
        }
        for vk in [VK_SHIFT, VK_CONTROL, VK_MENU] {
            state.keys[vk as usize] = false;
        }
        state.caps = GetKeyState(VK_CAPITAL as i32) & 1 != 0;
    });
}
/// Raw Input observes physical keys; RegisterHotKey owns and consumes shortcuts.
/// A shortcut must not enter the word buffer as a destructive Backspace/Space.
unsafe fn process_key(data: &KBDLLHOOKSTRUCT, down: bool) -> bool {
    RAW_SEEN.store(GetTickCount64(), Relaxed);
    RAW_EVENTS.fetch_add(1, Relaxed);
    if down && SETTINGS.get().is_some() {
        refresh_game_mode(GetForegroundWindow(), false);
    }
    KEYS.with(|cell| {
        let mut state = cell.borrow_mut();
        let vk = data.vkCode;
        if vk >= 256 {
            return false;
        }
        let repeated = state.keys[vk as usize];
        state.keys[vk as usize] = down;
        if vk == VK_CAPITAL as u32 && down && !repeated {
            state.caps = !state.caps;
        }
        let shift = state.keys[VK_LSHIFT as usize] || state.keys[VK_RSHIFT as usize];
        let ctrl = state.keys[VK_LCONTROL as usize] || state.keys[VK_RCONTROL as usize];
        let alt = state.keys[VK_LMENU as usize] || state.keys[VK_RMENU as usize];
        let win = state.keys[VK_LWIN as usize] || state.keys[VK_RWIN as usize];
        if !down || modifier(vk) {
            return false;
        }
        let mods = shift as u8 | ((ctrl as u8) << 1) | ((alt as u8) << 2) | ((win as u8) << 3);
        if shortcut(vk, mods, REGISTERED.load(Relaxed)).is_some() {
            return true;
        }
        let seq = SEQUENCE.fetch_add(1, SeqCst) + 1;
        LAST_KEY_TIME.store(data.time, Relaxed);
        if !PAUSED.load(Relaxed) {
            let hwnd = GetForegroundWindow();
            let focused = focus(hwnd as usize);
            let tid = GetWindowThreadProcessId(
                if focused != 0 { focused as HWND } else { hwnd },
                null_mut(),
            );
            queue(Event::Key(Key {
                vk,
                scan: data.scanCode,
                mods,
                caps: state.caps,
                layout: GetKeyboardLayout(tid) as usize,
                hwnd: hwnd as usize,
                focused,
                seq,
                epoch: EPOCH.load(SeqCst),
            }));
        }
        false
    })
}
unsafe fn register_input(hwnd: HWND) -> bool {
    REGISTRATION_CALLS.fetch_add(1, Relaxed);
    let devices = [
        RAWINPUTDEVICE {
            usUsagePage: 1,
            usUsage: 6,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        },
        RAWINPUTDEVICE {
            usUsagePage: 1,
            usUsage: 2,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        },
    ];
    let ok = RegisterRawInputDevices(
        devices.as_ptr(),
        devices.len() as u32,
        size_of::<RAWINPUTDEVICE>() as u32,
    ) != 0;
    RAW_OK.store(ok, Relaxed);
    ok
}
unsafe fn device_changed() {
    // RIDEV_DEVNOTIFY can report attached devices when registration changes.
    // Re-registering from this notification can feed notifications back into
    // the queue and starve keyboard input/timers. The registration already
    // covers arriving devices; invalidate the captured word only.
    DEVICE_EVENTS.fetch_add(1, Relaxed);
    EPOCH.fetch_add(1, SeqCst);
    initialize_keys();
}
unsafe fn register_shortcuts(hwnd: HWND) {
    let mut registered = REGISTERED.load(Relaxed);
    for (id, mods, key) in [
        (PAUSE_ID, MOD_CONTROL | MOD_ALT, 0x50),
        (MANUAL_ID, MOD_CONTROL | MOD_ALT, 0x20),
        (UNDO_ID, MOD_CONTROL | MOD_ALT, 0x5a),
        (REPAIR_ID, MOD_CONTROL | MOD_ALT, 0x52),
        (SHIFT_MANUAL_ID, MOD_SHIFT, VK_BACK as u32),
    ] {
        if id == SHIFT_MANUAL_ID && PAUSED.load(Relaxed) {
            if registered & (1 << id) != 0 {
                UnregisterHotKey(hwnd, id);
                registered &= !(1 << id);
            }
        } else if registered & (1 << id) == 0
            && RegisterHotKey(hwnd, id, mods | MOD_NOREPEAT, key) != 0
        {
            registered |= 1 << id;
        }
    }
    REGISTERED.store(registered, Release);
    HOTKEY_ERRORS.store(
        (1..=5)
            .filter(|id| {
                !(PAUSED.load(Relaxed) && *id == SHIFT_MANUAL_ID) && registered & (1 << id) == 0
            })
            .count() as u32,
        Relaxed,
    );
}
unsafe fn dispatch_pending(hwnd: HWND) {
    KillTimer(hwnd, 2);
    if let Some(pending) = PENDING.with(|p| p.borrow_mut().take()) {
        if pending.epoch != EPOCH.load(SeqCst)
            || later(LAST_KEY_TIME.load(Relaxed), pending.time)
            || GetForegroundWindow() as usize != pending.hwnd
            || focus(pending.hwnd) != pending.focused
        {
            LAST_RESULT.store(3, Relaxed);
            return;
        }
        let seq = SEQUENCE.load(SeqCst);
        queue(if pending.id == UNDO_ID {
            Event::Undo {
                hwnd: pending.hwnd,
                seq,
                epoch: pending.epoch,
            }
        } else {
            Event::Manual {
                hwnd: pending.hwnd,
                seq,
                epoch: pending.epoch,
            }
        });
    }
}

#[derive(Default)]
struct Tail {
    text: String,
    spaces: usize,
    hwnd: usize,
    focus: usize,
    layout: usize,
    epoch: u64,
    lang: Option<Language>,
    blocked: bool,
    undo: Option<(String, String, usize)>,
}
impl Tail {
    fn clear(&mut self) {
        self.text.clear();
        self.spaces = 0;
        self.lang = None;
        self.blocked = false;
        self.undo = None;
    }
}

unsafe fn focus(hwnd: usize) -> usize {
    let mut gui: GUITHREADINFO = zeroed();
    gui.cbSize = size_of::<GUITHREADINFO>() as u32;
    let tid = GetWindowThreadProcessId(hwnd as HWND, null_mut());
    if tid == 0 || GetGUIThreadInfo(tid, &mut gui) == 0 {
        return 0;
    }
    gui.hwndFocus as usize
}
fn password_style(class: &str, style: u32) -> bool {
    let name = class.to_ascii_lowercase();
    (name == "edit" || name.starts_with("richedit")) && style & ES_PASSWORD as u32 != 0
}
unsafe fn permitted(hwnd: usize, focused: usize) -> bool {
    let mut class = [0u16; 128];
    let n = GetClassNameW(focused as HWND, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    let class = String::from_utf16_lossy(&class[..n]);
    if focused == 0 || password_style(&class, GetWindowLongPtrW(focused as HWND, GWL_STYLE) as u32)
    {
        return false;
    }
    let Some(path) = executable_path(hwnd) else {
        return false;
    };
    let name = path.rsplit('\\').next().unwrap_or("").to_ascii_lowercase();
    !settings().exclude.iter().any(|excluded| excluded == &name)
}

unsafe fn executable_path(hwnd: usize) -> Option<String> {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd as HWND, &mut pid);
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return None;
    }
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    let result = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
    CloseHandle(process);
    if result == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}
unsafe fn translate(vk: u32, scan: u32, mods: u8, caps: bool, layout: usize) -> Option<char> {
    let mut state = [0u8; 256];
    state[VK_SHIFT as usize] = if mods & 1 != 0 { 0x80 } else { 0 };
    state[VK_CAPITAL as usize] = caps as u8;
    let mut chars = [0u16; 4];
    // Bit 2 prevents modifying the foreground application's dead-key state.
    let len = ToUnicodeEx(
        vk,
        scan,
        state.as_ptr(),
        chars.as_mut_ptr(),
        4,
        4,
        layout as HKL,
    );
    if len == 1 {
        char::from_u32(chars[0] as u32)
    } else {
        None
    }
}
unsafe fn language(layout: usize) -> Option<Language> {
    let a = translate(0x41, 0x1e, 0, false, layout)?;
    let s = translate(0x53, 0x1f, 0, false, layout)?;
    match (a, s) {
        ('a', 's') => Some(Language::English),
        ('ฟ', 'ห') => Some(Language::Thai),
        _ => None,
    }
}
fn key_input(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: TAG,
            },
        },
    }
}
fn replacement_inputs(remove: usize, replacement: &str) -> Vec<INPUT> {
    let mut input = Vec::with_capacity(remove * 2 + replacement.len() * 2);
    for _ in 0..remove {
        input.push(key_input(VK_BACK, 0, 0));
        input.push(key_input(VK_BACK, 0, KEYEVENTF_KEYUP));
    }
    for ch in replacement.encode_utf16() {
        input.push(key_input(0, ch, KEYEVENTF_UNICODE));
        input.push(key_input(0, ch, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    input
}
unsafe fn safe_now(tail: &Tail, seq: u64, epoch: u64) -> bool {
    !PAUSED.load(Acquire)
        && EPOCH.load(SeqCst) == epoch
        && SEQUENCE.load(SeqCst) == seq
        && GetForegroundWindow() as usize == tail.hwnd
        && focus(tail.hwnd) == tail.focus
}
unsafe fn replace(
    tail: &Tail,
    before: &str,
    after: &str,
    spaces: usize,
    seq: u64,
    epoch: u64,
    wait: bool,
) -> bool {
    if wait {
        let start = Instant::now();
        // Never synthesize text while Ctrl/Alt/Shift/Win is physically held.
        while [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|v| GetAsyncKeyState(*v as i32) < 0)
        {
            if start.elapsed() > Duration::from_millis(900) || !safe_now(tail, seq, epoch) {
                LAST_RESULT.store(6, Relaxed);
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    } else if [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|v| GetAsyncKeyState(*v as i32) < 0)
    {
        LAST_RESULT.store(6, Relaxed);
        return false;
    }
    if !safe_now(tail, seq, epoch) {
        LAST_RESULT.store(3, Relaxed);
        return false;
    }
    if !permitted(tail.hwnd, tail.focus) {
        LAST_RESULT.store(4, Relaxed);
        return false;
    }
    let text = format!("{after}{}", " ".repeat(spaces));
    let inputs = replacement_inputs(before.encode_utf16().count() + spaces, &text);
    if !safe_now(tail, seq, epoch) {
        return false;
    }
    let sent = SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        size_of::<INPUT>() as i32,
    );
    if sent != inputs.len() as u32 {
        FAILURES.fetch_add(1, Relaxed);
        LAST_RESULT.store(7, Relaxed);
        EPOCH.fetch_add(1, SeqCst);
        return false;
    }
    LAST_RESULT.store(1, Relaxed);
    true
}
unsafe fn switch_language(hwnd: usize, target: Language) {
    if !settings().switch_layout {
        return;
    }
    let mut layouts = [null_mut(); 32];
    let count = GetKeyboardLayoutList(layouts.len() as i32, layouts.as_mut_ptr())
        .clamp(0, layouts.len() as i32);
    for layout in layouts.iter().take(count as usize) {
        if language(*layout as usize) == Some(target) {
            PostMessageW(hwnd as HWND, WM_INPUTLANGCHANGEREQUEST, 0, *layout as isize);
            break;
        }
    }
}
fn opposite(lang: Language) -> Language {
    match lang {
        Language::English => Language::Thai,
        Language::Thai => Language::English,
    }
}

#[derive(Clone, Copy)]
struct LivePending {
    due: Instant,
    seq: u64,
    epoch: u64,
}
impl LivePending {
    fn ready(self, now: Instant, seq: u64, epoch: u64) -> bool {
        now >= self.due && self.seq == seq && self.epoch == epoch
    }
}

unsafe fn apply_automatic(tail: &mut Tail, seq: u64, epoch: u64, live: bool) -> bool {
    if !automatic_allowed() || tail.blocked || tail.text.is_empty() {
        return false;
    }
    let Some(lang) = tail.lang else {
        return false;
    };
    let candidate = if live {
        automatic_live(&tail.text, lang)
    } else {
        automatic(&tail.text, lang)
    };
    let Some(candidate) = candidate else {
        return false;
    };
    // Wait for the final Shift/modifier to be released. New input or a focus
    // change cancels through safe_now while the input thread continues running.
    if !replace(tail, &tail.text, &candidate, tail.spaces, seq, epoch, true) {
        return false;
    }
    let before = std::mem::replace(&mut tail.text, candidate.clone());
    tail.undo = Some((before, candidate, tail.spaces));
    tail.lang = Some(opposite(lang));
    CONVERSIONS.fetch_add(1, Relaxed);
    AUTO_CONVERSIONS.fetch_add(1, Relaxed);
    switch_language(tail.hwnd, opposite(lang));
    true
}

fn worker(rx: mpsc::Receiver<Event>) {
    let mut tail = Tail::default();
    let mut last_event = Instant::now();
    let mut live_pending: Option<LivePending> = None;
    while !STOP.load(Relaxed) {
        let wait = live_pending
            .map(|pending| pending.due.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(1));
        let event = match rx.recv_timeout(wait) {
            Ok(e) => e,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(pending) = live_pending.take()
                    && pending.ready(Instant::now(), SEQUENCE.load(SeqCst), EPOCH.load(SeqCst))
                    && tail.spaces == 0
                {
                    unsafe {
                        apply_automatic(&mut tail, pending.seq, pending.epoch, true);
                    }
                }
                if tail.epoch != EPOCH.load(SeqCst)
                    || PAUSED.load(Relaxed)
                    || last_event.elapsed() > Duration::from_secs(30)
                {
                    tail.clear();
                }
                continue;
            }
            Err(_) => break,
        };
        // Only the latest text key may trigger a delayed edit. Navigation,
        // shortcuts and another key all invalidate the previous pending edit.
        live_pending = None;
        last_event = Instant::now();
        unsafe {
            match event {
                Event::Key(key) => {
                    let focused = key.focused;
                    if focused == 0 || focused != focus(key.hwnd) {
                        tail.clear();
                        continue;
                    }
                    if key.epoch != EPOCH.load(SeqCst) || PAUSED.load(Relaxed) {
                        tail.clear();
                        continue;
                    }
                    if tail.hwnd != key.hwnd || tail.focus != focused || tail.epoch != key.epoch {
                        tail.clear();
                        tail.hwnd = key.hwnd;
                        tail.focus = focused;
                        tail.epoch = key.epoch;
                    }
                    if tail.layout != key.layout {
                        // A layout change requested by our own last replacement preserves undo.
                        if tail.undo.is_none() {
                            tail.clear();
                        }
                        tail.layout = key.layout;
                    }
                    if key.mods & 14 != 0 {
                        if key.mods & 6 == 6 && matches!(key.vk, 0x20 | 0x5a | 0x50 | 0x52) {
                            continue;
                        }
                        tail.clear();
                        continue;
                    }
                    if key.vk == VK_SPACE as u32 {
                        if tail.blocked {
                            tail.clear();
                            continue;
                        }
                        if tail.text.is_empty() {
                            continue;
                        }
                        tail.spaces += 1;
                        if tail.spaces != 1 || !automatic_allowed() {
                            continue;
                        }
                        apply_automatic(&mut tail, key.seq, key.epoch, false);
                        continue;
                    }
                    if matches!(key.vk, 0x08 | 0x09 | 0x0d | 0x1b | 0x21..=0x2e | 0x70..=0x87 | 0x5d)
                    {
                        tail.clear();
                        continue;
                    }
                    if key.vk == VK_CAPITAL as u32 {
                        continue;
                    }
                    if tail.spaces > 0 {
                        tail.clear();
                    }
                    if tail.blocked {
                        continue;
                    }
                    let Some(lang) = language(key.layout) else {
                        tail.clear();
                        continue;
                    };
                    let Some(ch) = translate(key.vk, key.scan, key.mods, key.caps, key.layout)
                    else {
                        tail.clear();
                        continue;
                    };
                    if ch.is_control() || ch.is_whitespace() {
                        tail.clear();
                        continue;
                    }
                    if tail.lang.is_some_and(|l| l != lang) {
                        tail.clear();
                    }
                    if tail.text.chars().count() >= 96 {
                        tail.clear();
                        tail.blocked = true;
                        continue;
                    }
                    // Standard password fields and excluded applications never enter the buffer.
                    if !permitted(key.hwnd, focused) {
                        tail.clear();
                        continue;
                    }
                    tail.lang = Some(lang);
                    tail.undo = None;
                    tail.text.push(ch);
                    if automatic_allowed() {
                        live_pending = Some(LivePending {
                            due: Instant::now() + Duration::from_millis(settings().auto_delay_ms),
                            seq: key.seq,
                            epoch: key.epoch,
                        });
                    }
                }
                Event::Manual { hwnd, seq, epoch } => {
                    if tail.hwnd != hwnd
                        || tail.epoch != epoch
                        || tail.text.is_empty()
                        || tail.blocked
                    {
                        LAST_RESULT.store(if tail.text.is_empty() { 2 } else { 3 }, Relaxed);
                        tail.clear();
                        continue;
                    }
                    if let Some(lang) = tail.lang
                        && let Some(candidate) = convert(&tail.text, lang)
                    {
                        if replace(&tail, &tail.text, &candidate, tail.spaces, seq, epoch, true) {
                            let before = std::mem::replace(&mut tail.text, candidate.clone());
                            tail.undo = Some((before, candidate, tail.spaces));
                            tail.lang = Some(opposite(lang));
                            CONVERSIONS.fetch_add(1, Relaxed);
                            switch_language(hwnd, opposite(lang));
                        } else {
                            tail.clear();
                        }
                    } else {
                        LAST_RESULT.store(8, Relaxed);
                    }
                }
                Event::Undo { hwnd, seq, epoch } => {
                    if tail.hwnd != hwnd || tail.epoch != epoch {
                        tail.clear();
                        continue;
                    }
                    if let Some((before, after, _)) = tail.undo.take() {
                        if replace(&tail, &after, &before, tail.spaces, seq, epoch, true) {
                            tail.text = before;
                            if let Some(lang) = tail.lang {
                                tail.lang = Some(opposite(lang));
                                switch_language(hwnd, opposite(lang));
                            }
                        } else {
                            tail.clear();
                        }
                    }
                }
            }
        }
    }
}

fn result_label(result: u32) -> &'static str {
    match result {
        0 => "Ready",
        1 => "Converted",
        2 => "No recent word: type a new word first",
        3 => "Cancelled: focus or input changed",
        4 => "Excluded app or password field",
        6 => "Cancelled: modifier held too long",
        7 => "Windows blocked input (check app elevation)",
        8 => "Unsupported characters or layout",
        9 => "Could not schedule shortcut",
        _ => "Unknown",
    }
}

unsafe fn tray(hwnd: HWND, operation: u32) {
    let mut icon: NOTIFYICONDATAW = zeroed();
    icon.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    icon.hWnd = hwnd;
    icon.uID = 1;
    icon.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    icon.uCallbackMessage = TRAY;
    icon.hIcon = LoadIconW(
        null_mut(),
        if PAUSED.load(Relaxed) {
            IDI_WARNING
        } else {
            IDI_INFORMATION
        },
    );
    let text = format!(
        concat!(
            "RightLang Rust ",
            env!("CARGO_PKG_VERSION"),
            " | {} | {} | input resets {}"
        ),
        if PAUSED.load(Relaxed) {
            "Paused"
        } else {
            "Active"
        },
        if GAME_ACTIVE.load(Relaxed) {
            "Auto suspended: game"
        } else if AUTO.load(Relaxed) {
            "Auto"
        } else {
            "Manual"
        },
        REPAIRS.load(Relaxed)
    );
    for (dst, src) in icon.szTip.iter_mut().zip(text.encode_utf16()) {
        *dst = src;
    }
    Shell_NotifyIconW(operation, &icon);
}
fn request_repair() {
    EPOCH.fetch_add(1, SeqCst);
    unsafe {
        let hwnd = APP_WINDOW.load(Acquire) as HWND;
        if !hwnd.is_null() {
            register_input(hwnd);
            initialize_keys();
            register_shortcuts(hwnd);
            REPAIRS.fetch_add(1, Relaxed);
        }
    }
}
unsafe fn diagnostics(hwnd: HWND) {
    let text = format!(
        concat!(
            "RightLang Rust ",
            env!("CARGO_PKG_VERSION"),
            "\n\nRaw Input: {}\nPhysical key events: {}\nManual requests: {}\nConversions: {}\nInput resets: {}\nFailed batches: {}\nQueue overflows: {}\nHotkey conflicts: {}\nShift+Backspace registered: {}\nLast result: {}\n\nAuto corrects while typing without Space (120 ms default).\nGame pause: enabled by default; see tray tooltip.\nShift+Backspace / Ctrl+Alt+Space: convert latest typed word\nCtrl+Alt+Z: undo\nCtrl+Alt+P: pause/resume\nCtrl+Alt+R: refresh input\n\nNo typed text is written to files. Settings: rightlang.ini"
        ),
        RAW_OK.load(Relaxed),
        RAW_EVENTS.load(Relaxed),
        MANUAL_REQUESTS.load(Relaxed),
        CONVERSIONS.load(Relaxed),
        REPAIRS.load(Relaxed),
        FAILURES.load(Relaxed),
        OVERFLOWS.load(Relaxed),
        HOTKEY_ERRORS.load(Relaxed),
        REGISTERED.load(Relaxed) & (1 << SHIFT_MANUAL_ID) != 0,
        result_label(LAST_RESULT.load(Relaxed))
    );
    MessageBoxW(
        hwnd,
        w(&text).as_ptr(),
        w("RightLang Rust - Status").as_ptr(),
        MB_OK | MB_ICONINFORMATION,
    );
}
unsafe fn context_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    AppendMenuW(
        menu,
        MF_STRING,
        10,
        w(if PAUSED.load(Relaxed) {
            "Resume / เริ่มทำงาน"
        } else {
            "Pause / หยุดชั่วคราว"
        })
        .as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING | if AUTO.load(Relaxed) { MF_CHECKED } else { 0 },
        11,
        w("Auto correct while typing / แก้อัตโนมัติขณะพิมพ์").as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        12,
        w("Refresh input / ซ่อมการรับคีย์บอร์ด").as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        13,
        w("Status & shortcuts / สถานะและปุ่มลัด").as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING,
        15,
        w("Check for updates / ตรวจสอบอัปเดต").as_ptr(),
    );
    AppendMenuW(menu, MF_STRING, 14, w("Exit / ออกจากโปรแกรม").as_ptr());
    let mut point: POINT = zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    );
    DestroyMenu(menu);
    PostMessageW(hwnd, WM_NULL, 0, 0);
    match cmd {
        10 => {
            PAUSED.fetch_xor(true, SeqCst);
            EPOCH.fetch_add(1, SeqCst);
            if !PAUSED.load(Relaxed) {
                request_repair();
            }
        }
        11 => {
            AUTO.fetch_xor(true, Relaxed);
            EPOCH.fetch_add(1, SeqCst);
        }
        12 => request_repair(),
        13 => diagnostics(hwnd),
        15 => {
            if let Err(error) = crate::updater::launch(&settings().update_repo) {
                show_error(&error);
            }
        }
        14 => {
            DestroyWindow(hwnd);
        }
        _ => {}
    }
    register_shortcuts(hwnd);
    tray(hwnd, NIM_MODIFY);
}
unsafe fn raw_input(lp: LPARAM) {
    let mut raw: RAWINPUT = zeroed();
    let mut size = size_of::<RAWINPUT>() as u32;
    if GetRawInputData(
        lp as HRAWINPUT,
        RID_INPUT,
        &mut raw as *mut _ as *mut _,
        &mut size,
        size_of::<RAWINPUTHEADER>() as u32,
    ) == u32::MAX
    {
        return;
    }
    if raw.header.dwType == RIM_TYPEKEYBOARD
        && let Some((sample, down)) = normalize_raw(&raw.data.keyboard, GetMessageTime() as u32)
    {
        process_key(&sample, down);
    }
    if raw.header.dwType == RIM_TYPEMOUSE && raw.data.mouse.Anonymous.Anonymous.usButtonFlags != 0 {
        EPOCH.fetch_add(1, SeqCst);
        SEQUENCE.fetch_add(1, SeqCst);
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            raw_input(lp);
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        WM_HOTKEY => {
            match wp as i32 {
                PAUSE_ID => {
                    PAUSED.fetch_xor(true, SeqCst);
                    EPOCH.fetch_add(1, SeqCst);
                    if !PAUSED.load(Relaxed) {
                        request_repair();
                    }
                    register_shortcuts(hwnd);
                    tray(hwnd, NIM_MODIFY);
                }
                REPAIR_ID => request_repair(),
                MANUAL_ID | UNDO_ID | SHIFT_MANUAL_ID => {
                    MANUAL_REQUESTS.fetch_add(1, Relaxed);
                    let target = GetForegroundWindow() as usize;
                    PENDING.with(|p| {
                        *p.borrow_mut() = Some(Pending {
                            id: wp as i32,
                            hwnd: target,
                            focused: focus(target),
                            epoch: EPOCH.load(SeqCst),
                            time: GetMessageTime() as u32,
                        })
                    });
                    // WM_HOTKEY can precede WM_INPUT for its own chord. Timer dispatch
                    // drains earlier raw samples first, then verifies no later text key arrived.
                    if SetTimer(hwnd, 2, 20, None) == 0 {
                        LAST_RESULT.store(9, Relaxed);
                    }
                }
                _ => {}
            }
            return 0;
        }
        WM_TIMER => {
            if wp == 2 {
                dispatch_pending(hwnd);
            } else {
                refresh_game_mode(GetForegroundWindow(), true);
                let now = GetTickCount64();
                if now.saturating_sub(LAST_REFRESH.load(Relaxed)) >= 30_000 {
                    register_input(hwnd);
                    register_shortcuts(hwnd);
                    LAST_REFRESH.store(now, Relaxed);
                }
                publish_status();
            }
            return 0;
        }
        WM_INPUT_DEVICE_CHANGE => {
            device_changed();
            return 0;
        }
        STATUS => {
            return match wp {
                0 => crate::updater::VERSION_CODE as isize,
                1 => RAW_EVENTS.load(Relaxed) as isize,
                2 => MANUAL_REQUESTS.load(Relaxed) as isize,
                3 => CONVERSIONS.load(Relaxed) as isize,
                4 => LAST_RESULT.load(Relaxed) as isize,
                5 => REGISTERED.load(Relaxed) as isize,
                6 => RAW_OK.load(Relaxed) as isize,
                7 => FAILURES.load(Relaxed) as isize,
                _ => 0,
            };
        }
        WM_POWERBROADCAST => {
            if wp == 18 || wp == 7 {
                request_repair();
            }
            return 1;
        }
        WM_WTSSESSION_CHANGE => {
            EPOCH.fetch_add(1, SeqCst);
            request_repair();
            return 0;
        }
        TRAY => {
            if lp as u32 == WM_RBUTTONUP {
                context_menu(hwnd);
            } else if lp as u32 == WM_LBUTTONDBLCLK {
                diagnostics(hwnd);
            }
            return 0;
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            tray(hwnd, NIM_DELETE);
            KillTimer(hwnd, 1);
            KillTimer(hwnd, 2);
            WTSUnRegisterSessionNotification(hwnd);
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }
    // Explorer recreates the notification area after it restarts.
    let taskbar = RegisterWindowMessageW(w("TaskbarCreated").as_ptr());
    if taskbar != 0 && msg == taskbar {
        tray(hwnd, NIM_ADD);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

pub fn show_error(text: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            w(text).as_ptr(),
            w("RightLang Rust").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
pub fn run() -> Result<(), String> {
    unsafe {
        let mutex = CreateMutexW(null(), 0, w("Local\\RightLangRust-0.1").as_ptr());
        if mutex.is_null() {
            return Err("Cannot create application mutex".into());
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return Ok(());
        }
        SETTINGS.set(read_settings()).ok();
        let instance = GetModuleHandleW(null());
        let name = w("RightLangRustWindow");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return Err(format!("RegisterClass: {}", GetLastError()));
        }
        // Hidden top-level window receives resume, session and Explorer broadcasts.
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            w("RightLang Rust").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(format!("CreateWindow: {}", GetLastError()));
        }
        APP_WINDOW.store(hwnd as usize, Release);
        if !register_input(hwnd) {
            return Err(format!("Raw Input registration failed: {}", GetLastError()));
        }
        initialize_keys();
        let (tx, rx) = mpsc::sync_channel(128);
        TX.set(tx).ok();
        let worker = thread::Builder::new()
            .name("converter".into())
            .stack_size(256 * 1024)
            .spawn(move || worker(rx))
            .map_err(|e| e.to_string())?;
        register_shortcuts(hwnd);
        let status_mapping = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            null(),
            PAGE_READWRITE,
            0,
            4096,
            w("Local\\RightLangRust.Status-0.1").as_ptr(),
        );
        let status_view = if !status_mapping.is_null() {
            MapViewOfFile(status_mapping, FILE_MAP_ALL_ACCESS, 0, 0, 4096)
        } else {
            MEMORY_MAPPED_VIEW_ADDRESS { Value: null_mut() }
        };
        STATUS_VIEW.store(status_view.Value as usize, Release);
        publish_status();
        LAST_REFRESH.store(GetTickCount64(), Relaxed);
        if SetTimer(hwnd, 1, 1000, None) == 0 {
            return Err("Cannot start input refresh timer".into());
        }
        startup_stage(10);
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        startup_stage(11);
        tray(hwnd, NIM_ADD);
        startup_stage(12);
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        STOP.store(true, Relaxed);
        for id in 1..=5 {
            UnregisterHotKey(hwnd, id);
        }
        let _ = worker.join();
        STATUS_VIEW.store(0, Release);
        if !status_view.Value.is_null() {
            UnmapViewOfFile(status_view);
        }
        if !status_mapping.is_null() {
            CloseHandle(status_mapping);
        }
        CloseHandle(mutex);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::Controls::EM_SETSEL;

    #[test]
    fn shift_backspace_is_registered_command_not_a_destructive_key() {
        let all = (1..=5).fold(0, |bits, id| bits | (1 << id));
        assert_eq!(shortcut(VK_BACK as u32, 1, all), Some(SHIFT_MANUAL_ID));
        assert_eq!(shortcut(VK_BACK as u32, 0, all), None);
        assert_eq!(shortcut(VK_BACK as u32, 3, all), None);
        assert_eq!(
            shortcut(VK_BACK as u32, 1, all & !(1 << SHIFT_MANUAL_ID)),
            None
        );
        assert_eq!(shortcut(VK_SPACE as u32, 6, all), Some(MANUAL_ID));
        assert_eq!(shortcut(0x5a, 6, all), Some(UNDO_ID));
    }

    #[test]
    fn raw_left_right_modifiers_and_break_are_normalized() {
        for (vk, scan, flags, expected, down) in [
            (VK_SHIFT, 0x2a, 0, VK_LSHIFT, true),
            (VK_SHIFT, 0x36, 0, VK_RSHIFT, true),
            (VK_SHIFT, 0x36, RI_KEY_BREAK as u16, VK_RSHIFT, false),
            (VK_CONTROL, 0x1d, RI_KEY_E0 as u16, VK_RCONTROL, true),
            (VK_CONTROL, 0x1d, 0, VK_LCONTROL, true),
            (VK_MENU, 0x38, RI_KEY_E0 as u16, VK_RMENU, true),
        ] {
            let raw = RAWKEYBOARD {
                VKey: vk,
                MakeCode: scan,
                Flags: flags,
                ..Default::default()
            };
            let (key, is_down) = normalize_raw(&raw, 123).unwrap();
            assert_eq!(key.vkCode, expected as u32);
            assert_eq!(key.time, 123);
            assert_eq!(is_down, down);
        }
        assert!(
            normalize_raw(
                &RAWKEYBOARD {
                    VKey: 0xff,
                    ..Default::default()
                },
                0
            )
            .is_none()
        );
        assert!(
            normalize_raw(
                &RAWKEYBOARD {
                    VKey: 0x41,
                    ExtraInformation: TAG as u32,
                    ..Default::default()
                },
                0
            )
            .is_none()
        );
    }

    #[test]
    fn browser_style_bits_must_not_be_treated_as_edit_password_flags() {
        assert!(password_style("Edit", ES_PASSWORD as u32));
        assert!(password_style("RICHEDIT50W", ES_PASSWORD as u32));
        assert!(!password_style("Chrome_WidgetWin_1", ES_PASSWORD as u32));
        assert!(!password_style(
            "Chrome_RenderWidgetHostHWND",
            ES_PASSWORD as u32
        ));
        assert!(!password_style("Edit", ES_MULTILINE as u32));
    }

    #[test]
    fn hotkey_deferral_rejects_later_text_and_handles_tick_wrap() {
        assert!(!later(100, 100));
        assert!(!later(99, 100));
        assert!(later(101, 100));
        assert!(later(5, u32::MAX - 2));
        assert!(!later(u32::MAX - 2, 5));
    }

    #[test]
    fn live_edit_waits_for_last_key_and_cancels_on_new_input_or_click() {
        let start = Instant::now();
        let pending = LivePending {
            due: start + Duration::from_millis(120),
            seq: 10,
            epoch: 3,
        };
        assert!(!pending.ready(start, 10, 3));
        assert!(pending.ready(start + Duration::from_millis(120), 10, 3));
        assert!(!pending.ready(start + Duration::from_millis(120), 11, 3));
        assert!(!pending.ready(start + Duration::from_millis(120), 10, 4));
    }

    #[test]
    fn device_notification_invalidates_word_without_registering_again() {
        let before = REGISTRATION_CALLS.load(Relaxed);
        let epoch = EPOCH.load(SeqCst);
        unsafe {
            device_changed();
        }
        assert_eq!(REGISTRATION_CALLS.load(Relaxed), before);
        assert!(EPOCH.load(SeqCst) > epoch);
    }

    #[test]
    fn raw_shortcut_does_not_clear_word_or_advance_input_sequence() {
        // No receiver or window is needed: raw samples for a registered shortcut
        // must be consumed before the word-tracking queue is touched.
        REGISTERED.store(1 << SHIFT_MANUAL_ID, Relaxed);
        KEYS.with(|keys| keys.borrow_mut().keys[VK_RSHIFT as usize] = true);
        let before = SEQUENCE.load(SeqCst);
        unsafe {
            assert!(process_key(
                &KBDLLHOOKSTRUCT {
                    vkCode: VK_BACK as u32,
                    ..zeroed()
                },
                true
            ));
        }
        assert_eq!(SEQUENCE.load(SeqCst), before);
        KEYS.with(|keys| keys.borrow_mut().keys[VK_RSHIFT as usize] = false);
        REGISTERED.store(0, Relaxed);
    }

    #[test]
    #[ignore = "creates a hidden Win32 EDIT control; no global keyboard or focus is used"]
    fn native_game_auto_guard() {
        unsafe {
            let mut cfg = Settings::default();
            cfg.game_exes.push(
                std::env::current_exe()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_lowercase(),
            );
            assert!(SETTINGS.set(cfg).is_ok());
            let edit = CreateWindowExW(
                0,
                w("EDIT").as_ptr(),
                w("dki").as_ptr(),
                0,
                0,
                0,
                100,
                30,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            );
            assert!(!edit.is_null());
            AUTO.store(true, Relaxed);
            let epoch = EPOCH.load(SeqCst);
            refresh_game_mode(edit, true);
            assert!(GAME_ACTIVE.load(Relaxed));
            assert!(!automatic_allowed());
            assert!(
                AUTO.load(Relaxed),
                "game mode must preserve the user's Auto setting"
            );
            assert!(
                EPOCH.load(SeqCst) > epoch,
                "entering game cancels pending edits"
            );
            LAST_RESULT.store(999, Relaxed);
            let mut tail = Tail {
                text: "dki".into(),
                lang: Some(Language::English),
                hwnd: edit as usize,
                focus: edit as usize,
                ..Default::default()
            };
            assert!(!apply_automatic(
                &mut tail,
                SEQUENCE.load(SeqCst),
                EPOCH.load(SeqCst),
                true
            ));
            assert_eq!(tail.text, "dki");
            assert_eq!(
                LAST_RESULT.load(Relaxed),
                999,
                "must stop before attempting to send keys"
            );
            set_game_mode(false);
            assert!(
                automatic_allowed(),
                "leaving a game resumes Auto if requested"
            );
            AUTO.store(false, Relaxed);
            set_game_mode(true);
            set_game_mode(false);
            assert!(
                !automatic_allowed(),
                "leaving a game must not enable a user-disabled Auto"
            );
            DestroyWindow(edit);
        }
    }

    #[test]
    #[ignore = "creates a hidden Win32 EDIT control; no global keyboard or focus is used"]
    fn native_edit_replacement() {
        unsafe {
            let edit = CreateWindowExW(
                0,
                w("EDIT").as_ptr(),
                w("prefix l;ylfu ").as_ptr(),
                ES_MULTILINE as u32,
                0,
                0,
                300,
                100,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            );
            assert!(!edit.is_null());
            SendMessageW(edit, EM_SETSEL, 0xffff, 0xffff);
            for input in replacement_inputs(7, "สวัสดี ") {
                let key = input.Anonymous.ki;
                if key.dwFlags & KEYEVENTF_KEYUP != 0 {
                    continue;
                }
                SendMessageW(
                    edit,
                    WM_CHAR,
                    if key.wVk == VK_BACK {
                        8
                    } else {
                        key.wScan as usize
                    },
                    1,
                );
            }
            let mut text = [0u16; 128];
            let n = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32);
            assert_eq!(
                String::from_utf16_lossy(&text[..n as usize]),
                "prefix สวัสดี "
            );
            DestroyWindow(edit);
        }
    }
}
