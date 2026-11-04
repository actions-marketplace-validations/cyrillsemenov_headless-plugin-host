use objc2::rc::Retained;
use objc2_app_kit::{NSBox, NSBoxType, NSColor, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
use std::ffi::c_void;

pub unsafe fn create_mock_cocoa_view(width: u32, height: u32) -> Option<Retained<NSView>> {
    let mtm = MainThreadMarker::new()?;
    let rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width as f64, height as f64),
    );
    let view = NSView::initWithFrame(mtm.alloc(), rect);

    // Background panel
    let bg_box = NSBox::initWithFrame(mtm.alloc(), rect);
    bg_box.setBoxType(NSBoxType::NSBoxCustom);
    bg_box.setBorderWidth(2.0);
    let bg_fill = NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.16, 0.24, 1.0);
    let border_col = NSColor::colorWithSRGBRed_green_blue_alpha(0.28, 0.58, 0.92, 1.0);
    bg_box.setFillColor(&bg_fill);
    bg_box.setBorderColor(&border_col);
    view.addSubview(&bg_box);

    // Draw 3 decorative meter/knob indicator bars inside the mock GUI
    let colors = [(0.20, 0.78, 0.55), (0.95, 0.62, 0.20), (0.90, 0.28, 0.38)];
    for (i, &(r, g, b)) in colors.iter().enumerate() {
        let bar_rect = NSRect::new(
            NSPoint::new(40.0 + (i as f64) * 140.0, 40.0),
            NSSize::new(100.0, 180.0 - (i as f64) * 30.0),
        );
        let bar = NSBox::initWithFrame(mtm.alloc(), bar_rect);
        bar.setBoxType(NSBoxType::NSBoxCustom);
        bar.setBorderWidth(1.0);
        let fill = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
        bar.setFillColor(&fill);
        bar.setBorderColor(&border_col);
        view.addSubview(&bar);
    }

    Some(view)
}

pub unsafe fn destroy_mock_cocoa_view(child_view: &mut Option<Retained<NSView>>) {
    if let Some(ref v) = child_view {
        v.removeFromSuperview();
    }
    *child_view = None;
}

pub unsafe fn resize_mock_cocoa_view(
    child_view: &Option<Retained<NSView>>,
    width: u32,
    height: u32,
) {
    if let Some(ref v) = child_view {
        v.setFrameSize(NSSize::new(width as f64, height as f64));
    }
}

pub unsafe fn attach_mock_cocoa_view(
    child_view: &Option<Retained<NSView>>,
    parent_cocoa: *mut c_void,
) -> bool {
    let parent_ptr = parent_cocoa as *mut NSView;
    if parent_ptr.is_null() {
        return false;
    }
    let parent_view = &*parent_ptr;
    if let Some(ref child) = child_view {
        parent_view.addSubview(child);
        return true;
    }
    false
}
