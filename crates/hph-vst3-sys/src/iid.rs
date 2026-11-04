use crate::base::{smtg_inline_uid, Steinberg_TUID};

// Platform UI types and categories
pub const kPlatformTypeNSView: &[u8] = b"NSView\0";
pub const kPlatformTypeHWND: &[u8] = b"HWND\0";
pub const kPlatformTypeX11EmbedWindowID: &[u8] = b"X11EmbedWindowID\0";
pub const kVstAudioEffectClass: &str = "Audio Module Class";
pub const kVstComponentControllerClass: &str = "Component Controller Class";
pub const kEditorViewType: &[u8] = b"editor\0";

// IIDs
pub const Steinberg_FUnknown_iid: Steinberg_TUID =
    smtg_inline_uid(0x00000000, 0x00000000, 0xC0000000, 0x00000046);
pub const Steinberg_IBStream_iid: Steinberg_TUID =
    smtg_inline_uid(0xC3BF6EA2, 0x30994752, 0x9B6BF990, 0x1EE33E9B);
pub const Steinberg_ISizeableStream_iid: Steinberg_TUID =
    smtg_inline_uid(0x04F9549E, 0xE02F4E6E, 0x87E86A87, 0x47F4E17F);
pub const Steinberg_IPlugView_iid: Steinberg_TUID =
    smtg_inline_uid(0x5BC32507, 0xD06049EA, 0xA6151B52, 0x2B755B29);
pub const Steinberg_IPlugFrame_iid: Steinberg_TUID =
    smtg_inline_uid(0x367FAF01, 0xAFA94693, 0x8D4DA2A0, 0xED0882A3);
pub const Steinberg_Linux_IEventHandler_iid: Steinberg_TUID =
    smtg_inline_uid(0x561E65C9, 0x13A0496F, 0x813A2C35, 0x654D7983);
pub const Steinberg_Linux_ITimerHandler_iid: Steinberg_TUID =
    smtg_inline_uid(0x10BDD94F, 0x41424774, 0x821FAD8F, 0xECA72CA9);
pub const Steinberg_Linux_IRunLoop_iid: Steinberg_TUID =
    smtg_inline_uid(0x18C35366, 0x97764F1A, 0x9C5B8385, 0x7A871389);
pub const Steinberg_IPluginBase_iid: Steinberg_TUID =
    smtg_inline_uid(0x22888DDB, 0x156E45AE, 0x8358B348, 0x08190625);
pub const Steinberg_IPluginFactory_iid: Steinberg_TUID =
    smtg_inline_uid(0x7A4D811C, 0x52114A1F, 0xAED9D2EE, 0x0B43BF9F);
pub const Steinberg_IPluginFactory2_iid: Steinberg_TUID =
    smtg_inline_uid(0x0007B650, 0xF24B4C0B, 0xA464EDB9, 0xF00B2ABB);
pub const Steinberg_IPluginFactory3_iid: Steinberg_TUID =
    smtg_inline_uid(0x4555A2AB, 0xC1234E57, 0x9B122910, 0x36878931);
pub const Steinberg_Vst_IComponent_iid: Steinberg_TUID =
    smtg_inline_uid(0xE831FF31, 0xF2D54301, 0x928EBBEE, 0x25697802);
pub const Steinberg_Vst_IAudioProcessor_iid: Steinberg_TUID =
    smtg_inline_uid(0x42043F99, 0xB7DA453C, 0xA569E79D, 0x9AAEC33D);
pub const Steinberg_Vst_IEditController_iid: Steinberg_TUID =
    smtg_inline_uid(0xDCD7BBE3, 0x7742448D, 0xA874AACC, 0x979C759E);
pub const Steinberg_Vst_IEditController2_iid: Steinberg_TUID =
    smtg_inline_uid(0x7F4EFE59, 0xF3204967, 0xAC27A3AE, 0xAFB63038);
pub const Steinberg_Vst_IComponentHandler_iid: Steinberg_TUID =
    smtg_inline_uid(0x93A0BEA3, 0x0BD045DB, 0x8E890B0C, 0xC1E46AC6);
pub const Steinberg_Vst_IComponentHandler2_iid: Steinberg_TUID =
    smtg_inline_uid(0xF040B4B3, 0xA36045EC, 0xABCDC045, 0xB4D5A2CC);
pub const Steinberg_Vst_IConnectionPoint_iid: Steinberg_TUID =
    smtg_inline_uid(0x70A4156F, 0x6E6E4026, 0x989148BF, 0xAA60D8D1);
pub const Steinberg_Vst_IHostApplication_iid: Steinberg_TUID =
    smtg_inline_uid(0x58E595CC, 0xDB2D4969, 0x8B6AAF8C, 0x36A664E5);
pub const Steinberg_Vst_IParamValueQueue_iid: Steinberg_TUID =
    smtg_inline_uid(0x01263A18, 0xED074F6F, 0x98C9D356, 0x4686F9BA);
pub const Steinberg_Vst_IParameterChanges_iid: Steinberg_TUID =
    smtg_inline_uid(0xA4779663, 0x0BB64A56, 0xB44384A8, 0x466FEB9D);
pub const Steinberg_Vst_IEventList_iid: Steinberg_TUID =
    smtg_inline_uid(0x3A2C4214, 0x346349FE, 0xB2C4F397, 0xB9695A44);
pub const Steinberg_Vst_IUnitInfo_iid: Steinberg_TUID =
    smtg_inline_uid(0x3D4BD6B5, 0x913A4FD2, 0xA886E768, 0xA5EB92C1);
pub const Steinberg_Vst_IMidiMapping_iid: Steinberg_TUID =
    smtg_inline_uid(0xDF0FF9F7, 0x49B74669, 0xB63AB732, 0x7ADBF5E5);
pub const Steinberg_Vst_INoteExpressionController_iid: Steinberg_TUID =
    smtg_inline_uid(0xB7F8F859, 0x41234872, 0x91169581, 0x4F3721A3);
pub const Steinberg_Vst_IKeyswitchController_iid: Steinberg_TUID =
    smtg_inline_uid(0x1F2F76D3, 0xBFFB4B96, 0xB99527A5, 0x5EBCCEF4);
pub const Steinberg_Vst_IAudioPresentationLatency_iid: Steinberg_TUID =
    smtg_inline_uid(0x309ECE78, 0xEB7D4FAE, 0x8B2225D9, 0x09FD08B6);
pub const Steinberg_Vst_IProcessContextRequirements_iid: Steinberg_TUID =
    smtg_inline_uid(0x2A654303, 0xEF764E3D, 0x95B5FE83, 0x730EF6D0);
pub const Steinberg_Vst_IAttributeList_iid: Steinberg_TUID =
    smtg_inline_uid(0x1E5F0AEB, 0xCC7F4533, 0xA2544011, 0x38AD5EE4);
pub const Steinberg_Vst_IMessage_iid: Steinberg_TUID =
    smtg_inline_uid(0x936F033B, 0xC6C047DB, 0xBB0882F8, 0x13C1E613);

use std::ffi::c_void;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::base::*;
use crate::gui::*;
use crate::interfaces::*;

/// Type-level binding between a VST3 COM interface struct, its vtable type, and its 16-byte IID.
///
/// # Safety
/// Implementors must be `#[repr(C)]` structs whose first field is `lpVtbl: *const Self::Vtbl`,
/// where `Self::Vtbl` begins with the 3 `Steinberg_FUnknownVtbl` methods (`queryInterface`, `addRef`, `release`).
pub unsafe trait ComInterface: Sized {
    type Vtbl;
    const IID: Steinberg_TUID;
}

macro_rules! impl_com_interface {
    ($($ty:ty, $vtbl:ty => $iid:ident),* $(,)?) => {
        $(
            unsafe impl ComInterface for $ty {
                type Vtbl = $vtbl;
                const IID: Steinberg_TUID = $iid;
            }
        )*
    };
}

impl_com_interface! {
    Steinberg_FUnknown, Steinberg_FUnknownVtbl => Steinberg_FUnknown_iid,
    Steinberg_IBStream, Steinberg_IBStreamVtbl => Steinberg_IBStream_iid,
    Steinberg_IPlugView, Steinberg_IPlugViewVtbl => Steinberg_IPlugView_iid,
    Steinberg_IPlugFrame, Steinberg_IPlugFrameVtbl => Steinberg_IPlugFrame_iid,
    Steinberg_Linux_IEventHandler, Steinberg_Linux_IEventHandlerVtbl => Steinberg_Linux_IEventHandler_iid,
    Steinberg_Linux_ITimerHandler, Steinberg_Linux_ITimerHandlerVtbl => Steinberg_Linux_ITimerHandler_iid,
    Steinberg_Linux_IRunLoop, Steinberg_Linux_IRunLoopVtbl => Steinberg_Linux_IRunLoop_iid,
    Steinberg_IPluginBase, Steinberg_IPluginBaseVtbl => Steinberg_IPluginBase_iid,
    Steinberg_IPluginFactory, Steinberg_IPluginFactoryVtbl => Steinberg_IPluginFactory_iid,
    Steinberg_IPluginFactory2, Steinberg_IPluginFactory2Vtbl => Steinberg_IPluginFactory2_iid,
    Steinberg_IPluginFactory3, Steinberg_IPluginFactory3Vtbl => Steinberg_IPluginFactory3_iid,
    Steinberg_Vst_IComponent, Steinberg_Vst_IComponentVtbl => Steinberg_Vst_IComponent_iid,
    Steinberg_Vst_IAudioProcessor, Steinberg_Vst_IAudioProcessorVtbl => Steinberg_Vst_IAudioProcessor_iid,
    Steinberg_Vst_IEditController, Steinberg_Vst_IEditControllerVtbl => Steinberg_Vst_IEditController_iid,
    Steinberg_Vst_IEditController2, Steinberg_Vst_IEditController2Vtbl => Steinberg_Vst_IEditController2_iid,
    Steinberg_Vst_IComponentHandler, Steinberg_Vst_IComponentHandlerVtbl => Steinberg_Vst_IComponentHandler_iid,
    Steinberg_Vst_IComponentHandler2, Steinberg_Vst_IComponentHandler2Vtbl => Steinberg_Vst_IComponentHandler2_iid,
    Steinberg_Vst_IConnectionPoint, Steinberg_Vst_IConnectionPointVtbl => Steinberg_Vst_IConnectionPoint_iid,
    Steinberg_Vst_IHostApplication, Steinberg_Vst_IHostApplicationVtbl => Steinberg_Vst_IHostApplication_iid,
    Steinberg_Vst_IParamValueQueue, Steinberg_Vst_IParamValueQueueVtbl => Steinberg_Vst_IParamValueQueue_iid,
    Steinberg_Vst_IParameterChanges, Steinberg_Vst_IParameterChangesVtbl => Steinberg_Vst_IParameterChanges_iid,
    Steinberg_Vst_IEventList, Steinberg_Vst_IEventListVtbl => Steinberg_Vst_IEventList_iid,
    Steinberg_Vst_IUnitInfo, Steinberg_Vst_IUnitInfoVtbl => Steinberg_Vst_IUnitInfo_iid,
    Steinberg_Vst_IMidiMapping, Steinberg_Vst_IMidiMappingVtbl => Steinberg_Vst_IMidiMapping_iid,
    Steinberg_Vst_INoteExpressionController, Steinberg_Vst_INoteExpressionControllerVtbl => Steinberg_Vst_INoteExpressionController_iid,
    Steinberg_Vst_IKeyswitchController, Steinberg_Vst_IKeyswitchControllerVtbl => Steinberg_Vst_IKeyswitchController_iid,
    Steinberg_Vst_IAudioPresentationLatency, Steinberg_Vst_IAudioPresentationLatencyVtbl => Steinberg_Vst_IAudioPresentationLatency_iid,
    Steinberg_Vst_IProcessContextRequirements, Steinberg_Vst_IProcessContextRequirementsVtbl => Steinberg_Vst_IProcessContextRequirements_iid,
    Steinberg_Vst_IAttributeList, Steinberg_Vst_IAttributeListVtbl => Steinberg_Vst_IAttributeList_iid,
    Steinberg_Vst_IMessage, Steinberg_Vst_IMessageVtbl => Steinberg_Vst_IMessage_iid,
}

/// RAII smart pointer for VST3 COM interfaces that automatically calls `release()` on [`Drop`].
pub struct ComPtr<T: ComInterface>(NonNull<T>);

impl<T: ComInterface> ComPtr<T> {
    /// Wrap a raw COM pointer that already owns a reference count (e.g. from `createInstance` or `createView`).
    /// Returns `None` if `ptr` is null or its `lpVtbl` pointer is null.
    ///
    /// # Safety
    /// `ptr` must either be null or point to a valid VST3 COM object of type `T` with an owned reference count.
    pub unsafe fn from_raw(ptr: *mut T) -> Option<Self> {
        let nn = NonNull::new(ptr)?;
        if (*(ptr as *const *const T::Vtbl)).is_null() {
            return None;
        }
        Some(Self(nn))
    }

    /// Query interface `T` (`T::IID`) from any VST3 COM pointer `src`.
    ///
    /// # Safety
    /// `src` must either be null or point to a valid VST3 `FUnknown`-compatible COM object.
    pub unsafe fn query<S>(src: *mut S) -> Option<Self> {
        Self::query_iid(src, &T::IID)
    }

    /// Query an arbitrary `iid` from `src` and wrap the result as `ComPtr<T>`.
    ///
    /// # Safety
    /// `src` must either be null or point to a valid VST3 `FUnknown`-compatible COM object.
    pub unsafe fn query_iid<S>(src: *mut S, iid: &Steinberg_TUID) -> Option<Self> {
        if src.is_null() {
            return None;
        }
        let unk = src as *mut Steinberg_FUnknown;
        let vtbl = (*unk).lpVtbl;
        if vtbl.is_null() {
            return None;
        }
        let mut out: *mut c_void = std::ptr::null_mut();
        if ((*vtbl).queryInterface)(src as *mut c_void, iid, &mut out) == kResultOk {
            Self::from_raw(out as *mut T)
        } else {
            None
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }

    pub fn as_void(&self) -> *mut c_void {
        self.0.as_ptr() as *mut c_void
    }

    /// Consume the `ComPtr` and return the raw pointer without calling `release()`.
    pub fn into_raw(self) -> *mut T {
        let ptr = self.0.as_ptr();
        std::mem::forget(self);
        ptr
    }
}

impl<T: ComInterface> Deref for ComPtr<T> {
    type Target = T::Vtbl;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let vtbl_ptr = *(self.0.as_ptr() as *const *const T::Vtbl);
            &*vtbl_ptr
        }
    }
}

impl<T: ComInterface> Drop for ComPtr<T> {
    fn drop(&mut self) {
        unsafe {
            let unk = self.0.as_ptr() as *mut Steinberg_FUnknown;
            if !(*unk).lpVtbl.is_null() {
                ((*(*unk).lpVtbl).release)(self.0.as_ptr() as *mut c_void);
            }
        }
    }
}
