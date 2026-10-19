use crossbeam_channel::{bounded, unbounded, Sender};
use std::cell::{Cell, RefCell};
use std::result::Result::Ok;
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};
use tolk_core::engine::Rect;
use tolk_core::win::hook;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_HINT: u32 = WM_APP + 1;
const FRAME_MS: u32 = 16;
const PARGB: i32 = 0xE200B;
const FADE_MS: f32 = 120.0;
const VARIANTS_FOR: Duration = Duration::from_secs(5);
const NOTICE_FOR: Duration = Duration::from_millis(2500);
const MEDIUM: &[u8] = include_bytes!("../assets/KockersSans-Medium.ttf");
const BOLD: &[u8] = include_bytes!("../assets/KockersSans-Bold.ttf");

pub enum Hint {
    Variants { list: Vec<String>, current: usize, rect: Option<Rect> },
    Waiting { rect: Option<Rect> },
    Notice { text: String, error: bool, rect: Option<Rect> },
    Hide,
}

type Hit = (f32, f32, f32, f32, usize);
type Pick = Box<dyn Fn(usize)>;

thread_local! {
    static HITS: RefCell<Vec<Hit>> = const { RefCell::new(Vec::new()) };
    static PICK: RefCell<Option<Pick>> = const { RefCell::new(None) };
}

pub fn start(on_pick: impl Fn(usize) + Send + 'static) -> Sender<Hint> {
    let (tx, rx) = unbounded::<Hint>();
    let (ready_tx, ready_rx) = bounded::<u32>(1);
    std::thread::Builder::new().name("tolk-hint-ui".into()).spawn(move || ui(Box::new(on_pick), ready_tx)).expect("hint ui thread");
    let tid = ready_rx.recv().unwrap_or(0);
    std::thread::Builder::new()
        .name("tolk-hint".into())
        .spawn(move || {
            for hint in rx {
                let raw = Box::into_raw(Box::new(hint));
                if unsafe { PostThreadMessageW(tid, WM_HINT, WPARAM(0), LPARAM(raw as isize)) }.is_err() {
                    drop(unsafe { Box::from_raw(raw) });
                }
            }
        })
        .expect("hint thread");
    tx
}

fn ui(on_pick: Pick, ready: Sender<u32>) {
    unsafe {
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let _ = ready.send(GetCurrentThreadId());
        PICK.with(|p| *p.borrow_mut() = Some(on_pick));
        let mut token = 0usize;
        let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
        if GdiplusStartup(&mut token, &input, std::ptr::null_mut()) != Status(0) {
            return;
        }
        let Some(mut state) = State::create() else { return };
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HINT && msg.hwnd.is_invalid() {
                let hint = *Box::from_raw(msg.lParam.0 as *mut Hint);
                state.apply(hint);
                continue;
            }
            if msg.message == WM_TIMER && msg.hwnd == state.hwnd {
                state.tick();
                continue;
            }
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            let x = (lp.0 & 0xffff) as i16 as f32;
            let y = ((lp.0 >> 16) & 0xffff) as i16 as f32;
            let hit = HITS.with(|h| h.borrow().iter().find(|b| x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3).map(|b| b.4));
            if let Some(n) = hit {
                PICK.with(|p| {
                    if let Some(f) = p.borrow().as_ref() {
                        f(n);
                    }
                });
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[derive(Clone, Copy)]
struct Theme {
    cosmic: u32,
    ring: u32,
    fg: u32,
    dim: u32,
    strong: u32,
    raised: u32,
    danger: u32,
    tk: u32,
    ink: u32,
    shadow: f32,
}

impl Theme {
    fn current() -> Self {
        if apps_light() {
            Theme { cosmic: 0xffffff, ring: 0xdedee2, fg: 0x17171a, dim: 0x56565e, strong: 0xd2d2d5, raised: 0xebebee, danger: 0xb3261e, tk: 0x9a3fc2, ink: 0xffffff, shadow: 0.14 }
        } else {
            Theme { cosmic: 0x1c1c1f, ring: 0x2e2e33, fg: 0xececef, dim: 0xa8a8af, strong: 0x3c3c3f, raised: 0x242427, danger: 0xff8a80, tk: 0xe9a8ff, ink: 0x2c1038, shadow: 0.34 }
        }
    }
}

fn apps_light() -> bool {
    let mut value = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 1
}

fn animations_off() -> bool {
    let mut on = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&mut on as *mut windows::core::BOOL).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    !on.as_bool()
}

struct Family(*mut GpFontFamily);

impl Family {
    unsafe fn load(data: &'static [u8]) -> Option<Self> {
        let mut coll = std::ptr::null_mut();
        GdipNewPrivateFontCollection(&mut coll);
        if coll.is_null() || GdipPrivateAddMemoryFont(coll, data.as_ptr().cast(), data.len() as i32) != Status(0) {
            return None;
        }
        let mut fams = [std::ptr::null_mut(); 1];
        let mut found = 0;
        GdipGetFontCollectionFamilyList(coll, &mut fams, &mut found);
        (found >= 1 && !fams[0].is_null()).then(|| Family(fams[0]))
    }

    unsafe fn font(&self, px: f32) -> Option<*mut GpFont> {
        for style in [FontStyleRegular.0, FontStyleBold.0] {
            let mut f = std::ptr::null_mut();
            if GdipCreateFont(self.0, px, style, UnitPixel, &mut f) == Status(0) && !f.is_null() {
                return Some(f);
            }
        }
        None
    }
}

struct Fonts {
    med13: *mut GpFont,
    bold13: *mut GpFont,
}

impl Fonts {
    unsafe fn new(med: &Family, bold: &Family, k: f32) -> Option<Self> {
        Some(Fonts { med13: med.font(13.0 * k)?, bold13: bold.font(13.0 * k)? })
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteFont(self.med13);
            GdipDeleteFont(self.bold13);
        }
    }
}

struct Item {
    x: f32,
    w: f32,
    word: Vec<u16>,
}

enum Kind {
    Variants { items: Vec<Item>, seps: Vec<f32>, sep: Vec<u16>, current: usize },
    Waiting,
    Notice { text: Vec<u16>, error: bool },
}

struct Shown {
    kind: Kind,
    start: Instant,
    expire: Option<Duration>,
    k: f32,
    theme: Theme,
    still: bool,
    origin: (i32, i32),
    size: (i32, i32),
    pill: (f32, f32, f32, f32),
    fonts: Fonts,
    last: Cell<Option<(u32, Option<usize>)>>,
}

struct State {
    hwnd: HWND,
    med: Family,
    bold: Family,
    fmt: *mut GpStringFormat,
    meter: *mut GpGraphics,
    cur: Option<Shown>,
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub(crate) type Box4 = (i32, i32, i32, i32);

fn caret() -> Option<Box4> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() {
            return None;
        }
        let tid = GetWindowThreadProcessId(fg, None);
        let mut gi = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        GetGUIThreadInfo(tid, &mut gi).ok()?;
        if gi.hwndCaret.is_invalid() {
            return None;
        }
        let r = gi.rcCaret;
        if r.bottom - r.top <= 0 {
            return None;
        }
        let mut a = POINT { x: r.left, y: r.top };
        let mut b = POINT { x: r.right, y: r.bottom };
        if !ClientToScreen(gi.hwndCaret, &mut a).as_bool() || !ClientToScreen(gi.hwndCaret, &mut b).as_bool() {
            return None;
        }
        Some((a.x, a.y, b.x.max(a.x + 1), b.y))
    }
}

fn mouse() -> Option<Box4> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y, p.x + 1, p.y + 22))
}

fn anchor(rect: Option<Rect>) -> Box4 {
    match rect {
        Some(r) => (r.left, r.top, r.right, r.bottom),
        None => caret().or_else(mouse).unwrap_or((0, 0, 1, 22)),
    }
}

pub(crate) fn metrics(a: Box4) -> (f32, RECT) {
    unsafe {
        let m = MonitorFromPoint(POINT { x: (a.0 + a.2) / 2, y: (a.1 + a.3) / 2 }, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96, 96);
        let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let work = if GetMonitorInfoW(m, &mut mi).as_bool() { mi.rcWork } else { RECT { left: i32::MIN / 2, top: i32::MIN / 2, right: i32::MAX / 2, bottom: i32::MAX / 2 } };
        (dx as f32 / 96.0, work)
    }
}

fn place_above(a: Box4, k: f32, w: f32, h: f32, work: &RECT) -> (f32, f32) {
    let mut x = a.0 as f32 - 8.0 * k;
    let mut y = a.1 as f32 - 8.0 * k - h;
    if y < work.top as f32 {
        y = a.3 as f32 + 8.0 * k;
    }
    if x + w > work.right as f32 {
        x = work.right as f32 - w;
    }
    if x < work.left as f32 {
        x = work.left as f32;
    }
    (x, y)
}

impl State {
    unsafe fn create() -> Option<State> {
        let med = Family::load(MEDIUM)?;
        let bold = Family::load(BOLD)?;
        let module = GetModuleHandleW(None).ok()?;
        let class = w!("TolkHint");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let hwnd = CreateWindowExW(ex, class, PCWSTR::null(), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None).ok()?;
        let mut generic = std::ptr::null_mut();
        GdipStringFormatGetGenericTypographic(&mut generic);
        let mut fmt = std::ptr::null_mut();
        GdipCloneStringFormat(generic, &mut fmt);
        GdipSetStringFormatAlign(fmt, StringAlignmentNear);
        GdipSetStringFormatLineAlign(fmt, StringAlignmentCenter);
        let mut bmp = std::ptr::null_mut();
        GdipCreateBitmapFromScan0(1, 1, 4, PARGB, None, &mut bmp);
        let mut meter = std::ptr::null_mut();
        GdipGetImageGraphicsContext(bmp.cast(), &mut meter);
        GdipSetTextRenderingHint(meter, TextRenderingHintAntiAlias);
        Some(State { hwnd, med, bold, fmt, meter, cur: None })
    }

    unsafe fn width(&self, font: *mut GpFont, s: &[u16]) -> f32 {
        let mut bound = RectF::default();
        let layout = RectF { X: 0.0, Y: 0.0, Width: 10000.0, Height: 1000.0 };
        GdipMeasureString(self.meter, PCWSTR(s.as_ptr()), s.len() as i32, font, &layout, self.fmt, &mut bound, std::ptr::null_mut(), std::ptr::null_mut());
        bound.Width
    }

    fn apply(&mut self, hint: Hint) {
        if let Hint::Hide = hint {
            self.hide();
            return;
        }
        let was_variants = matches!(self.cur.as_ref().map(|s| &s.kind), Some(Kind::Variants { .. }));
        let Some(shown) = (unsafe { self.build(hint) }) else {
            self.hide();
            return;
        };
        let is_variants = matches!(shown.kind, Kind::Variants { .. });
        if was_variants && !is_variants {
            hook::set_hint_rect(None);
            hook::HINT_OPEN.store(false, SeqCst);
        }
        HITS.with(|h| *h.borrow_mut() = Vec::new());
        if let Kind::Variants { items, .. } = &shown.kind {
            let k = shown.k;
            let iy = shown.pill.1 + (shown.pill.3 - 26.0 * k) / 2.0;
            let hits: Vec<Hit> = items.iter().enumerate().map(|(i, it)| (it.x, iy, it.w, 26.0 * k, i + 1)).collect();
            HITS.with(|h| *h.borrow_mut() = hits);
            let l = shown.origin.0 + shown.pill.0 as i32;
            let t = shown.origin.1 + shown.pill.1 as i32;
            hook::set_hint_rect(Some((l, t, l + shown.pill.2.ceil() as i32, t + shown.pill.3.ceil() as i32)));
        }
        self.cur = Some(shown);
        self.render(true);
        unsafe { SetTimer(Some(self.hwnd), 1, FRAME_MS, None) };
    }

    fn hide(&mut self) {
        if let Some(s) = self.cur.take() {
            if matches!(s.kind, Kind::Variants { .. }) {
                hook::set_hint_rect(None);
                hook::HINT_OPEN.store(false, SeqCst);
            }
        }
        HITS.with(|h| *h.borrow_mut() = Vec::new());
        unsafe {
            let _ = KillTimer(Some(self.hwnd), 1);
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn tick(&mut self) {
        let Some(s) = self.cur.as_ref() else {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), 1);
            }
            return;
        };
        if s.expire.is_some_and(|e| s.start.elapsed() >= e) {
            self.hide();
            return;
        }
        self.render(false);
    }

    fn hover(&self, s: &Shown) -> Option<usize> {
        let Kind::Variants { items, .. } = &s.kind else { return None };
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p).ok()? };
        let (x, y) = ((p.x - s.origin.0) as f32, (p.y - s.origin.1) as f32);
        let iy = s.pill.1 + (s.pill.3 - 26.0 * s.k) / 2.0;
        if y < iy || y >= iy + 26.0 * s.k {
            return None;
        }
        items.iter().position(|it| x >= it.x && x < it.x + it.w)
    }

    fn render(&self, force: bool) {
        let Some(s) = self.cur.as_ref() else { return };
        let ms = s.start.elapsed().as_secs_f32() * 1000.0;
        let op = if s.still { 1.0 } else { (ms / FADE_MS).min(1.0) };
        let hover = self.hover(s);
        let sig = ((op * 255.0).round() as u32, hover);
        if !matches!(s.kind, Kind::Waiting) && !force && s.last.get() == Some(sig) {
            return;
        }
        s.last.set(Some(sig));
        let fmt = self.fmt;
        present(self.hwnd, s.origin, s.size, sig.0 as u8, |g| unsafe { s.draw(&Cv { g, fmt }, ms, hover) });
    }

    unsafe fn build(&self, hint: Hint) -> Option<Shown> {
        let still = animations_off();
        let theme = Theme::current();
        let start = Instant::now();
        match hint {
            Hint::Hide => None,
            Hint::Waiting { rect } => {
                let a = anchor(rect);
                let (k, work) = metrics(a);
                let fonts = Fonts::new(&self.med, &self.bold, k)?;
                let pad = (8.0 * k).ceil();
                let (dw, dh) = (26.0 * k, 6.0 * k);
                let mut x = a.2 as f32 + 6.0 * k;
                let y = (a.1 + a.3) as f32 / 2.0 - dh / 2.0;
                x = x.min(work.right as f32 - dw).max(work.left as f32);
                Some(Shown {
                    kind: Kind::Waiting,
                    start,
                    expire: None,
                    k,
                    theme,
                    still,
                    origin: (x.round() as i32 - pad as i32, y.round() as i32 - pad as i32),
                    size: (dw.ceil() as i32 + 2 * pad as i32, dh.ceil() as i32 + 2 * pad as i32),
                    pill: (pad, pad, dw, dh),
                    fonts,
                    last: Cell::new(None),
                })
            }
            Hint::Notice { text, error, rect } => {
                let a = anchor(rect);
                let (k, work) = metrics(a);
                let fonts = Fonts::new(&self.med, &self.bold, k)?;
                let pad = (52.0 * k).ceil();
                let text = utf16(&text);
                let tw = self.width(fonts.med13, &text);
                let (pw, ph) = (7.0 * k + 24.0 * k + 10.0 * k + tw + 16.0 * k, 38.0 * k);
                let (x, y) = place_above(a, k, pw, ph, &work);
                Some(Shown {
                    kind: Kind::Notice { text, error },
                    start,
                    expire: Some(NOTICE_FOR),
                    k,
                    theme,
                    still,
                    origin: (x.round() as i32 - pad as i32, y.round() as i32 - pad as i32),
                    size: (pw.ceil() as i32 + 2 * pad as i32, ph.ceil() as i32 + 2 * pad as i32),
                    pill: (pad, pad, pw, ph),
                    fonts,
                    last: Cell::new(None),
                })
            }
            Hint::Variants { list, current, rect } => {
                if list.is_empty() {
                    return None;
                }
                let a = anchor(rect);
                let (k, work) = metrics(a);
                let fonts = Fonts::new(&self.med, &self.bold, k)?;
                let pad = (52.0 * k).ceil();
                let ph = 36.0 * k;
                let mut x = pad + 6.0 * k;
                let sep = utf16("\u{b7}");
                let sw = self.width(fonts.med13, &sep);
                let mut items = Vec::new();
                let mut seps = Vec::new();
                for (i, word) in list.iter().enumerate() {
                    if i > 0 {
                        x += 2.0 * k;
                        seps.push(x);
                        x += sw + 2.0 * k;
                    }
                    let word = utf16(word);
                    let ww = self.width(if i == current { fonts.bold13 } else { fonts.med13 }, &word);
                    let w = 10.0 * k + ww + 10.0 * k;
                    items.push(Item { x, w, word });
                    x += w;
                }
                let pw = x + 6.0 * k - pad;
                let (sx, sy) = place_above(a, k, pw, ph, &work);
                Some(Shown {
                    kind: Kind::Variants { items, seps, sep, current },
                    start,
                    expire: Some(VARIANTS_FOR),
                    k,
                    theme,
                    still,
                    origin: (sx.round() as i32 - pad as i32, sy.round() as i32 - pad as i32),
                    size: (pw.ceil() as i32 + 2 * pad as i32, ph.ceil() as i32 + 2 * pad as i32),
                    pill: (pad, pad, pw, ph),
                    fonts,
                    last: Cell::new(None),
                })
            }
        }
    }
}

struct Cv {
    g: *mut GpGraphics,
    fmt: *mut GpStringFormat,
}

impl Cv {
    unsafe fn round(&self, color: u32, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let r = r.min(w / 2.0).min(h / 2.0);
        let d = r * 2.0;
        let mut path = std::ptr::null_mut();
        GdipCreatePath(FillModeWinding, &mut path);
        if d > 0.0 {
            GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
            GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
            GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
            GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
        } else {
            let pts = [PointF { X: x, Y: y }, PointF { X: x + w, Y: y }, PointF { X: x + w, Y: y + h }, PointF { X: x, Y: y + h }];
            GdipAddPathPolygon(path, pts.as_ptr(), 4);
        }
        GdipClosePathFigure(path);
        let mut brush = std::ptr::null_mut();
        GdipCreateSolidFill(color, &mut brush);
        GdipFillPath(self.g, brush.cast(), path);
        GdipDeleteBrush(brush.cast());
        GdipDeletePath(path);
    }

    unsafe fn text(&self, font: *mut GpFont, s: &[u16], color: u32, x: f32, y: f32, h: f32) {
        let mut brush = std::ptr::null_mut();
        GdipCreateSolidFill(color, &mut brush);
        let rect = RectF { X: x, Y: y, Width: 4000.0, Height: h };
        GdipDrawString(self.g, PCWSTR(s.as_ptr()), s.len() as i32, font, &rect, self.fmt, brush.cast());
        GdipDeleteBrush(brush.cast());
    }

    unsafe fn shadow(&self, strength: f32, x: f32, y: f32, w: f32, h: f32, k: f32) {
        for i in 0..12 {
            let e = 34.0 * k * ((i as f32 + 0.5) / 12.0 - 0.5);
            let (sw, sh) = (w + 2.0 * e, h + 2.0 * e);
            if sw > 0.0 && sh > 0.0 {
                self.round(argb(0x000000, strength / 12.0), x - e, y + 14.0 * k - e, sw, sh, (h / 2.0 + e).max(0.0));
            }
        }
    }

    unsafe fn frame(&self, t: &Theme, x: f32, y: f32, w: f32, h: f32, k: f32) {
        self.shadow(t.shadow, x, y, w, h, k);
        self.round(argb(t.ring, 1.0), x - k, y - k, w + 2.0 * k, h + 2.0 * k, h / 2.0 + k);
        self.round(argb(t.cosmic, 1.0), x, y, w, h, h / 2.0);
    }

    unsafe fn pen(&self, color: u32, width: f32) -> *mut GpPen {
        let mut pen = std::ptr::null_mut();
        GdipCreatePen1(color, width, UnitPixel, &mut pen);
        GdipSetPenStartCap(pen, LineCapRound);
        GdipSetPenEndCap(pen, LineCapRound);
        GdipSetPenLineJoin(pen, LineJoinRound);
        pen
    }

    unsafe fn dot(&self, color: u32, cx: f32, cy: f32, r: f32) {
        self.round(color, cx - r, cy - r, r * 2.0, r * 2.0, r);
    }

    unsafe fn info(&self, color: u32, cx: f32, cy: f32, k: f32) {
        let s = 14.0 * k / 24.0;
        let m = |vx: f32, vy: f32| (cx + (vx - 12.0) * s, cy + (vy - 12.0) * s);
        let pen = self.pen(color, 1.6 * s);
        let (ox, oy) = m(3.0, 3.0);
        GdipDrawEllipse(self.g, pen, ox, oy, 18.0 * s, 18.0 * s);
        let (x1, y1) = m(12.0, 16.0);
        let (x2, y2) = m(12.0, 12.0);
        GdipDrawLine(self.g, pen, x1, y1, x2, y2);
        GdipDeletePen(pen);
        let (dx, dy) = m(12.0, 8.0);
        self.dot(color, dx, dy, 0.8 * s);
    }

    unsafe fn alert(&self, color: u32, cx: f32, cy: f32, k: f32) {
        let s = 14.0 * k / 24.0;
        let m = |vx: f32, vy: f32| (cx + (vx - 12.0) * s, cy + (vy - 12.0) * s);
        let pen = self.pen(color, 1.6 * s);
        let v = [(12.0f32, 3.5f32), (2.2, 20.5), (21.8, 20.5)];
        let rr = 2.2f32;
        let toward = |c: (f32, f32), p: (f32, f32)| {
            let (dx, dy) = (p.0 - c.0, p.1 - c.1);
            let len = (dx * dx + dy * dy).sqrt();
            m(c.0 + dx / len * rr, c.1 + dy / len * rr)
        };
        let mut path = std::ptr::null_mut();
        GdipCreatePath(FillModeWinding, &mut path);
        for i in 0..3 {
            let (prev, cur, next) = (v[(i + 2) % 3], v[i], v[(i + 1) % 3]);
            let a = toward(cur, prev);
            let b = toward(cur, next);
            let c = m(cur.0, cur.1);
            GdipAddPathBezier(path, a.0, a.1, c.0, c.1, c.0, c.1, b.0, b.1);
        }
        GdipClosePathFigure(path);
        GdipDrawPath(self.g, pen, path);
        GdipDeletePath(path);
        let (x1, y1) = m(12.0, 9.5);
        let (x2, y2) = m(12.0, 13.5);
        GdipDrawLine(self.g, pen, x1, y1, x2, y2);
        GdipDeletePen(pen);
        let (dx, dy) = m(12.0, 17.0);
        self.dot(color, dx, dy, 0.8 * s);
    }
}

impl Shown {
    unsafe fn draw(&self, cv: &Cv, ms: f32, hover: Option<usize>) {
        let (px, py, pw, ph) = self.pill;
        let (k, t, f) = (self.k, &self.theme, &self.fonts);
        match &self.kind {
            Kind::Waiting => {
                let el = ms / 1000.0;
                for i in 0..3 {
                    let p = ((el - 0.15 * i as f32) / 1.2).rem_euclid(1.0);
                    let (o, sc) = pulse(p);
                    let cx = px + 3.0 * k + i as f32 * 10.0 * k;
                    cv.dot(argb(t.tk, o), cx, py + 3.0 * k, 3.0 * k * sc);
                }
            }
            Kind::Variants { items, seps, sep, current } => {
                cv.frame(t, px, py, pw, ph, k);
                for x in seps {
                    cv.text(f.med13, sep, argb(t.strong, 1.0), *x, py, ph);
                }
                let (iy, ih) = (py + (ph - 26.0 * k) / 2.0, 26.0 * k);
                for (i, it) in items.iter().enumerate() {
                    let cur = i == *current;
                    if cur {
                        cv.round(argb(t.tk, 1.0), it.x, iy, it.w, ih, ih / 2.0);
                    } else if hover == Some(i) {
                        cv.round(argb(t.strong, 1.0), it.x, iy, it.w, ih, ih / 2.0);
                    }
                    let (wc, wf) = if cur { (argb(t.ink, 1.0), f.bold13) } else { (argb(t.fg, 1.0), f.med13) };
                    cv.text(wf, &it.word, wc, it.x + 10.0 * k, iy, ih);
                }
            }
            Kind::Notice { text, error } => {
                cv.frame(t, px, py, pw, ph, k);
                let (cx, cy) = (px + 7.0 * k + 12.0 * k, py + ph / 2.0);
                let (bg, ic) = if *error { (argb(t.danger, 0.16), argb(t.danger, 1.0)) } else { (argb(t.raised, 1.0), argb(t.dim, 1.0)) };
                cv.dot(bg, cx, cy, 12.0 * k);
                if *error {
                    cv.alert(ic, cx, cy, k);
                } else {
                    cv.info(ic, cx, cy, k);
                }
                cv.text(f.med13, text, argb(t.fg, 1.0), px + 41.0 * k, py, ph);
            }
        }
    }
}

fn argb(rgb: u32, alpha: f32) -> u32 {
    (((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24) | rgb
}

fn ease_in_out(x: f32) -> f32 {
    let (x1, y1, x2, y2) = (0.42f32, 0.0f32, 0.58f32, 1.0f32);
    let bez = |t: f32, a: f32, b: f32| 3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

fn pulse(p: f32) -> (f32, f32) {
    let (a, b) = if p < 0.35 { ((0.3, 0.7), (1.0, 1.15)) } else { ((1.0, 1.15), (0.3, 0.7)) };
    let local = if p < 0.35 { p / 0.35 } else { (p - 0.35) / 0.65 };
    let e = ease_in_out(local);
    (a.0 + (b.0 - a.0) * e, a.1 + (b.1 - a.1) * e)
}

fn present(hwnd: HWND, origin: (i32, i32), size: (i32, i32), alpha: u8, draw: impl FnOnce(*mut GpGraphics)) {
    let (w, h) = size;
    if w <= 0 || h <= 0 {
        return;
    }
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) else { return };
        let dc = CreateCompatibleDC(None);
        let old = SelectObject(dc, dib.into());
        let mut bmp = std::ptr::null_mut();
        GdipCreateBitmapFromScan0(w, h, w * 4, PARGB, Some(bits.cast()), &mut bmp);
        let mut g = std::ptr::null_mut();
        GdipGetImageGraphicsContext(bmp.cast(), &mut g);
        GdipGraphicsClear(g, 0);
        GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
        GdipSetTextRenderingHint(g, TextRenderingHintAntiAlias);
        draw(g);
        GdipDeleteGraphics(g);
        GdipDisposeImage(bmp.cast());
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: AC_SRC_ALPHA as u8 };
        let pos = POINT { x: origin.0, y: origin.1 };
        let sz = SIZE { cx: w, cy: h };
        let src = POINT::default();
        let _ = UpdateLayeredWindow(hwnd, None, Some(&pos), Some(&sz), Some(dc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);
        if !IsWindowVisible(hwnd).as_bool() {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(dib.into());
    }
}
