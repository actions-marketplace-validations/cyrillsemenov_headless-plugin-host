use crate::{save_rgba_png, GuiBenchmarkReport, GuiScreenshotResult};
use hph_core::{HostError, HostResult, PluginInstance};
use std::path::Path;

use objc2::msg_send_id;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBitmapImageRep, NSColor,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSDate, NSPoint, NSRect, NSSize};
use std::ffi::c_void;
use std::time::{Duration, Instant};

fn ensure_headless_nsapp(mtm: MainThreadMarker) -> Retained<NSApplication> {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app
}

fn pump_runloop_and_plugin(plugin: &mut dyn PluginInstance, duration_ms: u64) {
    let step_ms = 16u64;
    let mut elapsed = 0u64;
    while elapsed < duration_ms {
        let chunk = step_ms.min(duration_ms - elapsed).max(1);
        unsafe {
            let until = NSDate::dateWithTimeIntervalSinceNow((chunk as f64) * 0.001);
            let run_loop: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSRunLoop), currentRunLoop];
            let _: () = objc2::msg_send![&run_loop, runUntilDate: &*until];
        }
        plugin.tick_main_thread(chunk);
        elapsed += chunk;
    }
}

unsafe fn capture_nsview_rgba(
    view: &NSView,
    width: u32,
    height: u32,
) -> HostResult<(u32, u32, Vec<u8>)> {
    let bounds = view.bounds();
    let rep: Option<Retained<NSBitmapImageRep>> =
        view.bitmapImageRepForCachingDisplayInRect(bounds);
    let rep = rep.ok_or_else(|| {
        HostError::GuiError("bitmapImageRepForCachingDisplayInRect returned nil".into())
    })?;

    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);

    let pixels_wide = rep.pixelsWide().max(width as isize) as u32;
    let pixels_high = rep.pixelsHigh().max(height as isize) as u32;
    let bytes_per_row = rep.bytesPerRow() as usize;
    let samples_per_pixel = rep.samplesPerPixel() as usize;
    let bitmap_ptr = rep.bitmapData();

    if bitmap_ptr.is_null() {
        return Err(HostError::GuiError(
            "NSBitmapImageRep.bitmapData is null".into(),
        ));
    }

    let mut rgba = vec![0u8; (pixels_wide * pixels_high * 4) as usize];
    for y in 0..(pixels_high as usize) {
        let row_src = std::slice::from_raw_parts(bitmap_ptr.add(y * bytes_per_row), bytes_per_row);
        for x in 0..(pixels_wide as usize) {
            let dst_idx = (y * (pixels_wide as usize) + x) * 4;
            let src_idx = x * samples_per_pixel;
            if src_idx + 3 < row_src.len() {
                rgba[dst_idx] = row_src[src_idx];
                rgba[dst_idx + 1] = row_src[src_idx + 1];
                rgba[dst_idx + 2] = row_src[src_idx + 2];
                rgba[dst_idx + 3] = if samples_per_pixel >= 4 {
                    row_src[src_idx + 3]
                } else {
                    255
                };
            }
        }
    }

    Ok((pixels_wide, pixels_high, rgba))
}

pub fn capture_plugin_screenshot(
    plugin: &mut dyn PluginInstance,
    output_path: &Path,
    warmup_ms: u64,
    custom_size: Option<(u32, u32)>,
) -> HostResult<GuiScreenshotResult> {
    if !plugin.supports_gui() {
        return Err(HostError::Unsupported(
            "Plugin does not support embedded Cocoa GUI".into(),
        ));
    }

    let mtm = MainThreadMarker::new().ok_or_else(|| {
        HostError::GuiError("GUI screenshot must be called on the macOS main thread".into())
    })?;
    let _app = ensure_headless_nsapp(mtm);

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

    let rect = NSRect::new(
        NSPoint::new(-10000.0, -10000.0),
        NSSize::new(width as f64, height as f64),
    );

    unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        let bg = NSColor::colorWithSRGBRed_green_blue_alpha(0.11, 0.12, 0.15, 1.0);
        window.setBackgroundColor(Some(&bg));

        let content_view = window
            .contentView()
            .ok_or_else(|| HostError::GuiError("NSWindow.contentView is nil".into()))?;
        content_view.setWantsLayer(true);

        let ns_view_ptr = Retained::as_ptr(&content_view) as *mut c_void;
        plugin.gui_set_parent(ns_view_ptr)?;

        let t_show = Instant::now();
        plugin.gui_show()?;
        window.orderBack(None);
        window.displayIfNeeded();
        let show_time_us = t_show.elapsed().as_secs_f64() * 1e6;

        pump_runloop_and_plugin(plugin, warmup_ms.max(32));
        content_view.setNeedsDisplay(true);
        window.display();

        let t_cap = Instant::now();
        let (cap_w, cap_h, rgba) = capture_nsview_rgba(&content_view, width, height)?;
        let capture_time_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let non_blank_pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] > 0 || px[1] > 0 || px[2] > 0)
            .count();

        save_rgba_png(output_path, cap_w, cap_h, &rgba)?;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        window.close();

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

    let mtm = MainThreadMarker::new().ok_or_else(|| {
        HostError::GuiError("GUI benchmark must run on the macOS main thread".into())
    })?;
    let _app = ensure_headless_nsapp(mtm);

    let t0 = Instant::now();
    plugin.gui_create()?;
    let create_latency_us = t0.elapsed().as_secs_f64() * 1e6;

    let (w, h) = plugin.gui_get_size().unwrap_or((640, 480));
    let can_resize = plugin.gui_can_resize();

    let rect = NSRect::new(
        NSPoint::new(-10000.0, -10000.0),
        NSSize::new(w.max(64) as f64, h.max(64) as f64),
    );

    unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        let content_view = window
            .contentView()
            .ok_or_else(|| HostError::GuiError("NSWindow.contentView is nil".into()))?;

        let t_parent = Instant::now();
        plugin.gui_set_parent(Retained::as_ptr(&content_view) as *mut c_void)?;
        let set_parent_latency_us = t_parent.elapsed().as_secs_f64() * 1e6;

        let t_show = Instant::now();
        plugin.gui_show()?;
        window.orderBack(None);
        window.display();
        let show_latency_us = t_show.elapsed().as_secs_f64() * 1e6;

        let n_frames = frames_to_pump.max(10);
        let mut total_frame_us = 0.0;
        let mut max_frame_us = 0.0f64;
        for _ in 0..n_frames {
            let tf = Instant::now();
            let until = NSDate::dateWithTimeIntervalSinceNow(0.001);
            let run_loop: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSRunLoop), currentRunLoop];
            let _: () = objc2::msg_send![&run_loop, runUntilDate: &*until];
            plugin.tick_main_thread(16);
            content_view.setNeedsDisplay(true);
            window.displayIfNeeded();
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
                window.setContentSize(NSSize::new(aw as f64, ah as f64));
                window.displayIfNeeded();
                sum_us += tr.elapsed().as_secs_f64() * 1e6;
            }
            Some(sum_us / (sizes.len() as f64))
        } else {
            None
        };

        let t_cap = Instant::now();
        let _ = capture_nsview_rgba(&content_view, w.max(64), h.max(64))?;
        let capture_rgba_latency_us = t_cap.elapsed().as_secs_f64() * 1e6;

        let _ = plugin.gui_hide();
        plugin.gui_destroy();
        window.close();
        let _ = Duration::from_micros(1);

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
