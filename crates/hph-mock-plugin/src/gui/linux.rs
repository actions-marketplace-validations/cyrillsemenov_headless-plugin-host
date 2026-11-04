use super::*;
use libc::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::ptr;

type Display = c_void;
type Window = c_ulong;
type Drawable = c_ulong;
type GC = *mut c_void;

const EXPOSURE_MASK: c_long = 1 << 15;

#[repr(C)]
#[derive(Clone, Copy)]
struct XEvent {
    pad: [c_ulong; 24],
}

pub struct MockX11State {
    dpy: *mut Display,
    parent_win: Window,
    child_win: Window,
    fd: c_int,
    close_display: unsafe extern "C" fn(*mut Display) -> c_int,
    default_screen: unsafe extern "C" fn(*mut Display) -> c_int,
    default_gc: unsafe extern "C" fn(*mut Display, c_int) -> GC,
    resize_window: unsafe extern "C" fn(*mut Display, Window, c_uint, c_uint) -> c_int,
    destroy_window: unsafe extern "C" fn(*mut Display, Window) -> c_int,
    set_foreground: unsafe extern "C" fn(*mut Display, GC, c_ulong) -> c_int,
    fill_rectangle:
        unsafe extern "C" fn(*mut Display, Drawable, GC, c_int, c_int, c_uint, c_uint) -> c_int,
    flush: unsafe extern "C" fn(*mut Display) -> c_int,
    sync: unsafe extern "C" fn(*mut Display, c_int) -> c_int,
    pending: unsafe extern "C" fn(*mut Display) -> c_int,
    next_event: unsafe extern "C" fn(*mut Display, *mut XEvent) -> c_int,
}

impl MockX11State {
    pub unsafe fn create(
        host: *const clap_host_t,
        parent_win: Window,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let handle = libc::dlopen(b"libX11.so.6\0".as_ptr() as *const c_char, libc::RTLD_LAZY);
        if handle.is_null() {
            return None;
        }
        macro_rules! sym {
            ($name:literal) => {{
                let p = libc::dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char);
                if p.is_null() {
                    return None;
                }
                std::mem::transmute(p)
            }};
        }
        let open_display: unsafe extern "C" fn(*const c_char) -> *mut Display =
            sym!("XOpenDisplay");
        let close_display: unsafe extern "C" fn(*mut Display) -> c_int = sym!("XCloseDisplay");
        let default_screen: unsafe extern "C" fn(*mut Display) -> c_int = sym!("XDefaultScreen");
        let default_gc: unsafe extern "C" fn(*mut Display, c_int) -> GC = sym!("XDefaultGC");
        let connection_number: unsafe extern "C" fn(*mut Display) -> c_int =
            sym!("XConnectionNumber");
        let create_simple_window: unsafe extern "C" fn(
            *mut Display,
            Window,
            c_int,
            c_int,
            c_uint,
            c_uint,
            c_uint,
            c_ulong,
            c_ulong,
        ) -> Window = sym!("XCreateSimpleWindow");
        let select_input: unsafe extern "C" fn(*mut Display, Window, c_long) -> c_int =
            sym!("XSelectInput");
        let map_window: unsafe extern "C" fn(*mut Display, Window) -> c_int = sym!("XMapWindow");
        let resize_window: unsafe extern "C" fn(*mut Display, Window, c_uint, c_uint) -> c_int =
            sym!("XResizeWindow");
        let destroy_window: unsafe extern "C" fn(*mut Display, Window) -> c_int =
            sym!("XDestroyWindow");
        let set_foreground: unsafe extern "C" fn(*mut Display, GC, c_ulong) -> c_int =
            sym!("XSetForeground");
        let fill_rectangle: unsafe extern "C" fn(
            *mut Display,
            Drawable,
            GC,
            c_int,
            c_int,
            c_uint,
            c_uint,
        ) -> c_int = sym!("XFillRectangle");
        let flush: unsafe extern "C" fn(*mut Display) -> c_int = sym!("XFlush");
        let sync: unsafe extern "C" fn(*mut Display, c_int) -> c_int = sym!("XSync");
        let pending: unsafe extern "C" fn(*mut Display) -> c_int = sym!("XPending");
        let next_event: unsafe extern "C" fn(*mut Display, *mut XEvent) -> c_int =
            sym!("XNextEvent");

        let dpy = open_display(ptr::null());
        if dpy.is_null() {
            return None;
        }

        let child_win = create_simple_window(
            dpy,
            parent_win,
            0,
            0,
            width.max(1),
            height.max(1),
            0,
            0,
            0x00192132,
        );
        select_input(dpy, child_win, EXPOSURE_MASK);
        map_window(dpy, child_win);
        let fd = connection_number(dpy);

        if !host.is_null() {
            if let Some(get_ext) = (*host).get_extension {
                let ext_ptr = get_ext(host, CLAP_EXT_POSIX_FD_SUPPORT.as_ptr() as *const c_char)
                    as *const clap_host_posix_fd_support_t;
                if !ext_ptr.is_null() {
                    if let Some(reg_fd) = (*ext_ptr).register_fd {
                        let _ = reg_fd(host, fd, CLAP_POSIX_FD_READ);
                    }
                }
            }
        }

        let st = Self {
            dpy,
            parent_win,
            child_win,
            fd,
            close_display,
            default_screen,
            default_gc,
            resize_window,
            destroy_window,
            set_foreground,
            fill_rectangle,
            flush,
            sync,
            pending,
            next_event,
        };
        st.paint(width, height);
        Some(st)
    }

    pub unsafe fn paint(&self, width: u32, height: u32) {
        let screen = (self.default_screen)(self.dpy);
        let gc = (self.default_gc)(self.dpy, screen);

        for &target in &[self.parent_win, self.child_win] {
            if target == 0 {
                continue;
            }
            // Dark navy background (#192132)
            (self.set_foreground)(self.dpy, gc, 0x00192132);
            (self.fill_rectangle)(self.dpy, target, gc, 0, 0, width.max(1), height.max(1));

            // 3 meter bars matching macOS & Windows: (#2ECC89, #F09932, #EB3B5A)
            // Note: X11 top-down Y coordinates match Win32 GDI:
            // bar 0: x=40..140, y=40..220 (h=180)
            // bar 1: x=180..280, y=40..190 (h=150)
            // bar 2: x=320..420, y=40..160 (h=120)
            let bars: [(c_int, c_int, c_uint, c_uint, c_ulong); 3] = [
                (40, 40, 100, 180, 0x002ECC89),
                (180, 40, 100, 150, 0x00F09932),
                (320, 40, 100, 120, 0x00EB3B5A),
            ];
            for &(x, y, w, h, col) in &bars {
                (self.set_foreground)(self.dpy, gc, col);
                (self.fill_rectangle)(self.dpy, target, gc, x, y, w, h);
            }
        }
        (self.sync)(self.dpy, 0);
    }

    pub unsafe fn on_fd(&self, width: u32, height: u32) {
        while (self.pending)(self.dpy) > 0 {
            let mut ev = XEvent { pad: [0; 24] };
            (self.next_event)(self.dpy, &mut ev);
        }
        self.paint(width, height);
        (self.flush)(self.dpy);
    }

    pub unsafe fn resize(&self, width: u32, height: u32) {
        (self.resize_window)(self.dpy, self.child_win, width.max(1), height.max(1));
        self.paint(width, height);
    }

    pub unsafe fn destroy(&mut self, host: *const clap_host_t) {
        if !host.is_null() {
            if let Some(get_ext) = (*host).get_extension {
                let ext_ptr = get_ext(host, CLAP_EXT_POSIX_FD_SUPPORT.as_ptr() as *const c_char)
                    as *const clap_host_posix_fd_support_t;
                if !ext_ptr.is_null() {
                    if let Some(unreg_fd) = (*ext_ptr).unregister_fd {
                        let _ = unreg_fd(host, self.fd);
                    }
                }
            }
        }
        if self.child_win != 0 {
            (self.destroy_window)(self.dpy, self.child_win);
            self.child_win = 0;
        }
        if !self.dpy.is_null() {
            (self.close_display)(self.dpy);
            self.dpy = ptr::null_mut();
        }
    }
}
