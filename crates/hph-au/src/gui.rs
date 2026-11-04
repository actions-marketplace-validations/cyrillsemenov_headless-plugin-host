use crate::bundle::ObjCAudioUnitRecord;
use crate::instance::AuPluginInstance;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use hph_au_sys::*;
use hph_core::*;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::ffi::c_void;

impl PluginGui for AuPluginInstance {
    fn supports_gui(&self) -> bool {
        let mut size: u32 = 0;
        let mut writable: Boolean = 0;
        unsafe {
            AudioUnitGetPropertyInfo(
                self.unit,
                kAudioUnitProperty_CocoaUI,
                kAudioUnitScope_Global,
                0,
                &mut size,
                &mut writable,
            ) == noErr
                && size >= (std::mem::size_of::<AudioUnitCocoaViewInfo>() as u32)
        }
    }

    fn gui_create(&mut self) -> HostResult<()> {
        if self.cocoa_view.is_some() {
            return Ok(());
        }
        if !self.is_initialized {
            let sr = if self.sample_rate > 0.0 {
                self.sample_rate
            } else {
                48000.0
            };
            let _ = self.activate(sr, 1, 512);
        }
        unsafe {
            let mut data_size: u32 = 0;
            let mut writable: Boolean = 0;
            let status = AudioUnitGetPropertyInfo(
                self.unit,
                kAudioUnitProperty_CocoaUI,
                kAudioUnitScope_Global,
                0,
                &mut data_size,
                &mut writable,
            );
            if status != noErr || data_size < (std::mem::size_of::<AudioUnitCocoaViewInfo>() as u32)
            {
                return Err(HostError::Unsupported(
                    "AudioUnit does not implement kAudioUnitProperty_CocoaUI".into(),
                ));
            }

            let mut buf = vec![0u8; data_size as usize];
            let status = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_CocoaUI,
                kAudioUnitScope_Global,
                0,
                buf.as_mut_ptr() as *mut c_void,
                &mut data_size,
            );
            if status != noErr {
                return Err(HostError::GuiError(format!(
                    "AudioUnitGetProperty(kAudioUnitProperty_CocoaUI) failed with OSStatus {status}"
                )));
            }

            let info = &*(buf.as_ptr() as *const AudioUnitCocoaViewInfo);
            let bundle_url_ref = info.mCocoaAUViewBundleLocation;
            let factory_class_ref = info.mCocoaAUViewClass[0];

            if bundle_url_ref.is_null() || factory_class_ref.is_null() {
                return Err(HostError::GuiError(
                    "AudioUnitCocoaViewInfo returned null bundle URL or view class".into(),
                ));
            }

            let cf_url = CFURL::wrap_under_create_rule(bundle_url_ref);
            let cf_class_name = CFString::wrap_under_create_rule(factory_class_ref);
            let class_name_str = cf_class_name.to_string();

            if let Some(bundle_path) = cf_url.to_path() {
                let ns_path = NSString::from_str(&bundle_path.to_string_lossy());
                let ns_bundle: Option<Retained<AnyObject>> =
                    objc2::msg_send_id![objc2::class!(NSBundle), bundleWithPath: &*ns_path];
                if let Some(ref b) = ns_bundle {
                    let _: bool = msg_send![b, load];
                }
            }

            let Some(cls) = AnyClass::get(&class_name_str) else {
                return Err(HostError::GuiError(format!(
                    "Objective-C AUCocoaUIBase class '{class_name_str}' not found"
                )));
            };

            let factory: Option<Retained<AnyObject>> = objc2::msg_send_id![cls, new];
            let factory = factory.ok_or_else(|| {
                HostError::GuiError(format!("Failed to instantiate '{class_name_str}'"))
            })?;

            let pref_size = NSSize::new(800.0, 600.0);
            let au_ptr = self.unit as *mut ObjCAudioUnitRecord;
            let raw_view: *mut NSView =
                msg_send![&factory, uiViewForAudioUnit: au_ptr, withSize: pref_size];
            if raw_view.is_null() {
                return Err(HostError::GuiError(format!(
                    "-[{class_name_str} uiViewForAudioUnit:withSize:] returned nil"
                )));
            }

            let view = Retained::retain(raw_view)
                .ok_or_else(|| HostError::GuiError("Failed to retain AU Cocoa NSView".into()))?;

            let frame: NSRect = view.frame();
            let w = (frame.size.width.round() as u32).max(320);
            let h = (frame.size.height.round() as u32).max(200);
            self.gui_size = Some((w, h));
            self.cocoa_view = Some(view);
            Ok(())
        }
    }

    fn gui_destroy(&mut self) {
        if let Some(view) = self.cocoa_view.take() {
            unsafe {
                view.removeFromSuperview();
            }
        }
        self.gui_size = None;
    }

    fn gui_get_size(&self) -> Option<(u32, u32)> {
        if let Some(ref view) = self.cocoa_view {
            let frame = view.frame();
            let w = (frame.size.width.round() as u32).max(64);
            let h = (frame.size.height.round() as u32).max(64);
            Some((w, h))
        } else {
            self.gui_size
        }
    }

    fn gui_adjust_size(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        Some((width, height))
    }

    fn gui_set_size(&mut self, width: u32, height: u32) -> HostResult<()> {
        if let Some(ref view) = self.cocoa_view {
            unsafe {
                view.setFrameSize(NSSize::new(width as f64, height as f64));
            }
            self.gui_size = Some((width, height));
        }
        Ok(())
    }

    unsafe fn gui_set_parent(&mut self, ns_view: *mut c_void) -> HostResult<()> {
        if ns_view.is_null() {
            return Err(HostError::GuiError("Parent NSView pointer is null".into()));
        }
        let Some(ref child_view) = self.cocoa_view else {
            return Err(HostError::GuiError(
                "gui_create must be called before gui_set_parent".into(),
            ));
        };
        let parent_view = &*(ns_view as *const NSView);
        let (w, h) = self.gui_get_size().unwrap_or((640, 480));
        child_view.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(w as f64, h as f64),
        ));
        parent_view.addSubview(child_view);
        Ok(())
    }

    fn gui_show(&mut self) -> HostResult<()> {
        if let Some(ref view) = self.cocoa_view {
            view.setHidden(false);
            unsafe {
                view.setNeedsDisplay(true);
            }
        }
        Ok(())
    }

    fn gui_hide(&mut self) -> HostResult<()> {
        if let Some(ref view) = self.cocoa_view {
            view.setHidden(true);
        }
        Ok(())
    }
}
