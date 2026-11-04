use std::ffi::c_void;

use crate::base::{
    Steinberg_FIDString, Steinberg_FUnknown, Steinberg_IBStream, Steinberg_TBool, Steinberg_TUID,
    Steinberg_Vst_BusDirection, Steinberg_Vst_CString, Steinberg_Vst_CtrlNumber,
    Steinberg_Vst_IoMode, Steinberg_Vst_MediaType, Steinberg_Vst_ParamID, Steinberg_Vst_ParamValue,
    Steinberg_Vst_ProgramListID, Steinberg_Vst_SpeakerArrangement, Steinberg_Vst_TChar,
    Steinberg_Vst_UnitID, Steinberg_char8, Steinberg_int16, Steinberg_int32, Steinberg_int64,
    Steinberg_tresult, Steinberg_uint32,
};
use crate::gui::Steinberg_IPlugView;
use crate::vst_types::{
    Steinberg_Vst_BusInfo, Steinberg_Vst_Event, Steinberg_Vst_ParameterInfo,
    Steinberg_Vst_ProcessData, Steinberg_Vst_ProcessSetup, Steinberg_Vst_ProgramListInfo,
    Steinberg_Vst_RoutingInfo, Steinberg_Vst_UnitInfo,
};

#[repr(C)]
pub struct Steinberg_Vst_IComponentVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub initialize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        context: *mut Steinberg_FUnknown,
    ) -> Steinberg_tresult,
    pub terminate: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
    pub getControllerClassId: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        classId: *mut Steinberg_TUID,
    ) -> Steinberg_tresult,
    pub setIoMode: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        mode: Steinberg_Vst_IoMode,
    ) -> Steinberg_tresult,
    pub getBusCount: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        r#type: Steinberg_Vst_MediaType,
        dir: Steinberg_Vst_BusDirection,
    ) -> Steinberg_int32,
    pub getBusInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        r#type: Steinberg_Vst_MediaType,
        dir: Steinberg_Vst_BusDirection,
        index: Steinberg_int32,
        bus: *mut Steinberg_Vst_BusInfo,
    ) -> Steinberg_tresult,
    pub getRoutingInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        inInfo: *mut Steinberg_Vst_RoutingInfo,
        outInfo: *mut Steinberg_Vst_RoutingInfo,
    ) -> Steinberg_tresult,
    pub activateBus: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        r#type: Steinberg_Vst_MediaType,
        dir: Steinberg_Vst_BusDirection,
        index: Steinberg_int32,
        state: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub setActive: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub setState: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
    pub getState: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IComponent {
    pub lpVtbl: *const Steinberg_Vst_IComponentVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IAudioProcessorVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setBusArrangements: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        inputs: *mut Steinberg_Vst_SpeakerArrangement,
        numIns: Steinberg_int32,
        outputs: *mut Steinberg_Vst_SpeakerArrangement,
        numOuts: Steinberg_int32,
    ) -> Steinberg_tresult,
    pub getBusArrangement: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        dir: Steinberg_Vst_BusDirection,
        index: Steinberg_int32,
        arr: *mut Steinberg_Vst_SpeakerArrangement,
    ) -> Steinberg_tresult,
    pub canProcessSampleSize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        symbolicSampleSize: Steinberg_int32,
    ) -> Steinberg_tresult,
    pub getLatencySamples:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setupProcessing: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        setup: *mut Steinberg_Vst_ProcessSetup,
    ) -> Steinberg_tresult,
    pub setProcessing: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub process: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        data: *mut Steinberg_Vst_ProcessData,
    ) -> Steinberg_tresult,
    pub getTailSamples: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
}

#[repr(C)]
pub struct Steinberg_Vst_IAudioProcessor {
    pub lpVtbl: *const Steinberg_Vst_IAudioProcessorVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IComponentHandlerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub beginEdit: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
    ) -> Steinberg_tresult,
    pub performEdit: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        valueNormalized: Steinberg_Vst_ParamValue,
    ) -> Steinberg_tresult,
    pub endEdit: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
    ) -> Steinberg_tresult,
    pub restartComponent: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        flags: Steinberg_int32,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IComponentHandler {
    pub lpVtbl: *const Steinberg_Vst_IComponentHandlerVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IEditControllerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub initialize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        context: *mut Steinberg_FUnknown,
    ) -> Steinberg_tresult,
    pub terminate: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
    pub setComponentState: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
    pub setState: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
    pub getState: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
    pub getParameterCount: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getParameterInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        paramIndex: Steinberg_int32,
        info: *mut Steinberg_Vst_ParameterInfo,
    ) -> Steinberg_tresult,
    pub getParamStringByValue: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        valueNormalized: Steinberg_Vst_ParamValue,
        string: *mut Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub getParamValueByString: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        string: *const Steinberg_Vst_TChar,
        valueNormalized: *mut Steinberg_Vst_ParamValue,
    ) -> Steinberg_tresult,
    pub normalizedParamToPlain: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        valueNormalized: Steinberg_Vst_ParamValue,
    ) -> Steinberg_Vst_ParamValue,
    pub plainParamToNormalized: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        plainValue: Steinberg_Vst_ParamValue,
    ) -> Steinberg_Vst_ParamValue,
    pub getParamNormalized: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
    ) -> Steinberg_Vst_ParamValue,
    pub setParamNormalized: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_ParamID,
        value: Steinberg_Vst_ParamValue,
    ) -> Steinberg_tresult,
    pub setComponentHandler: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        handler: *mut Steinberg_Vst_IComponentHandler,
    ) -> Steinberg_tresult,
    pub createView: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        name: Steinberg_FIDString,
    ) -> *mut Steinberg_IPlugView,
}

#[repr(C)]
pub struct Steinberg_Vst_IEditController {
    pub lpVtbl: *const Steinberg_Vst_IEditControllerVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IConnectionPointVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub connect: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        other: *mut Steinberg_Vst_IConnectionPoint,
    ) -> Steinberg_tresult,
    pub disconnect: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        other: *mut Steinberg_Vst_IConnectionPoint,
    ) -> Steinberg_tresult,
    pub notify: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        message: *mut c_void,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IConnectionPoint {
    pub lpVtbl: *const Steinberg_Vst_IConnectionPointVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IHostApplicationVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getName: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        name: *mut Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub createInstance: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        cid: *const Steinberg_TUID,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IHostApplication {
    pub lpVtbl: *const Steinberg_Vst_IHostApplicationVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IParamValueQueueVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getParameterId:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_Vst_ParamID,
    pub getPointCount: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getPoint: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        sampleOffset: *mut Steinberg_int32,
        value: *mut Steinberg_Vst_ParamValue,
    ) -> Steinberg_tresult,
    pub addPoint: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        sampleOffset: Steinberg_int32,
        value: Steinberg_Vst_ParamValue,
        index: *mut Steinberg_int32,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IParamValueQueue {
    pub lpVtbl: *const Steinberg_Vst_IParamValueQueueVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IParameterChangesVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getParameterCount: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getParameterData: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
    ) -> *mut Steinberg_Vst_IParamValueQueue,
    pub addParameterData: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: *const Steinberg_Vst_ParamID,
        index: *mut Steinberg_int32,
    ) -> *mut Steinberg_Vst_IParamValueQueue,
}

#[repr(C)]
pub struct Steinberg_Vst_IParameterChanges {
    pub lpVtbl: *const Steinberg_Vst_IParameterChangesVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IEventListVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getEventCount: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getEvent: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        e: *mut Steinberg_Vst_Event,
    ) -> Steinberg_tresult,
    pub addEvent: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        e: *mut Steinberg_Vst_Event,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IEventList {
    pub lpVtbl: *const Steinberg_Vst_IEventListVtbl,
}

pub type Steinberg_Vst_AttrID = *const Steinberg_char8;

#[repr(C)]
pub struct Steinberg_Vst_IAttributeListVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setInt: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        value: Steinberg_int64,
    ) -> Steinberg_tresult,
    pub getInt: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        value: *mut Steinberg_int64,
    ) -> Steinberg_tresult,
    pub setFloat: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        value: f64,
    ) -> Steinberg_tresult,
    pub getFloat: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        value: *mut f64,
    ) -> Steinberg_tresult,
    pub setString: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        string: *const Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub getString: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        string: *mut Steinberg_Vst_TChar,
        sizeInBytes: Steinberg_uint32,
    ) -> Steinberg_tresult,
    pub setBinary: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        data: *const c_void,
        sizeInBytes: Steinberg_uint32,
    ) -> Steinberg_tresult,
    pub getBinary: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        id: Steinberg_Vst_AttrID,
        data: *mut *const c_void,
        sizeInBytes: *mut Steinberg_uint32,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IAttributeList {
    pub lpVtbl: *const Steinberg_Vst_IAttributeListVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IMessageVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getMessageID: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_FIDString,
    pub setMessageID:
        unsafe extern "system" fn(thisInterface: *mut c_void, id: Steinberg_FIDString),
    pub getAttributes:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> *mut Steinberg_Vst_IAttributeList,
}

#[repr(C)]
pub struct Steinberg_Vst_IMessage {
    pub lpVtbl: *const Steinberg_Vst_IMessageVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IComponentHandler2Vtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setDirty: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub requestOpenEditor: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        name: Steinberg_FIDString,
    ) -> Steinberg_tresult,
    pub startGroupEdit: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
    pub finishGroupEdit: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IComponentHandler2 {
    pub lpVtbl: *const Steinberg_Vst_IComponentHandler2Vtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IProcessContextRequirementsVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getProcessContextRequirements:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
}

#[repr(C)]
pub struct Steinberg_Vst_IProcessContextRequirements {
    pub lpVtbl: *const Steinberg_Vst_IProcessContextRequirementsVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IAudioPresentationLatencyVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setAudioPresentationLatencySamples: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        dir: Steinberg_Vst_BusDirection,
        busIndex: Steinberg_int32,
        latencyInSamples: Steinberg_uint32,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IAudioPresentationLatency {
    pub lpVtbl: *const Steinberg_Vst_IAudioPresentationLatencyVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IEditController2Vtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub setKnobMode: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        mode: Steinberg_int32,
    ) -> Steinberg_tresult,
    pub openHelp: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        onlyCheck: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub openAboutBox: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        onlyCheck: Steinberg_TBool,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IEditController2 {
    pub lpVtbl: *const Steinberg_Vst_IEditController2Vtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IUnitInfoVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getUnitCount: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getUnitInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        unitIndex: Steinberg_int32,
        info: *mut Steinberg_Vst_UnitInfo,
    ) -> Steinberg_tresult,
    pub getProgramListCount:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getProgramListInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listIndex: Steinberg_int32,
        info: *mut Steinberg_Vst_ProgramListInfo,
    ) -> Steinberg_tresult,
    pub getProgramName: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listId: Steinberg_Vst_ProgramListID,
        programIndex: Steinberg_int32,
        name: *mut Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub getProgramInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listId: Steinberg_Vst_ProgramListID,
        programIndex: Steinberg_int32,
        attributeId: Steinberg_Vst_CString,
        attributeValue: *mut Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub hasProgramPitchNames: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listId: Steinberg_Vst_ProgramListID,
        programIndex: Steinberg_int32,
    ) -> Steinberg_tresult,
    pub getProgramPitchName: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listId: Steinberg_Vst_ProgramListID,
        programIndex: Steinberg_int32,
        midiPitch: Steinberg_int16,
        name: *mut Steinberg_Vst_TChar,
    ) -> Steinberg_tresult,
    pub getSelectedUnit:
        unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_Vst_UnitID,
    pub selectUnit: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        unitId: Steinberg_Vst_UnitID,
    ) -> Steinberg_tresult,
    pub getUnitByBus: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        r#type: Steinberg_Vst_MediaType,
        dir: Steinberg_Vst_BusDirection,
        busIndex: Steinberg_int32,
        channel: Steinberg_int32,
        unitId: *mut Steinberg_Vst_UnitID,
    ) -> Steinberg_tresult,
    pub setUnitProgramData: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        listOrUnitId: Steinberg_int32,
        programIndex: Steinberg_int32,
        data: *mut Steinberg_IBStream,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IUnitInfo {
    pub lpVtbl: *const Steinberg_Vst_IUnitInfoVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IMidiMappingVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getMidiControllerAssignment: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        busIndex: Steinberg_int32,
        channel: Steinberg_int16,
        midiControllerNumber: Steinberg_Vst_CtrlNumber,
        id: *mut Steinberg_Vst_ParamID,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Vst_IMidiMapping {
    pub lpVtbl: *const Steinberg_Vst_IMidiMappingVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_INoteExpressionControllerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getNoteExpressionCount: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        busIndex: Steinberg_int32,
        channel: Steinberg_int16,
    ) -> Steinberg_int32,
}

#[repr(C)]
pub struct Steinberg_Vst_INoteExpressionController {
    pub lpVtbl: *const Steinberg_Vst_INoteExpressionControllerVtbl,
}

#[repr(C)]
pub struct Steinberg_Vst_IKeyswitchControllerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getKeyswitchCount: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        busIndex: Steinberg_int32,
        channel: Steinberg_int16,
    ) -> Steinberg_int32,
}

#[repr(C)]
pub struct Steinberg_Vst_IKeyswitchController {
    pub lpVtbl: *const Steinberg_Vst_IKeyswitchControllerVtbl,
}
