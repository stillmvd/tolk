use std::time::{Duration, Instant};
use tolk_core::config::Config;
use tolk_core::engine::{CoreEvent, Engine};
use tolk_core::win::hook::TEST_MARK;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn edit_window() {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap().into();
        let class = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance, lpszClassName: w!("TolkE2E"), ..Default::default() };
        RegisterClassW(&class);
        let top = CreateWindowExW(WINDOW_EX_STYLE(0), w!("TolkE2E"), w!("Tolk E2E"), WS_OVERLAPPEDWINDOW | WS_VISIBLE, 200, 200, 600, 200, None, None, Some(instance), None).unwrap();
        let style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32);
        let edit = CreateWindowExW(WINDOW_EX_STYLE(0), w!("EDIT"), w!(""), style, 0, 0, 580, 160, Some(top), None, Some(instance), None).unwrap();
        let _ = SetForegroundWindow(top);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(edit));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: TEST_MARK } } }
}

fn send(v: &[INPUT]) {
    for i in v {
        unsafe { SendInput(std::slice::from_ref(i), std::mem::size_of::<INPUT>() as i32) };
        std::thread::sleep(Duration::from_millis(12));
    }
}

fn type_text(text: &str) {
    let mut v = Vec::new();
    for u in text.encode_utf16() {
        v.push(input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE));
        v.push(input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    send(&v);
}

fn chord(m: VIRTUAL_KEY, k: VIRTUAL_KEY) {
    let scan = |vk: VIRTUAL_KEY| unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    send(&[input(m, scan(m), KEYBD_EVENT_FLAGS(0)), input(k, scan(k), KEYBD_EVENT_FLAGS(0)), input(k, scan(k), KEYEVENTF_KEYUP), input(m, scan(m), KEYEVENTF_KEYUP)]);
}

fn text_of(edit: HWND) -> String {
    let mut buf = [0u16; 1024];
    let n = unsafe { SendMessageW(edit, WM_GETTEXT, Some(WPARAM(buf.len())), Some(LPARAM(buf.as_mut_ptr() as isize))) };
    String::from_utf16_lossy(&buf[..n.0 as usize])
}

fn clear(edit: HWND) {
    unsafe { SendMessageW(edit, 0x00B1, Some(WPARAM(0)), Some(LPARAM(-1))) };
    send(&[
        input(VK_ESCAPE, 0x01, KEYBD_EVENT_FLAGS(0)),
        input(VK_ESCAPE, 0x01, KEYEVENTF_KEYUP),
        input(VK_BACK, 0x0E, KEYBD_EVENT_FLAGS(0)),
        input(VK_BACK, 0x0E, KEYEVENTF_KEYUP),
    ]);
    wait_text(edit, "", Duration::from_secs(1));
}

fn wait_text(edit: HWND, want: &str, limit: Duration) -> String {
    let start = Instant::now();
    loop {
        let t = text_of(edit);
        if t == want || start.elapsed() > limit {
            return t;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn main() {
    if std::env::args().any(|a| a == "--edit") {
        return edit_window();
    }
    std::env::set_var("TOLK_ACCEPT_TEST", "1");
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap()).arg("--edit").spawn().unwrap();
    let (top, edit) = loop {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(top) = unsafe { FindWindowW(w!("TolkE2E"), None) } {
            if let Ok(edit) = unsafe { FindWindowExW(Some(top), None, w!("EDIT"), None) } {
                break (top, edit);
            }
        }
    };
    if std::env::args().any(|a| a == "--demo") {
        unsafe {
            let _ = SetForegroundWindow(top);
        }
        std::thread::sleep(Duration::from_millis(300));
        type_text("замок");
        chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
        println!("{:?}", wait_text(edit, "lock", Duration::from_secs(3)));
        std::thread::sleep(Duration::from_secs(3));
        clear(edit);
        type_text("Good morning, friends");
        unsafe {
            SendMessageW(edit, 0x00CF, Some(WPARAM(1)), None);
            SendMessageW(edit, 0x00B1, Some(WPARAM(0)), Some(LPARAM(-1)));
        }
        std::thread::sleep(Duration::from_millis(100));
        chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
        std::thread::sleep(Duration::from_secs(4));
        unsafe {
            let _ = PostMessageW(Some(top), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        let _ = child.wait();
        return;
    }
    let (tx, rx) = crossbeam_channel::unbounded();
    let engine = Engine::start(Config { repeat_ms: 3000, ..Config::default() }, tx);
    std::thread::spawn(move || {
        for ev in rx {
            if !matches!(ev, CoreEvent::Translated { .. }) {
                println!("  событие: {ev:?}");
            }
        }
    });
    std::thread::sleep(Duration::from_millis(500));
    unsafe {
        let _ = SetForegroundWindow(top);
    }
    std::thread::sleep(Duration::from_millis(300));

    let mut ok = 0;
    let mut total = 0;
    let mut check = |name: &str, got: String, want: &str| {
        total += 1;
        let pass = got == want;
        ok += pass as i32;
        println!("{} {name}: {got:?}{}", if pass { "OK  " } else { "FAIL" }, if pass { String::new() } else { format!(" (ждал {want:?})") });
    };

    type_text("замок");
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    check("слово ru→en", wait_text(edit, "lock", Duration::from_secs(3)), "lock");
    std::thread::sleep(Duration::from_millis(300));
    engine.pick(2);
    check("вариант кликом", wait_text(edit, "castle", Duration::from_secs(2)), "castle");
    engine.pick(2);
    std::thread::sleep(Duration::from_millis(200));
    chord(VK_LCONTROL, VIRTUAL_KEY(b'Z' as u16));
    check("Ctrl+Z", wait_text(edit, "замок", Duration::from_secs(2)), "замок");

    clear(edit);
    type_text("Надо открыть замок");
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    wait_text(edit, "Надо открыть lock", Duration::from_secs(3));
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    let got = wait_text(edit, "Надо open the lock", Duration::from_secs(3));
    check("расширение на слово", got.clone(), "Надо open the lock");

    clear(edit);
    type_text("Hello");
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    check("слово en→ru", wait_text(edit, "Привет", Duration::from_secs(3)), "Привет");

    clipboard_win::set_clipboard_string("ДО ТЕСТА").unwrap();
    clear(edit);
    type_text("Good morning, friends");
    unsafe { SendMessageW(edit, 0x00B1, Some(WPARAM(0)), Some(LPARAM(-1))) };
    std::thread::sleep(Duration::from_millis(100));
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    check("выделение en→ru", wait_text(edit, "Доброе утро, друзья", Duration::from_secs(4)), "Доброе утро, друзья");
    chord(VK_LCONTROL, VIRTUAL_KEY(b'Z' as u16));
    check("Ctrl+Z выделения", wait_text(edit, "Good morning, friends", Duration::from_secs(2)), "Good morning, friends");
    check("буфер обмена цел", clipboard_win::get_clipboard_string().unwrap_or_default(), "ДО ТЕСТА");

    clear(edit);
    type_text("Мой дом");
    send(&[input(VK_ESCAPE, 0x01, KEYBD_EVENT_FLAGS(0)), input(VK_ESCAPE, 0x01, KEYEVENTF_KEYUP)]);
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    check("слово перед кареткой без буфера", wait_text(edit, "Мой house", Duration::from_secs(4)), "Мой house");

    clear(edit);
    type_text("123");
    chord(VK_LMENU, VIRTUAL_KEY(b'Q' as u16));
    std::thread::sleep(Duration::from_millis(600));
    check("нечего переводить", text_of(edit), "123");

    println!("итог: {ok}/{total}");
    let held: Vec<VIRTUAL_KEY> = [VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .filter(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
        .collect();
    if !held.is_empty() {
        println!("ВНИМАНИЕ: залипли {held:?} — отпускаю");
        send(&held.iter().map(|&vk| input(vk, 0, KEYEVENTF_KEYUP)).collect::<Vec<_>>());
    }
    unsafe {
        let _ = PostMessageW(Some(top), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    let _ = child.wait();
}
