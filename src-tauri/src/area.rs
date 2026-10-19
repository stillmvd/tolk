use crate::hint::metrics;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use tauri::AppHandle;
use tolk_core::engine::Rect;
use tolk_core::win::{keys, ocr};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;

static ACTIVE: AtomicBool = AtomicBool::new(false);
const ACCENT: COLORREF = COLORREF(0x00ffa8e9);
const PILL: COLORREF = COLORREF(0x001f1c1c);
const INK: COLORREF = COLORREF(0x00efecec);
const DIM: u8 = 120;
const MIN_SIDE: i32 = 6;

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

pub fn start(app: AppHandle) {
    if ACTIVE.swap(true, SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("tolk-area".into()).spawn(move || {
        let _done = Release;
        if let Some((near, pixels, w, h)) = unsafe { select() } {
            match ocr::recognize(&pixels, w, h) {
                Ok(text) => crate::ui::open_translator(&app, Some(text), Some(near)),
                Err(e) => crate::notice(e.message(), e != ocr::OcrError::Empty, Some(near)),
            }
        }
    });
    if spawned.is_err() {
        ACTIVE.store(false, SeqCst);
    }
}

struct Release;

impl Drop for Release {
    fn drop(&mut self) {
        ACTIVE.store(false, SeqCst);
    }
}

struct Layer {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
}

impl Layer {
    unsafe fn new(screen: HDC, w: i32, h: i32) -> Self {
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp.into());
        Layer { dc, bmp, old }
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bmp.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

struct Overlay {
    hwnd: HWND,
    size: (i32, i32),
    shot: Layer,
    dark: Layer,
    back: Layer,
    font: HFONT,
    k: f32,
    start: Option<POINT>,
    cur: POINT,
    done: Option<Option<RECT>>,
}

impl Overlay {
    fn sel(&self) -> Option<RECT> {
        let s = self.start?;
        Some(RECT { left: s.x.min(self.cur.x), top: s.y.min(self.cur.y), right: s.x.max(self.cur.x), bottom: s.y.max(self.cur.y) })
    }

    fn extent(&self) -> Option<RECT> {
        let r = self.sel()?;
        let m = (56.0 * self.k) as i32;
        Some(RECT { left: r.left - m, top: r.top - m, right: r.right.max(r.left + 3 * m) + m, bottom: r.bottom + m })
    }

    unsafe fn update(&mut self, change: impl FnOnce(&mut Self)) {
        let before = self.extent();
        change(self);
        let after = self.extent();
        for r in [before, after].into_iter().flatten() {
            let _ = InvalidateRect(Some(self.hwnd), Some(&r), false);
        }
    }

    unsafe fn paint(&self, hdc: HDC, rc: RECT) {
        let b = self.back.dc;
        let blit = |dst: HDC, r: &RECT, src: HDC| {
            let _ = BitBlt(dst, r.left, r.top, r.right - r.left, r.bottom - r.top, Some(src), r.left, r.top, SRCCOPY);
        };
        blit(b, &rc, self.dark.dc);
        if let Some(sel) = self.sel() {
            let mut inner = RECT::default();
            if IntersectRect(&mut inner, &sel, &rc).as_bool() {
                blit(b, &inner, self.shot.dc);
            }
            let t = (2.0 * self.k).round().max(1.0) as i32;
            let brush = CreateSolidBrush(ACCENT);
            for edge in [
                RECT { left: sel.left - t, top: sel.top - t, right: sel.right + t, bottom: sel.top },
                RECT { left: sel.left - t, top: sel.bottom, right: sel.right + t, bottom: sel.bottom + t },
                RECT { left: sel.left - t, top: sel.top, right: sel.left, bottom: sel.bottom },
                RECT { left: sel.right, top: sel.top, right: sel.right + t, bottom: sel.bottom },
            ] {
                FillRect(b, &edge, brush);
            }
            let _ = DeleteObject(brush.into());
            self.label(b, sel);
        }
        blit(hdc, &rc, b);
    }

    unsafe fn label(&self, dc: HDC, sel: RECT) {
        let text: Vec<u16> = format!("{} × {}", sel.right - sel.left, sel.bottom - sel.top).encode_utf16().collect();
        let old = SelectObject(dc, self.font.into());
        let mut sz = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &text, &mut sz);
        let (px, py) = ((10.0 * self.k) as i32, (5.0 * self.k) as i32);
        let (w, h) = (sz.cx + 2 * px, sz.cy + 2 * py);
        let gap = (8.0 * self.k) as i32;
        let mut y = sel.bottom + gap;
        if y + h > self.size.1 {
            y = (sel.top - gap - h).max(0);
        }
        let x = sel.left.min(self.size.0 - w).max(0);
        let rgn = CreateRoundRectRgn(x, y, x + w + 1, y + h + 1, h, h);
        let brush = CreateSolidBrush(PILL);
        let _ = FillRgn(dc, rgn, brush);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(rgn.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, INK);
        let _ = TextOutW(dc, x + px, y + py, &text);
        SelectObject(dc, old);
    }
}

unsafe fn select() -> Option<(Rect, Vec<u8>, u32, u32)> {
    let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
    let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
    let screen = GetDC(None);
    let shot = Layer::new(screen, w, h);
    let _ = BitBlt(shot.dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT);
    let dark = Layer::new(screen, w, h);
    let _ = BitBlt(dark.dc, 0, 0, w, h, Some(shot.dc), 0, 0, SRCCOPY);
    let black = Layer::new(screen, 1, 1);
    let _ = PatBlt(black.dc, 0, 0, 1, 1, BLACKNESS);
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: DIM, AlphaFormat: 0 };
    let _ = AlphaBlend(dark.dc, 0, 0, w, h, black.dc, 0, 0, 1, 1, blend);
    let back = Layer::new(screen, w, h);
    ReleaseDC(None, screen);

    let module = GetModuleHandleW(None).ok()?;
    let class = w!("TolkArea");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(proc),
        hInstance: module.into(),
        lpszClassName: class,
        hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassExW(&wc);
    let hwnd = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, class, PCWSTR::null(), WS_POPUP, x, y, w, h, None, None, Some(module.into()), None).ok()?;
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let (k, _) = metrics((cursor.x, cursor.y, cursor.x, cursor.y));
    let font = CreateFontW(-(13.0 * k).round() as i32, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"));
    OVERLAY.with(|o| *o.borrow_mut() = Some(Overlay { hwnd, size: (w, h), shot, dark, back, font, k, start: None, cur: POINT::default(), done: None }));

    let _ = ShowWindow(hwnd, SW_SHOW);
    keys::suppress_menu();
    let _ = SetForegroundWindow(hwnd);
    let _ = SetFocus(Some(hwnd));
    let _ = UpdateWindow(hwnd);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
        if OVERLAY.with(|o| o.borrow().as_ref().is_some_and(|o| o.done.is_some())) {
            break;
        }
    }
    let _ = ShowWindow(hwnd, SW_HIDE);
    let ov = OVERLAY.with(|o| o.borrow_mut().take())?;
    let _ = DestroyWindow(hwnd);
    let _ = DeleteObject(ov.font.into());
    let sel = ov.done.flatten()?;
    let (pixels, cw, ch) = crop(&ov.shot, sel)?;
    Some((Rect { left: sel.left + x, top: sel.top + y, right: sel.right + x, bottom: sel.bottom + y }, pixels, cw, ch))
}

unsafe fn crop(shot: &Layer, r: RECT) -> Option<(Vec<u8>, u32, u32)> {
    let max = ocr::max_side() as f64;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let s = 2f64.min(max / w as f64).min(max / h as f64);
    let (cw, ch) = (((w as f64 * s) as i32).max(1), ((h as f64 * s) as i32).max(1));
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: cw, biHeight: -ch, biPlanes: 1, biBitCount: 32, ..Default::default() },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let dib = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
    let dc = CreateCompatibleDC(None);
    let old = SelectObject(dc, dib.into());
    SetStretchBltMode(dc, HALFTONE);
    let _ = SetBrushOrgEx(dc, 0, 0, None);
    let _ = StretchBlt(dc, 0, 0, cw, ch, Some(shot.dc), r.left, r.top, w, h, SRCCOPY);
    let _ = GdiFlush();
    let mut pixels = std::slice::from_raw_parts(bits as *const u8, (cw * ch * 4) as usize).to_vec();
    pixels.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    SelectObject(dc, old);
    let _ = DeleteDC(dc);
    let _ = DeleteObject(dib.into());
    Some((pixels, cw as u32, ch as u32))
}

fn point(lp: LPARAM) -> POINT {
    POINT { x: (lp.0 & 0xffff) as i16 as i32, y: ((lp.0 >> 16) & 0xffff) as i16 as i32 }
}

unsafe fn with(f: impl FnOnce(&mut Overlay)) {
    OVERLAY.with(|o| {
        if let Some(o) = o.borrow_mut().as_mut() {
            f(o);
        }
    });
}

fn cancel(o: &mut Overlay) {
    o.done.get_or_insert(None);
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            OVERLAY.with(|o| {
                if let Some(o) = o.borrow().as_ref() {
                    o.paint(hdc, ps.rcPaint);
                }
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            SetCapture(hwnd);
            with(|o| o.update(|o| (o.start, o.cur) = (Some(point(lp)), point(lp))));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            with(|o| {
                if o.start.is_some() {
                    o.update(|o| o.cur = point(lp));
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            with(|o| {
                o.update(|o| o.cur = point(lp));
                match o.sel() {
                    Some(r) if r.right - r.left >= MIN_SIDE && r.bottom - r.top >= MIN_SIDE => o.done = Some(Some(r)),
                    _ => o.update(|o| o.start = None),
                }
            });
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            with(cancel);
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 == VK_ESCAPE.0 as usize => {
            with(cancel);
            LRESULT(0)
        }
        WM_ACTIVATEAPP if wp.0 == 0 => {
            with(cancel);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
