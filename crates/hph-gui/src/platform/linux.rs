use crate::{save_rgba_png, GuiBenchmarkReport, GuiScreenshotResult};
use hph_core::{HostError, HostResult, PluginInstance};
use std::path::Path;

use libc::{c_char, c_int, c_uint, c_ulong, c_void};
use std::ptr;
use std::time::{Duration, Instant};

type Display = c_void;
type Window = c_ulong;
type Drawable = c_ulong;
type XErrorHandler = unsafe extern "C" fn(*mut Display, *mut c_void) -> c_int;

const ALL_PLANES: c_ulong = !0;
const ZPIXMAP: c_int = 2;

#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut u8,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    obdata: *mut c_void,
    create_image: *mut c_void,
    destroy_image: Option<unsafe extern "C" fn(*mut XImage) -> c_int>,
    get_pixel: Option<unsafe extern "C" fn(*mut XImage, c_int, c_int) -> c_ulong>,
    put_pixel: *mut c_void,
    sub_image: *mut c_void,
    add_pixel: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XEvent {
    pad: [c_ulong; 24],
}

struct XlibFns {
    _handle: *mut c_void,
    open_display: unsafe extern "C" fn(*const c_char) -> *mut Display,
    close_display: unsafe extern "C" fn(*mut Display) -> c_int,
    default_screen: unsafe extern "C" fn(*mut Display) -> c_int,
    root_window: unsafe extern "C" fn(*mut Display, c_int) -> Window,
    create_simple_window: unsafe extern "C" fn(
        *mut Display,
        Window,
        c_int,
        c_int,
        c_uint,
        c_uint,
        c_uint,
        c_ulong,
        c_ulong,
    ) -> Window,
    map_window: unsafe extern "C" fn(*mut Display, Window) -> c_int,
    map_subwindows: unsafe extern "C" fn(*mut Display, Window) -> c_int,
    resize_window: unsafe extern "C" fn(*mut Display, Window, c_uint, c_uint) -> c_int,
    destroy_window: unsafe extern "C" fn(*mut Display, Window) -> c_int,
    flush: unsafe extern "C" fn(*mut Display) -> c_int,
    sync: unsafe extern "C" fn(*mut Display, c_int) -> c_int,
    pending: unsafe extern "C" fn(*mut Display) -> c_int,
    next_event: unsafe extern "C" fn(*mut Display, *mut XEvent) -> c_int,
    clear_area:
        unsafe extern "C" fn(*mut Display, Window, c_int, c_int, c_uint, c_uint, c_int) -> c_int,
    query_tree: unsafe extern "C" fn(
        *mut Display,
        Window,
        *mut Window,
        *mut Window,
        *mut *mut Window,
        *mut c_uint,
    ) -> c_int,
    free: unsafe extern "C" fn(*mut c_void) -> c_int,
    get_image: unsafe extern "C" fn(
        *mut Display,
        Drawable,
        c_int,
        c_int,
        c_uint,
        c_uint,
        c_ulong,
        c_int,
    ) -> *mut XImage,
    set_error_handler: unsafe extern "C" fn(XErrorHandler) -> *mut c_void,
}

unsafe extern "C" fn ignore_x11_error(_dpy: *mut Display, _err: *mut c_void) -> c_int {
    0
}

impl XlibFns {
    unsafe fn load() -> HostResult<Self> {
        let handle = libc::dlopen(b"libX11.so.6\0".as_ptr() as *const c_char, libc::RTLD_LAZY);
        if handle.is_null() {
            return Err(HostError::GuiError(
                "Failed to dlopen libX11.so.6 (is X11 installed?)".into(),
            ));
        }
        macro_rules! sym {
            ($name:literal) => {{
                let p = libc::dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char);
                if p.is_null() {
                    return Err(HostError::GuiError(format!("Missing X11 symbol {}", $name)));
                }
                std::mem::transmute(p)
            }};
        }
        Ok(Self {
            _handle: handle,
            open_display: sym!("XOpenDisplay"),
            close_display: sym!("XCloseDisplay"),
            default_screen: sym!("XDefaultScreen"),
            root_window: sym!("XRootWindow"),
            create_simple_window: sym!("XCreateSimpleWindow"),
            map_window: sym!("XMapWindow"),
            map_subwindows: sym!("XMapSubwindows"),
            resize_window: sym!("XResizeWindow"),
            destroy_window: sym!("XDestroyWindow"),
            flush: sym!("XFlush"),
            sync: sym!("XSync"),
            pending: sym!("XPending"),
            next_event: sym!("XNextEvent"),
            clear_area: sym!("XClearArea"),
            query_tree: sym!("XQueryTree"),
            free: sym!("XFree"),
            get_image: sym!("XGetImage"),
            set_error_handler: sym!("XSetErrorHandler"),
        })
    }
}

unsafe fn trigger_expose_recursive(x11: &XlibFns, dpy: *mut Display, win: Window) {
    (x11.map_subwindows)(dpy, win);
    (x11.clear_area)(dpy, win, 0, 0, 0, 0, 1);
    let mut root_ret: Window = 0;
    let mut parent_ret: Window = 0;
    let mut children: *mut Window = ptr::null_mut();
    let mut nchildren: c_uint = 0;
    if (x11.query_tree)(
        dpy,
        win,
        &mut root_ret,
        &mut parent_ret,
        &mut children,
        &mut nchildren,
    ) != 0
        && !children.is_null()
    {
        let slice = std::slice::from_raw_parts(children, nchildren as usize);
        for &child in slice {
            (x11.map_window)(dpy, child);
            (x11.clear_area)(dpy, child, 0, 0, 0, 0, 1);
        }
        (x11.free)(children as *mut c_void);
    }
}

unsafe fn pump_x11_and_plugin(
    x11: &XlibFns,
    dpy: *mut Display,
    win: Window,
    plugin: &mut dyn PluginInstance,
    duration_ms: u64,
) {
    let step_ms = 16u64;
    let mut elapsed = 0u64;
    trigger_expose_recursive(x11, dpy, win);
    (x11.sync)(dpy, 0);
    while elapsed < duration_ms {
        let chunk = step_ms.min(duration_ms - elapsed).max(1);
        while (x11.pending)(dpy) > 0 {
            let mut ev = XEvent { pad: [0; 24] };
            (x11.next_event)(dpy, &mut ev);
        }
        plugin.tick_main_thread(chunk);
        (x11.flush)(dpy);
        std::thread::sleep(Duration::from_millis(chunk.min(4)));
        elapsed += chunk;
    }
    (x11.sync)(dpy, 0);
}

unsafe fn capture_x11_rgba(
    x11: &XlibFns,
    dpy: *mut Display,
    win: Window,
    width: u32,
    height: u32,
) -> HostResult<(u32, u32, Vec<u8>)> {
    let w = width.max(1);
    let h = height.max(1);
    (x11.map_subwindows)(dpy, win);
    (x11.sync)(dpy, 0);

    // If the plugin created a child window filling `win`, read from the child window directly
    // (or `win` if no child window exists).
    let mut target_win = win;
    let mut root_ret: Window = 0;
    let mut parent_ret: Window = 0;
    let mut children: *mut Window = ptr::null_mut();
    let mut nchildren: c_uint = 0;
    if (x11.query_tree)(
        dpy,
        win,
        &mut root_ret,
        &mut parent_ret,
        &mut children,
        &mut nchildren,
    ) != 0
        && !children.is_null()
    {
        if nchildren > 0 {
            let slice = std::slice::from_raw_parts(children, nchildren as usize);
            target_win = slice[nchildren as usize - 1];
        }
        (x11.free)(children as *mut c_void);
    }

    let img = (x11.get_image)(dpy, target_win, 0, 0, w, h, ALL_PLANES, ZPIXMAP);
    if img.is_null() {
        return Err(HostError::GuiError(
            "XGetImage returned null (ensure Xvfb or X11 display is active)".into(),
        ));
    }

    let img_w = ((*img).width as u32).min(w);
    let img_h = ((*img).height as u32).min(h);
    let bpp = (*img).bits_per_pixel;
    let stride = (*img).bytes_per_line as usize;
    let data_ptr = (*img).data;

    let mut rgba = vec![0u8; (img_w * img_h * 4) as usize];
    if !data_ptr.is_null() && bpp == 32 {
        for y in 0..(img_h as usize) {
            let row = std::slice::from_raw_parts(data_ptr.add(y * stride), stride);
            for x in 0..(img_w as usize) {
                let s = x * 4;
                let d = (y * (img_w as usize) + x) * 4;
                rgba[d] = row[s + 2]; // R
                rgba[d + 1] = row[s + 1]; // G
                rgba[d + 2] = row[s]; // B
                rgba[d + 3] = 255; // A
            }
        }
    } else if let Some(get_pixel) = (*img).get_pixel {
        for y in 0..(img_h as usize) {
            for x in 0..(img_w as usize) {
                let px = get_pixel(img, x as c_int, y as c_int);
                let d = (y * (img_w as usize) + x) * 4;
                rgba[d] = ((px >> 16) & 0xff) as u8;
                rgba[d + 1] = ((px >> 8) & 0xff) as u8;
                rgba[d + 2] = (px & 0xff) as u8;
                rgba[d + 3] = 255;
            }
        }
    }

    if let Some(destroy) = (*img).destroy_image {
        destroy(img);
    }

    Ok((img_w, img_h, rgba))
}

pub fn capture_plugin_screenshot(
    plugin: &mut dyn PluginInstance,
    output_path: &Path,
    warmup_ms: u64,
    custom_size: Option<(u32, u32)>,
) -> HostResult<GuiScreenshotResult> {
    if !plugin.supports_gui() {
        return Err(HostError::Unsupported(
            "Plugin does not support embedded X11 GUI".into(),
        ));
    }

    unsafe {
        let x11 = XlibFns::load()?;
        (x11.set_error_handler)(ignore_x11_error);
        let dpy = (x11.open_display)(ptr::null());
        if dpy.is_null() {
            return Err(HostError::GuiError(
                "XOpenDisplay(NULL) failed (set DISPLAY or run under `xvfb-run -a`)".into(),
            ));
        }

        let t0 = Instant::now();
        plugin.gui_create()?;
        let create_time_us = t0.elapsed().as_secs_f64() * 1e6;

        let (mut w, mut h) = plugin.gui_get_size().unwrap_or((640, 480));
        if let Some((cw, ch)) = custom_size {
            if plugin.gui_can_resize() {
                let (adj_w, adj_h) = plugin.gui_adjust_size(cw, ch).unwrap_or((cw, ch));
                if plugin.gui_set_size(adj_w, adj_h).is_ok() {
                    w = adj_w;
                    h = adj_h;
                }
            }
        }

        let screen = (x11.default_screen)(dpy);
        let root = (x11.root_window)(dpy, screen);
        let win =
            (x11.create_simple_window)(dpy, root, 0, 0, w.max(64), h.max(64), 0, 0, 0x00192132);
        (x11.map_window)(dpy, win);
        (x11.sync)(dpy, 0);

        let t_show = Instant::now();
        if let Err(e) = plugin.gui_set_parent(win as usize as *mut c_void) {
            plugin.gui_destroy();
            (x11.destroy_window)(dpy, win);
            (x11.close_display)(dpy);
            return Err(e);
        }
        let _ = plugin.gui_show();
        (x11.map_subwindows)(dpy, win);
        (x11.sync)(dpy, 0);
        let show_time_us = t_show.elapsed().as_secs_f64() * 1e6;

        if let Some((nw, nh)) = plugin.gui_get_size() {
            if nw > 0 && nh > 0 && (nw != w || nh != h) {
                w = nw;
                h = nh;
                (x11.resize_window)(dpy, win, w, h);
            }
        }

        pump_x11_and_plugin(&x11, dpy, win, plugin, warmup_ms.max(32));

        let t_cap = Instant::now();
        let (cap_w, cap_h, rgba) = capture_x11_rgba(&x11, dpy, win, w, h)?;
        let capture_time_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        (x11.destroy_window)(dpy, win);
        (x11.close_display)(dpy);

        let non_blank_pixels = rgba
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count();

        save_rgba_png(output_path, cap_w, cap_h, &rgba)?;

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

    unsafe {
        let x11 = XlibFns::load()?;
        (x11.set_error_handler)(ignore_x11_error);
        let dpy = (x11.open_display)(ptr::null());
        if dpy.is_null() {
            return Err(HostError::GuiError(
                "XOpenDisplay(NULL) failed (set DISPLAY or run under `xvfb-run -a`)".into(),
            ));
        }

        let t_create = Instant::now();
        plugin.gui_create()?;
        let create_latency_us = t_create.elapsed().as_secs_f64() * 1e6;

        let (w, h) = plugin.gui_get_size().unwrap_or((640, 480));
        let can_resize = plugin.gui_can_resize();

        let screen = (x11.default_screen)(dpy);
        let root = (x11.root_window)(dpy, screen);
        let win =
            (x11.create_simple_window)(dpy, root, 0, 0, w.max(64), h.max(64), 0, 0, 0x00192132);
        (x11.map_window)(dpy, win);
        (x11.sync)(dpy, 0);

        let t_parent = Instant::now();
        if let Err(e) = plugin.gui_set_parent(win as usize as *mut c_void) {
            plugin.gui_destroy();
            (x11.destroy_window)(dpy, win);
            (x11.close_display)(dpy);
            return Err(e);
        }
        let set_parent_latency_us = t_parent.elapsed().as_secs_f64() * 1e6;

        let t_show = Instant::now();
        let _ = plugin.gui_show();
        (x11.map_subwindows)(dpy, win);
        (x11.sync)(dpy, 0);
        let show_latency_us = t_show.elapsed().as_secs_f64() * 1e6;

        let n_frames = frames_to_pump.max(1);
        let mut total_frame_us = 0.0;
        let mut max_frame_us = 0.0f64;

        for _ in 0..n_frames {
            let tf = Instant::now();
            trigger_expose_recursive(&x11, dpy, win);
            (x11.flush)(dpy);
            while (x11.pending)(dpy) > 0 {
                let mut ev = XEvent { pad: [0; 24] };
                (x11.next_event)(dpy, &mut ev);
            }
            plugin.tick_main_thread(16);
            let elapsed_us = tf.elapsed().as_secs_f64() * 1e6;
            total_frame_us += elapsed_us;
            if elapsed_us > max_frame_us {
                max_frame_us = elapsed_us;
            }
        }

        let resize_avg_latency_us = if can_resize {
            let sizes = [(w + 100, h + 60), (w + 200, h + 120), (w, h)];
            let mut sum_us = 0.0;
            for &(rw, rh) in &sizes {
                let (aw, ah) = plugin.gui_adjust_size(rw, rh).unwrap_or((rw, rh));
                let tr = Instant::now();
                let _ = plugin.gui_set_size(aw, ah);
                (x11.resize_window)(dpy, win, aw, ah);
                sum_us += tr.elapsed().as_secs_f64() * 1e6;
            }
            Some(sum_us / (sizes.len() as f64))
        } else {
            None
        };

        let t_cap = Instant::now();
        let _ = capture_x11_rgba(&x11, dpy, win, w.max(64), h.max(64))?;
        let capture_rgba_latency_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        (x11.destroy_window)(dpy, win);
        (x11.close_display)(dpy);

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
