use crate::{save_rgba_png, GuiBenchmarkReport, GuiScreenshotResult};
use hph_core::{HostError, HostResult, PluginInstance};
use std::path::Path;

use std::ffi::c_void;
use std::ptr;
use std::sync::Once;
use std::time::{Duration, Instant};

type HWND = *mut c_void;
type HDC = *mut c_void;
type HBITMAP = *mut c_void;
type HGDIOBJ = *mut c_void;
type HINSTANCE = *mut c_void;
type HICON = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;
type HMENU = *mut c_void;
type WNDPROC = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

const WS_POPUP: u32 = 0x8000_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const SW_SHOWNOACTIVATE: i32 = 4;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const PM_REMOVE: u32 = 0x0001;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;
const RDW_INVALIDATE: u32 = 0x0001;
const RDW_UPDATENOW: u32 = 0x0100;
const RDW_ALLCHILDREN: u32 = 0x0080;

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: u32,
    style: u32,
    lpfn_wnd_proc: WNDPROC,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HINSTANCE,
    h_icon: HICON,
    h_cursor: HCURSOR,
    hbr_background: HBRUSH,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
    h_icon_sm: HICON,
}

#[repr(C)]
#[derive(Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [u32; 1],
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
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    fn DestroyWindow(hWnd: HWND) -> i32;
    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn RedrawWindow(
        hWnd: HWND,
        lprcUpdate: *const RECT,
        hrgnUpdate: *mut c_void,
        flags: u32,
    ) -> i32;
    fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn PeekMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    fn GetDC(hWnd: HWND) -> HDC;
    fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    fn PrintWindow(hwnd: HWND, hdcBlt: HDC, nFlags: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn DeleteDC(hdc: HDC) -> i32;
    fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: u32,
        ppvBits: *mut *mut u8,
        hSection: *mut c_void,
        offset: u32,
    ) -> HBITMAP;
    fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    fn DeleteObject(ho: HGDIOBJ) -> i32;
    fn BitBlt(
        hdc: HDC,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdcSrc: HDC,
        x1: i32,
        y1: i32,
        rop: u32,
    ) -> i32;
}

static REGISTER_HOST_CLASS: Once = Once::new();
const HOST_CLASS_NAME: &[u16] = &[
    b'H' as u16,
    b'p' as u16,
    b'h' as u16,
    b'H' as u16,
    b'o' as u16,
    b's' as u16,
    b't' as u16,
    b'W' as u16,
    b'n' as u16,
    b'd' as u16,
    0,
];

unsafe extern "system" fn host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    DefWindowProcW(hwnd, msg, w_param, l_param)
}

unsafe fn create_offscreen_host_hwnd(width: u32, height: u32) -> HostResult<HWND> {
    let h_instance = GetModuleHandleW(ptr::null());
    REGISTER_HOST_CLASS.call_once(|| {
        let wc = WNDCLASSEXW {
            cb_size: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfn_wnd_proc: host_wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance,
            h_icon: ptr::null_mut(),
            h_cursor: ptr::null_mut(),
            hbr_background: ptr::null_mut(),
            lpsz_menu_name: ptr::null(),
            lpsz_class_name: HOST_CLASS_NAME.as_ptr(),
            h_icon_sm: ptr::null_mut(),
        };
        let _ = RegisterClassExW(&wc);
    });

    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        HOST_CLASS_NAME.as_ptr(),
        HOST_CLASS_NAME.as_ptr(),
        WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
        -10000,
        -10000,
        width as i32,
        height as i32,
        ptr::null_mut(),
        ptr::null_mut(),
        h_instance,
        ptr::null_mut(),
    );
    if hwnd.is_null() {
        return Err(HostError::GuiError("CreateWindowExW returned null".into()));
    }
    Ok(hwnd)
}

unsafe fn pump_win32_messages() {
    let mut msg: MSG = std::mem::zeroed();
    while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
        let _ = TranslateMessage(&msg);
        let _ = DispatchMessageW(&msg);
    }
}

fn pump_runloop_and_plugin(hwnd: HWND, plugin: &mut dyn PluginInstance, duration_ms: u64) {
    let step_ms = 16u64;
    let mut elapsed = 0u64;
    while elapsed < duration_ms {
        let chunk = step_ms.min(duration_ms - elapsed).max(1);
        unsafe {
            pump_win32_messages();
            let _ = RedrawWindow(
                hwnd,
                ptr::null(),
                ptr::null_mut(),
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
            );
            pump_win32_messages();
        }
        std::thread::sleep(Duration::from_millis(chunk));
        plugin.tick_main_thread(chunk);
        elapsed += chunk;
    }
}

unsafe fn capture_hwnd_rgba(
    hwnd: HWND,
    width: u32,
    height: u32,
) -> HostResult<(u32, u32, Vec<u8>)> {
    let win_dc = GetDC(hwnd);
    if win_dc.is_null() {
        return Err(HostError::GuiError("GetDC(hwnd) returned null".into()));
    }
    let mem_dc = CreateCompatibleDC(win_dc);
    if mem_dc.is_null() {
        ReleaseDC(hwnd, win_dc);
        return Err(HostError::GuiError(
            "CreateCompatibleDC returned null".into(),
        ));
    }

    let bmi = BITMAPINFO {
        bmi_header: BITMAPINFOHEADER {
            bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            bi_width: width as i32,
            bi_height: -(height as i32), // top-down DIB
            bi_planes: 1,
            bi_bit_count: 32,
            bi_compression: BI_RGB,
            bi_size_image: width * height * 4,
            bi_x_pels_per_meter: 0,
            bi_y_pels_per_meter: 0,
            bi_clr_used: 0,
            bi_clr_important: 0,
        },
        bmi_colors: [0],
    };

    let mut bits_ptr: *mut u8 = ptr::null_mut();
    let hbm = CreateDIBSection(
        mem_dc,
        &bmi,
        DIB_RGB_COLORS,
        &mut bits_ptr,
        ptr::null_mut(),
        0,
    );
    if hbm.is_null() || bits_ptr.is_null() {
        DeleteDC(mem_dc);
        ReleaseDC(hwnd, win_dc);
        return Err(HostError::GuiError("CreateDIBSection failed".into()));
    }

    let old_obj = SelectObject(mem_dc, hbm);

    let _ = RedrawWindow(
        hwnd,
        ptr::null(),
        ptr::null_mut(),
        RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    pump_win32_messages();

    // Try PrintWindow first, fallback to BitBlt
    if PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT) == 0 {
        if PrintWindow(hwnd, mem_dc, 0) == 0 {
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width as i32,
                height as i32,
                win_dc,
                0,
                0,
                SRCCOPY,
            );
        }
    }

    let num_bytes = (width * height * 4) as usize;
    let bgra = std::slice::from_raw_parts(bits_ptr, num_bytes);
    let mut rgba = vec![0u8; num_bytes];
    for i in (0..num_bytes).step_by(4) {
        rgba[i] = bgra[i + 2]; // R
        rgba[i + 1] = bgra[i + 1]; // G
        rgba[i + 2] = bgra[i]; // B
        rgba[i + 3] = 255; // A
    }

    SelectObject(mem_dc, old_obj);
    DeleteObject(hbm);
    DeleteDC(mem_dc);
    ReleaseDC(hwnd, win_dc);

    Ok((width, height, rgba))
}

pub fn capture_plugin_screenshot(
    plugin: &mut dyn PluginInstance,
    output_path: &Path,
    warmup_ms: u64,
    custom_size: Option<(u32, u32)>,
) -> HostResult<GuiScreenshotResult> {
    if !plugin.supports_gui() {
        return Err(HostError::Unsupported(
            "Plugin does not support embedded Win32 GUI".into(),
        ));
    }

    let t_create = Instant::now();
    plugin.gui_create()?;
    let create_time_us = t_create.elapsed().as_secs_f64() * 1e6;

    let (mut width, mut height) = plugin.gui_get_size().unwrap_or((640, 480));
    if let Some((req_w, req_h)) = custom_size {
        if plugin.gui_can_resize() {
            let (adj_w, adj_h) = plugin
                .gui_adjust_size(req_w, req_h)
                .unwrap_or((req_w, req_h));
            if plugin.gui_set_size(adj_w, adj_h).is_ok() {
                width = adj_w;
                height = adj_h;
            }
        }
    }
    width = width.max(64);
    height = height.max(64);

    unsafe {
        let hwnd = create_offscreen_host_hwnd(width, height)?;
        plugin.gui_set_parent(hwnd)?;

        let t_show = Instant::now();
        plugin.gui_show()?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
        let show_time_us = t_show.elapsed().as_secs_f64() * 1e6;

        pump_runloop_and_plugin(hwnd, plugin, warmup_ms.max(32));

        let t_cap = Instant::now();
        let (cap_w, cap_h, rgba) = capture_hwnd_rgba(hwnd, width, height)?;
        let capture_time_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let non_blank_pixels = rgba
            .chunks_exact(4)
            .filter(|px| px[0] > 0 || px[1] > 0 || px[2] > 0)
            .count();

        save_rgba_png(output_path, cap_w, cap_h, &rgba)?;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        let _ = DestroyWindow(hwnd);

        Ok(GuiScreenshotResult {
            width: cap_w,
            height: cap_h,
            output_path: output_path.display().to_string(),
            create_time_us,
            show_time_us,
            warmup_ms,
            capture_time_us,
            non_blank_pixels,
        })
    }
}

pub fn benchmark_plugin_gui(
    plugin: &mut dyn PluginInstance,
    frames_to_pump: usize,
) -> HostResult<GuiBenchmarkReport> {
    if !plugin.supports_gui() {
        return Ok(GuiBenchmarkReport {
            supported: false,
            initial_size: (0, 0),
            can_resize: false,
            create_latency_us: 0.0,
            set_parent_latency_us: 0.0,
            show_latency_us: 0.0,
            avg_frame_pump_us: 0.0,
            max_frame_pump_us: 0.0,
            resize_avg_latency_us: None,
            capture_rgba_latency_us: 0.0,
        });
    }

    let t0 = Instant::now();
    plugin.gui_create()?;
    let create_latency_us = t0.elapsed().as_secs_f64() * 1e6;

    let (w, h) = plugin.gui_get_size().unwrap_or((640, 480));
    let can_resize = plugin.gui_can_resize();

    unsafe {
        let hwnd = create_offscreen_host_hwnd(w.max(64), h.max(64))?;

        let t_parent = Instant::now();
        plugin.gui_set_parent(hwnd)?;
        let set_parent_latency_us = t_parent.elapsed().as_secs_f64() * 1e6;

        let t_show = Instant::now();
        plugin.gui_show()?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
        let show_latency_us = t_show.elapsed().as_secs_f64() * 1e6;

        let n_frames = frames_to_pump.max(10);
        let mut total_frame_us = 0.0;
        let mut max_frame_us = 0.0f64;
        for _ in 0..n_frames {
            let tf = Instant::now();
            pump_win32_messages();
            plugin.tick_main_thread(16);
            let _ = RedrawWindow(
                hwnd,
                ptr::null(),
                ptr::null_mut(),
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
            );
            let us = tf.elapsed().as_secs_f64() * 1e6;
            total_frame_us += us;
            max_frame_us = max_frame_us.max(us);
        }

        let resize_avg_latency_us = if can_resize {
            let sizes = [(w + 100, h + 60), (w + 200, h + 120), (w, h)];
            let mut sum_us = 0.0;
            for &(rw, rh) in &sizes {
                let (aw, ah) = plugin.gui_adjust_size(rw, rh).unwrap_or((rw, rh));
                let tr = Instant::now();
                let _ = plugin.gui_set_size(aw, ah);
                let _ = SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    0,
                    0,
                    aw as i32,
                    ah as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                sum_us += tr.elapsed().as_secs_f64() * 1e6;
            }
            Some(sum_us / (sizes.len() as f64))
        } else {
            None
        };

        let t_cap = Instant::now();
        let _ = capture_hwnd_rgba(hwnd, w.max(64), h.max(64))?;
        let capture_rgba_latency_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        let _ = DestroyWindow(hwnd);

        Ok(GuiBenchmarkReport {
            supported: true,
            initial_size: (w, h),
            can_resize,
            create_latency_us,
            set_parent_latency_us,
            show_latency_us,
            avg_frame_pump_us: total_frame_us / (n_frames as f64),
            max_frame_pump_us: max_frame_us,
            resize_avg_latency_us,
            capture_rgba_latency_us,
        })
    }
}
