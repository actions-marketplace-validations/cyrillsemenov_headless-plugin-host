use hph_clap_sys::*;
use hph_core::*;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

#[repr(C)]
pub(crate) union RawClapEventUnion {
    pub(crate) header: clap_event_header_t,
    pub(crate) note: clap_event_note_t,
    pub(crate) param_val: clap_event_param_value_t,
    pub(crate) param_mod: clap_event_param_mod_t,
    pub(crate) midi: clap_event_midi_t,
}

pub(crate) struct ClapInputEventQueue {
    pub(crate) events: Vec<RawClapEventUnion>,
}

impl ClapInputEventQueue {
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self {
            events: Vec::with_capacity(cap),
        }
    }

    pub(crate) fn populate(
        &mut self,
        events: &[ProcessEvent],
        cookies: &HashMap<u32, *mut c_void>,
    ) {
        self.events.clear();
        for ev in events {
            match *ev {
                ProcessEvent::NoteOn {
                    sample_offset,
                    port_index,
                    channel,
                    key,
                    note_id,
                    velocity,
                } => {
                    self.events.push(RawClapEventUnion {
                        note: clap_event_note_t {
                            header: clap_event_header_t {
                                size: std::mem::size_of::<clap_event_note_t>() as u32,
                                time: sample_offset,
                                space_id: CLAP_CORE_EVENT_SPACE_ID,
                                type_: CLAP_EVENT_NOTE_ON,
                                flags: 0,
                            },
                            note_id,
                            port_index: port_index as i16,
                            channel,
                            key,
                            velocity,
                        },
                    });
                }
                ProcessEvent::NoteOff {
                    sample_offset,
                    port_index,
                    channel,
                    key,
                    note_id,
                    velocity,
                } => {
                    self.events.push(RawClapEventUnion {
                        note: clap_event_note_t {
                            header: clap_event_header_t {
                                size: std::mem::size_of::<clap_event_note_t>() as u32,
                                time: sample_offset,
                                space_id: CLAP_CORE_EVENT_SPACE_ID,
                                type_: CLAP_EVENT_NOTE_OFF,
                                flags: 0,
                            },
                            note_id,
                            port_index: port_index as i16,
                            channel,
                            key,
                            velocity,
                        },
                    });
                }
                ProcessEvent::ParamValue {
                    sample_offset,
                    param_id,
                    note_id,
                    port_index,
                    channel,
                    key,
                    value,
                } => {
                    let cookie = cookies.get(&param_id).copied().unwrap_or(ptr::null_mut());
                    self.events.push(RawClapEventUnion {
                        param_val: clap_event_param_value_t {
                            header: clap_event_header_t {
                                size: std::mem::size_of::<clap_event_param_value_t>() as u32,
                                time: sample_offset,
                                space_id: CLAP_CORE_EVENT_SPACE_ID,
                                type_: CLAP_EVENT_PARAM_VALUE,
                                flags: 0,
                            },
                            param_id,
                            cookie,
                            note_id,
                            port_index,
                            channel,
                            key,
                            value,
                        },
                    });
                }
                ProcessEvent::ParamMod {
                    sample_offset,
                    param_id,
                    note_id,
                    port_index,
                    channel,
                    key,
                    amount,
                } => {
                    let cookie = cookies.get(&param_id).copied().unwrap_or(ptr::null_mut());
                    self.events.push(RawClapEventUnion {
                        param_mod: clap_event_param_mod_t {
                            header: clap_event_header_t {
                                size: std::mem::size_of::<clap_event_param_mod_t>() as u32,
                                time: sample_offset,
                                space_id: CLAP_CORE_EVENT_SPACE_ID,
                                type_: CLAP_EVENT_PARAM_MOD,
                                flags: 0,
                            },
                            param_id,
                            cookie,
                            note_id,
                            port_index,
                            channel,
                            key,
                            amount,
                        },
                    });
                }
                ProcessEvent::Midi1 {
                    sample_offset,
                    port_index,
                    data,
                } => {
                    self.events.push(RawClapEventUnion {
                        midi: clap_event_midi_t {
                            header: clap_event_header_t {
                                size: std::mem::size_of::<clap_event_midi_t>() as u32,
                                time: sample_offset,
                                space_id: CLAP_CORE_EVENT_SPACE_ID,
                                type_: CLAP_EVENT_MIDI,
                                flags: 0,
                            },
                            port_index,
                            data,
                        },
                    });
                }
            }
        }
        self.events.sort_by_key(|u| unsafe { u.header.time });
    }

    pub(crate) fn from_events(
        events: &[ProcessEvent],
        cookies: &HashMap<u32, *mut c_void>,
    ) -> Self {
        let mut q = Self::with_capacity(events.len());
        q.populate(events, cookies);
        q
    }
}

pub(crate) unsafe extern "C" fn input_events_size(list: *const clap_input_events_t) -> u32 {
    let q = &*((*list).ctx as *const ClapInputEventQueue);
    q.events.len() as u32
}

pub(crate) unsafe extern "C" fn input_events_get(
    list: *const clap_input_events_t,
    index: u32,
) -> *const clap_event_header_t {
    let q = &*((*list).ctx as *const ClapInputEventQueue);
    if (index as usize) < q.events.len() {
        &q.events[index as usize].header as *const clap_event_header_t
    } else {
        ptr::null()
    }
}

pub(crate) unsafe extern "C" fn output_events_try_push(
    list: *const clap_output_events_t,
    event: *const clap_event_header_t,
) -> bool {
    if list.is_null() || event.is_null() {
        return false;
    }
    let out_vec = &mut *((*list).ctx as *mut Vec<ProcessEvent>);
    let hdr = &*event;
    if hdr.space_id != CLAP_CORE_EVENT_SPACE_ID {
        return true;
    }
    match hdr.type_ {
        CLAP_EVENT_NOTE_ON => {
            let n = &*(event as *const clap_event_note_t);
            out_vec.push(ProcessEvent::NoteOn {
                sample_offset: hdr.time,
                port_index: n.port_index.max(0) as u16,
                channel: n.channel,
                key: n.key,
                note_id: n.note_id,
                velocity: n.velocity,
            });
        }
        CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_END => {
            let n = &*(event as *const clap_event_note_t);
            out_vec.push(ProcessEvent::NoteOff {
                sample_offset: hdr.time,
                port_index: n.port_index.max(0) as u16,
                channel: n.channel,
                key: n.key,
                note_id: n.note_id,
                velocity: n.velocity,
            });
        }
        CLAP_EVENT_PARAM_VALUE => {
            let p = &*(event as *const clap_event_param_value_t);
            out_vec.push(ProcessEvent::ParamValue {
                sample_offset: hdr.time,
                param_id: p.param_id,
                note_id: p.note_id,
                port_index: p.port_index,
                channel: p.channel,
                key: p.key,
                value: p.value,
            });
        }
        CLAP_EVENT_PARAM_MOD => {
            let p = &*(event as *const clap_event_param_mod_t);
            out_vec.push(ProcessEvent::ParamMod {
                sample_offset: hdr.time,
                param_id: p.param_id,
                note_id: p.note_id,
                port_index: p.port_index,
                channel: p.channel,
                key: p.key,
                amount: p.amount,
            });
        }
        CLAP_EVENT_MIDI => {
            let m = &*(event as *const clap_event_midi_t);
            out_vec.push(ProcessEvent::Midi1 {
                sample_offset: hdr.time,
                port_index: m.port_index,
                data: m.data,
            });
        }
        _ => {}
    }
    true
}

pub(crate) unsafe extern "C" fn ostream_write(
    stream: *const clap_ostream_t,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    let vec = &mut *((*stream).ctx as *mut Vec<u8>);
    let slice = std::slice::from_raw_parts(buffer as *const u8, size as usize);
    vec.extend_from_slice(slice);
    size as i64
}

pub(crate) struct SliceReader<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) pos: usize,
}

pub(crate) unsafe extern "C" fn istream_read(
    stream: *const clap_istream_t,
    buffer: *mut c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    let reader = &mut *((*stream).ctx as *mut SliceReader<'_>);
    let remaining = reader.data.len().saturating_sub(reader.pos);
    let to_read = (size as usize).min(remaining);
    if to_read == 0 {
        return 0;
    }
    let dst = std::slice::from_raw_parts_mut(buffer as *mut u8, to_read);
    dst.copy_from_slice(&reader.data[reader.pos..reader.pos + to_read]);
    reader.pos += to_read;
    to_read as i64
}
