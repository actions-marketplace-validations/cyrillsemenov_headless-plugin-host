use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HINSTANCE = *mut c_void;
type WNDPROC = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WM_PAINT: u32 = 0x000F;
const WM_PRINTCLIENT: u32 = 0x0318;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub f_erase: i32,
    pub rc_paint: RECT,
    pub f_restore: i32,
    pub f_inc_update: i32,
    pub rgb_reserved: [u8; 32],
}

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: u32,
    style: u32,
    lpfn_wnd_proc: WNDPROC,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HINSTANCE,
    h_icon: *mut c_void,
    h_cursor: *mut c_void,
    hbr_background: HBRUSH,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
    h_icon_sm: *mut c_void,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: *mut c_void,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DestroyWindow(hWnd: HWND) -> i32;
    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> HBRUSH;
    fn DeleteObject(ho: *mut c_void) -> i32;
}

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

unsafe fn paint_mock_dc(hwnd: HWND, hdc: HDC) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let bg = CreateSolidBrush(rgb(31, 41, 61));
    let _ = FillRect(hdc, &rc, bg);
    let _ = DeleteObject(bg);

    let colors = [rgb(51, 199, 140), rgb(242, 158, 51), rgb(230, 71, 97)];
    for (i, &col) in colors.iter().enumerate() {
        let left = 40 + (i as i32) * 140;
        let bar_h = 180 - (i as i32) * 30;
        let bar_rc = RECT {
            left,
            top: 40,
            right: left + 100,
            bottom: 40 + bar_h,
        };
        let br = CreateSolidBrush(col);
        let _ = FillRect(hdc, &bar_rc, br);
        let _ = DeleteObject(br);
    }
}

unsafe extern "system" fn mock_child_wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                paint_mock_dc(hwnd, hdc);
            }
            let _ = EndPaint(hwnd, &ps);
            0
        }
        WM_PRINTCLIENT => {
            let hdc = w_param as HDC;
            if !hdc.is_null() {
                paint_mock_dc(hwnd, hdc);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

static REGISTER_MOCK_CLASS: Once = Once::new();
const MOCK_CLASS_NAME: &[u16] = &[
    b'H' as u16,
    b'p' as u16,
    b'h' as u16,
    b'M' as u16,
    b'o' as u16,
    b'c' as u16,
    b'k' as u16,
    b'G' as u16,
    b'u' as u16,
    b'i' as u16,
    0,
];

pub unsafe fn create_child_window(parent: HWND, width: u32, height: u32) -> HWND {
    let h_inst = GetModuleHandleW(ptr::null());
    REGISTER_MOCK_CLASS.call_once(|| {
        let wc = WNDCLASSEXW {
            cb_size: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfn_wnd_proc: mock_child_wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: h_inst,
            h_icon: ptr::null_mut(),
            h_cursor: ptr::null_mut(),
            hbr_background: ptr::null_mut(),
            lpsz_menu_name: ptr::null(),
            lpsz_class_name: MOCK_CLASS_NAME.as_ptr(),
            h_icon_sm: ptr::null_mut(),
        };
        let _ = RegisterClassExW(&wc);
    });

    CreateWindowExW(
        0,
        MOCK_CLASS_NAME.as_ptr(),
        MOCK_CLASS_NAME.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        0,
        0,
        width as i32,
        height as i32,
        parent,
        ptr::null_mut(),
        h_inst,
        ptr::null_mut(),
    )
}

pub unsafe fn resize_child_window(hwnd: HWND, width: u32, height: u32) {
    if !hwnd.is_null() {
        let _ = SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            width as i32,
            height as i32,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
