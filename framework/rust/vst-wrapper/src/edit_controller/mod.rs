use std::{
    cell::RefCell,
    collections::{hash_map, HashMap},
    rc,
};

use conformal_component::{
    parameters::{
        self,
        serialization::{DeserializationError, ReadInfoRef},
        InfoRef, TypeSpecificInfo, TypeSpecificInfoRef,
    },
    synth::{
        AFTERTOUCH_PARAMETER, CONTROLLER_PARAMETERS, EXPRESSION_PARAMETER, MOD_WHEEL_PARAMETER,
        PITCH_BEND_PARAMETER, SUSTAIN_PARAMETER,
    },
};

#[cfg(target_os = "macos")]
use macos_bundle::get_current_bundle_info;

use ui::Size;
use vst3::{
    Class, ComPtr, ComRef,
    Steinberg::{
        IPluginBase, IPluginBaseTrait,
        Vst::{
            IComponentHandler, IComponentHandlerTrait, IEditController, IEditControllerTrait,
            IHostApplication, IMidiMapping, IMidiMappingTrait,
        },
    },
};

use crate::{ExtraParameters, ParameterModel};

use super::{
    from_utf16_ptr, host_info,
    io::StreamRead,
    parameters::{
        convert_enum, convert_numeric, convert_switch, normalize_enum, normalize_numeric,
        normalize_switch,
    },
    processor::state,
    to_utf16, view,
};

#[cfg(test)]
mod tests;

fn as_deserialization(info: &parameters::Info) -> ReadInfoRef<impl Iterator<Item = &str> + Clone> {
    match &info.type_specific {
        TypeSpecificInfo::Enum { default, values } => ReadInfoRef::Enum {
            default: *default,
            values: values.iter().map(String::as_str),
        },
        TypeSpecificInfo::Numeric {
            default,
            valid_range,
            ..
        } => ReadInfoRef::Numeric {
            default: *default,
            valid_range: valid_range.clone(),
        },
        TypeSpecificInfo::Switch { default } => ReadInfoRef::Switch { default: *default },
    }
}

struct ParameterStore {
    unhash: HashMap<parameters::IdHash, String>,
    infos: HashMap<String, parameters::Info>,
    values: HashMap<String, parameters::InternalValue>,
    order: Vec<String>,

    component_handler: Option<ComPtr<IComponentHandler>>,

    // Note that unsized weak types can't dangle, so we use Option here to allow dangling.
    listener: Option<rc::Weak<dyn parameters::store::Listener>>,
}

#[derive(Clone)]
struct SharedStore {
    store: rc::Rc<RefCell<ParameterStore>>,
}

struct Initialized {
    store: SharedStore,
    parameter_model: ParameterModel,
    pref_domain: String,
}

fn lookup_by_hash<'a, T>(
    hash: parameters::IdHash,
    hash_to_id: &HashMap<parameters::IdHash, String>,
    values: &'a HashMap<String, T>,
) -> Option<&'a T> {
    hash_to_id.get(&hash).and_then(|id| values.get(id))
}

enum State {
    ReadyForInitialization(ParameterModel, String),
    Initialized(Initialized),
}

struct EditController {
    s: RefCell<Option<State>>,
    host: RefCell<Option<ComPtr<IHostApplication>>>,
    ui_initial_size: Size,
    bypass_id: Option<&'static str>,
}

// Brought out to a separate function for ease of testing
fn create_internal(
    parameter_model: ParameterModel,
    pref_domain: String,
    ui_initial_size: Size,
    bypass_id: Option<&'static str>,
) -> EditController {
    EditController {
        s: Some(State::ReadyForInitialization(parameter_model, pref_domain)).into(),
        host: Default::default(),
        ui_initial_size,
        bypass_id,
    }
}

pub fn create(
    parameter_model: ParameterModel,
    ui_initial_size: Size,
    bypass_id: Option<&'static str>,
) -> impl Class<Interfaces = (IPluginBase, IEditController, IMidiMapping)>
       + IEditControllerTrait
       + IMidiMappingTrait
       + 'static {
    create_internal(
        parameter_model,
        get_current_bundle_info()
            .expect("Could not find bundle info")
            .identifier,
        ui_initial_size,
        bypass_id,
    )
}

fn get_default(info: &TypeSpecificInfo) -> parameters::InternalValue {
    match info {
        TypeSpecificInfo::Enum { default, .. } => parameters::InternalValue::Enum(*default),
        TypeSpecificInfo::Numeric { default, .. } => parameters::InternalValue::Numeric(*default),
        TypeSpecificInfo::Switch { default } => parameters::InternalValue::Switch(*default),
    }
}

fn get_default_ref<S: AsRef<str>>(info: &TypeSpecificInfoRef<'_, S>) -> parameters::InternalValue {
    match info {
        TypeSpecificInfoRef::Enum { default, .. } => parameters::InternalValue::Enum(*default),
        TypeSpecificInfoRef::Numeric { default, .. } => {
            parameters::InternalValue::Numeric(*default)
        }
        TypeSpecificInfoRef::Switch { default } => parameters::InternalValue::Switch(*default),
    }
}

fn to_internal(
    unique_id: &str,
    value: &parameters::Value,
    infos: &HashMap<String, parameters::Info>,
) -> parameters::InternalValue {
    match (value, infos.get(unique_id).map(|info| &info.type_specific)) {
        (parameters::Value::Enum(v), Some(TypeSpecificInfo::Enum { values, .. })) => {
            parameters::InternalValue::Enum(
                values
                    .iter()
                    .position(|x| x == v)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
        }
        (parameters::Value::Numeric(v), Some(TypeSpecificInfo::Numeric { .. })) => {
            parameters::InternalValue::Numeric(*v)
        }
        (parameters::Value::Switch(v), Some(TypeSpecificInfo::Switch { .. })) => {
            parameters::InternalValue::Switch(*v)
        }
        _ => panic!("Invalid parameter!"),
    }
}

fn from_internal(
    unique_id: &str,
    value: parameters::InternalValue,
    infos: &HashMap<String, parameters::Info>,
) -> parameters::Value {
    match (value, infos.get(unique_id).map(|info| &info.type_specific)) {
        (parameters::InternalValue::Enum(v), Some(TypeSpecificInfo::Enum { values, .. })) => {
            parameters::Value::Enum(values[v as usize].to_string())
        }
        (parameters::InternalValue::Numeric(v), Some(TypeSpecificInfo::Numeric { .. })) => {
            parameters::Value::Numeric(v)
        }
        (parameters::InternalValue::Switch(v), Some(TypeSpecificInfo::Switch { .. })) => {
            parameters::Value::Switch(v)
        }
        _ => panic!("Invalid parameter!"),
    }
}

impl parameters::store::Store for SharedStore {
    fn get(&self, id: &str) -> Option<parameters::Value> {
        self.store
            .borrow()
            .values
            .get(id)
            .map(|v| from_internal(id, *v, &self.store.borrow().infos))
    }

    fn set_listener(&mut self, listener: rc::Weak<dyn parameters::store::Listener>) {
        self.store.borrow_mut().listener = Some(listener);
    }

    fn set(
        &mut self,
        unique_id: &str,
        value: parameters::Value,
    ) -> Result<(), parameters::store::SetError> {
        let maybe_set = if let ParameterStore {
            component_handler: Some(component_handler),
            infos,
            values,
            ..
        } = &mut (*self.store.borrow_mut())
        {
            (match (&value, infos.get(unique_id)) {
                (
                    parameters::Value::Numeric(value),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Numeric { valid_range, .. },
                        ..
                    }),
                ) => {
                    if valid_range.contains(value) {
                        Ok(normalize_numeric(*value, valid_range))
                    } else {
                        Err(parameters::store::SetError::InvalidValue)
                    }
                }
                (
                    parameters::Value::Enum(value),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Enum { values, .. },
                        ..
                    }),
                ) => values
                    .iter()
                    .position(|x| x == value)
                    .map(|index| {
                        normalize_enum(index.try_into().unwrap(), values.len().try_into().unwrap())
                    })
                    .ok_or(parameters::store::SetError::InvalidValue),
                (
                    parameters::Value::Switch(value),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { .. },
                        ..
                    }),
                ) => Ok(normalize_switch(*value)),
                (_, Some(_)) => Err(parameters::store::SetError::WrongType),
                (_, None) => Err(parameters::store::SetError::NotFound),
            })
            .map(|v| {
                values.insert(unique_id.to_string(), to_internal(unique_id, &value, infos));
                (component_handler.clone(), parameters::hash_id(unique_id), v)
            })
        } else {
            Err(parameters::store::SetError::InternalError)
        };
        maybe_set.map(|(component_handler, hash, v)| {
            unsafe {
                component_handler.performEdit(hash, v);
            };
        })
    }

    fn set_grabbed(
        &mut self,
        unique_id: &str,
        grabbed: bool,
    ) -> Result<(), parameters::store::SetGrabbedError> {
        let maybe_set = if let ParameterStore {
            component_handler: Some(component_handler),
            infos,
            ..
        } = &(*self.store.borrow())
        {
            if infos.contains_key(unique_id) {
                Ok((component_handler.clone(), parameters::hash_id(unique_id)))
            } else {
                Err(parameters::store::SetGrabbedError::NotFound)
            }
        } else {
            Err(parameters::store::SetGrabbedError::InternalError)
        };
        maybe_set.map(|(component_handler, hashed)| {
            if grabbed {
                unsafe {
                    component_handler.beginEdit(hashed);
                }
            } else {
                unsafe {
                    component_handler.endEdit(hashed);
                }
            }
        })
    }

    fn get_info(&self, unique_id: &str) -> Option<parameters::Info> {
        self.store.borrow().infos.get(unique_id).cloned()
    }
}

/// For testing only.
#[cfg(test)]
trait GetStore {
    type Store: parameters::store::Store;
    fn get_store(&self) -> Option<Self::Store>;
}

#[cfg(test)]
impl GetStore for EditController {
    type Store = SharedStore;
    fn get_store(&self) -> Option<Self::Store> {
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            Some(store.clone())
        } else {
            None
        }
    }
}

impl IPluginBaseTrait for EditController {
    unsafe fn initialize(
        &self,
        context: *mut vst3::Steinberg::FUnknown,
    ) -> vst3::Steinberg::tresult {
        if self.host.borrow().is_some() {
            return vst3::Steinberg::kInvalidArgument;
        }

        match ComRef::from_raw(context).and_then(|context| context.cast()) {
            Some(host) => self.host.replace(Some(host)),
            None => return vst3::Steinberg::kNoInterface,
        };

        let (s, res) = match (
            self.s.replace(None).unwrap(),
            host_info::get(&self.host.borrow().clone().unwrap()),
        ) {
            (State::ReadyForInitialization(parameter_model, pref_domain), Some(host_info)) => {
                let parameter_infos = {
                    let mut infos = (parameter_model.parameter_infos)(&host_info);
                    if parameter_model.extra_parameters == ExtraParameters::SynthControlParameters {
                        infos.extend(CONTROLLER_PARAMETERS.iter().map(parameters::Info::from));
                    }
                    infos
                };
                let parameters: HashMap<String, parameters::Info> = parameter_infos
                    .iter()
                    .map(|info| {
                        (
                            info.unique_id.clone(),
                            (&Into::<InfoRef<'_, _>>::into(info)).into(),
                        )
                    })
                    .collect();

                // If the client provided a bypass ID, this must exist and be a switch parameter
                // with default off.
                if let Some(bypass_id) = self.bypass_id {
                    assert!(parameters.contains_key(bypass_id));
                    if let Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { default },
                        ..
                    }) = parameters.get(bypass_id)
                    {
                        assert!(!*default);
                    } else {
                        panic!("Bypass ID must be a switch parameter with default off.");
                    }
                }

                // All parameters must have unique ids.
                assert_eq!(parameter_infos.len(), parameters.len());
                assert!(parameter_infos.len() < i32::MAX as usize);
                let s = State::Initialized(Initialized {
                    store: SharedStore {store: rc::Rc::new(RefCell::new(ParameterStore {
                        unhash: hash_parameter_ids(parameter_infos.iter().map(Into::into)).expect("Duplicate parameter ID hash! This could be caused by duplicate parameter IDs or a hash collision."),
                        infos: parameters,
                        values: parameter_infos.iter()
                            .map(|info| {
                            (
                                info.unique_id.clone(),
                                get_default_ref(&((&info.type_specific).into())),
                            )
                        })
                        .collect(),
                        order: parameter_infos
                        .iter()
                        .map(|info| info.unique_id.clone())
                        .collect(),
                        component_handler: Default::default(),
                        listener: Default::default(),
                    }))},
                    parameter_model,
                    pref_domain,
                });
                (s, vst3::Steinberg::kResultOk)
            }
            (s, _) => (s, vst3::Steinberg::kInvalidArgument),
        };
        self.s.replace(Some(s));
        res
    }

    unsafe fn terminate(&self) -> vst3::Steinberg::tresult {
        self.host.replace(None);
        match self.s.take() {
            Some(State::Initialized(Initialized {
                parameter_model,
                pref_domain,
                ..
            })) => {
                self.s.replace(Some(State::ReadyForInitialization(
                    parameter_model,
                    pref_domain,
                )));
                vst3::Steinberg::kResultOk
            }
            _ => vst3::Steinberg::kInvalidArgument,
        }
    }
}

fn hash_parameter_ids<'a, S: AsRef<str> + 'a, I: IntoIterator<Item = InfoRef<'a, S>>>(
    parameter_info: I,
) -> Option<HashMap<u32, String>> {
    let mut hash_to_id = HashMap::new();
    for info in parameter_info {
        let hash = parameters::hash_id(info.unique_id);
        match hash_to_id.entry(hash) {
            hash_map::Entry::Vacant(entry) => {
                entry.insert(info.unique_id.to_owned());
            }
            hash_map::Entry::Occupied(_) => return None,
        }
    }
    Some(hash_to_id)
}

/// Note this assumes that `new_values` contains every key in `parameter_values`.
fn apply_values<'a>(
    new_values: impl IntoIterator<Item = (&'a str, parameters::InternalValue)>,
    parameter_values: &mut HashMap<String, parameters::InternalValue>,
) {
    for (id, value) in new_values {
        parameter_values.insert(id.to_string(), value);
    }
}

impl IEditControllerTrait for EditController {
    unsafe fn setComponentState(
        &self,
        stream: *mut vst3::Steinberg::IBStream,
    ) -> vst3::Steinberg::tresult {
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow_mut().as_mut().unwrap()
        {
            let ParameterStore {
                ref infos,
                ref mut values,
                ref listener,
                ..
            } = &mut *store.store.borrow_mut();
            if let Some(com_stream) = ComRef::from_raw(stream) {
                let read = StreamRead::new(com_stream);
                if let Ok(state) = rmp_serde::from_read::<_, state::State>(read) {
                    return match state.params.into_snapshot(
                        infos
                            .iter()
                            .map(|(id, info)| (id.as_str(), as_deserialization(info))),
                    ) {
                        Ok(snapshot) => {
                            apply_values(
                                snapshot
                                    .values
                                    .iter()
                                    .map(|(k, v)| (k.as_str(), to_internal(k.as_str(), v, infos))),
                                values,
                            );

                            for (id, value) in &snapshot.values {
                                if let Some(listener) = listener {
                                    if let Some(listener) = listener.upgrade() {
                                        listener.parameter_changed(id.as_str(), value);
                                    }
                                }
                            }
                            vst3::Steinberg::kResultOk
                        }
                        Err(DeserializationError::Corrupted(_)) => {
                            vst3::Steinberg::kInvalidArgument
                        }
                        Err(DeserializationError::VersionTooNew()) => {
                            apply_values(
                                infos.iter().map(|(id, info)| {
                                    (id.as_str(), get_default(&info.type_specific))
                                }),
                                values,
                            );

                            vst3::Steinberg::kResultOk
                        }
                    };
                }
            }
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn setState(&self, _state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        // Note that we don't support any state on the edit controller side at the moment
        vst3::Steinberg::kResultOk
    }

    unsafe fn getState(&self, _state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        // Note that we don't support any state on the edit controller side at the moment
        vst3::Steinberg::kResultOk
    }

    unsafe fn getParameterCount(&self) -> vst3::Steinberg::int32 {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self.s.borrow().as_ref().unwrap() {
            State::Initialized(s) => i32::try_from(s.store.store.borrow().infos.len()).unwrap(),
            // Note - it is a host error to call this function if we are not intialized.
            // However, this function has no way to return an error message, so we just return 0.
            _ => 0,
        }
    }

    unsafe fn getParameterInfo(
        &self,
        param_index: vst3::Steinberg::int32,
        info_out: *mut vst3::Steinberg::Vst::ParameterInfo,
    ) -> vst3::Steinberg::tresult {
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            let ParameterStore { infos, order, .. } = &*store.store.borrow();
            if param_index < 0 || param_index as usize >= order.len() {
                return vst3::Steinberg::kInvalidArgument;
            }
            let param_id = order[param_index as usize].clone();
            let param_hash = parameters::hash_id(&param_id);
            let info = infos.get(&param_id).unwrap();

            let info_out = &mut *info_out;
            info_out.id = param_hash;
            to_utf16(&info.title, &mut info_out.title);
            to_utf16(&info.short_title, &mut info_out.shortTitle);
            info_out.flags = if info.flags.automatable {
                vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate as i32
            } else {
                0
            } | if self.bypass_id == Some(&param_id) {
                vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass as i32
            } else {
                0
            };

            match &info.type_specific {
                TypeSpecificInfo::Enum {
                    default,
                    ref values,
                } => {
                    info_out.flags |=
                        vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsList as i32;

                    assert!(
                        values.len() >= 2,
                        "Enum parameters must contain at least 2 values."
                    );
                    info_out.stepCount = i32::try_from(values.len()).unwrap() - 1;
                    info_out.defaultNormalizedValue =
                        // Note we checked that the number of values fit in an i32 on construction.
                        f64::from(*default) / f64::from(i32::try_from(values.len()).unwrap() - 1);
                    info_out.units[0] = 0;
                }
                TypeSpecificInfo::Numeric {
                    default,
                    valid_range,
                    ref units,
                    ..
                } => {
                    info_out.stepCount = 0;
                    info_out.defaultNormalizedValue = f64::from(
                        (default - valid_range.start()) / (valid_range.end() - valid_range.start()),
                    );
                    to_utf16(units, &mut info_out.units);
                }
                TypeSpecificInfo::Switch { default } => {
                    info_out.stepCount = 1;
                    info_out.defaultNormalizedValue = if *default { 1.0 } else { 0.0 };
                    info_out.units[0] = 0;
                }
            }

            vst3::Steinberg::kResultOk
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getParamStringByValue(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        value_normalized: vst3::Steinberg::Vst::ParamValue,
        string: *mut vst3::Steinberg::Vst::String128,
    ) -> vst3::Steinberg::tresult {
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            let ParameterStore { unhash, infos, .. } = &*store.store.borrow();
            match lookup_by_hash(id, unhash, infos) {
                Some(parameters::Info {
                    type_specific: TypeSpecificInfo::Numeric { valid_range, .. },
                    ..
                }) => {
                    let value = value_normalized
                        * f64::from(valid_range.end() - valid_range.start())
                        + f64::from(*valid_range.start());
                    let serialized = format!("{value:.2}");
                    to_utf16(serialized.as_str(), &mut *string);
                    vst3::Steinberg::kResultOk
                }
                Some(parameters::Info {
                    type_specific: TypeSpecificInfo::Enum { values, .. },
                    ..
                }) => {
                    #[allow(clippy::cast_possible_truncation)]
                    let index = (value_normalized * f64::from(u32::try_from(values.len()).unwrap()))
                        .min(f64::from(u32::try_from(values.len()).unwrap()) - 1.0)
                        .floor() as i32 as usize;
                    to_utf16(&values[index], &mut *string);
                    vst3::Steinberg::kResultOk
                }
                Some(parameters::Info {
                    type_specific: TypeSpecificInfo::Switch { .. },
                    ..
                }) => {
                    let serialized = if value_normalized > 0.5 { "On" } else { "Off" };

                    to_utf16(serialized, &mut *string);
                    vst3::Steinberg::kResultOk
                }
                _ => vst3::Steinberg::kInvalidArgument,
            }
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getParamValueByString(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        string: *mut vst3::Steinberg::Vst::TChar,
        value_normalized: *mut vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::tresult {
        // Note that VST3 doesn't put a limit on string sizes here,
        // so we make a reasonable size up.
        const MAX_STRING_SIZE: usize = 2049;

        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            let ParameterStore { unhash, infos, .. } = &*store.store.borrow();
            if let Some(string) = from_utf16_ptr(string, MAX_STRING_SIZE) {
                match lookup_by_hash(id, unhash, infos) {
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Numeric { valid_range, .. },
                        ..
                    }) => {
                        let value = string.parse::<f64>();
                        if let Ok(value) = value {
                            *value_normalized = (value - f64::from(*valid_range.start()))
                                / f64::from(valid_range.end() - valid_range.start());
                            vst3::Steinberg::kResultOk
                        } else {
                            vst3::Steinberg::kInvalidArgument
                        }
                    }
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Enum { values, .. },
                        ..
                    }) => {
                        if let Some(index) = values.iter().position(|v| v == &string) {
                            *value_normalized = f64::from(i32::try_from(index).unwrap())
                                / f64::from(i32::try_from(values.len()).unwrap() - 1);
                            vst3::Steinberg::kResultOk
                        } else {
                            vst3::Steinberg::kInvalidArgument
                        }
                    }
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { .. },
                        ..
                    }) => {
                        if string == "On" {
                            *value_normalized = 1.0;
                            vst3::Steinberg::kResultOk
                        } else if string == "Off" {
                            *value_normalized = 0.0;
                            vst3::Steinberg::kResultOk
                        } else {
                            vst3::Steinberg::kInvalidArgument
                        }
                    }
                    _ => vst3::Steinberg::kInvalidArgument,
                }
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn normalizedParamToPlain(
        &self,
        _id: vst3::Steinberg::Vst::ParamID,
        value_normalized: vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::Vst::ParamValue {
        // Note that this is a no-op.  In today's edition of "this is why we can't have nice things",
        // Ableton does not support parameters that have ranges other than 0.0->1.0 :(.
        //
        // So, we ignore the "plainParam" part of the VST3 spec to support ableton.
        value_normalized
    }

    unsafe fn plainParamToNormalized(
        &self,
        _id: vst3::Steinberg::Vst::ParamID,
        plain_value: vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::Vst::ParamValue {
        plain_value
    }

    unsafe fn getParamNormalized(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
    ) -> vst3::Steinberg::Vst::ParamValue {
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            let ParameterStore {
                unhash,
                infos,
                values,
                ..
            } = &*store.store.borrow();
            return match (
                lookup_by_hash(id, unhash, infos),
                lookup_by_hash(id, unhash, values),
            ) {
                (
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Numeric { valid_range, .. },
                        ..
                    }),
                    Some(parameters::InternalValue::Numeric(value)),
                ) => normalize_numeric(*value, valid_range),
                (
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Enum { values, .. },
                        ..
                    }),
                    Some(parameters::InternalValue::Enum(value)),
                ) => normalize_enum(*value, values.len().try_into().unwrap()),
                (
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { .. },
                        ..
                    }),
                    Some(parameters::InternalValue::Switch(value)),
                ) => normalize_switch(*value),
                // It's an error to call this with an invalid ID, but
                // we have no way of indicating an error here.
                (None, None) => 0.0,
                // We try to maintain an invariant that the param value and the param info
                // are always in sync, so this should never happen.
                _ => panic!(),
            };
        }

        // It's an error to call this before we're initialized, but we
        // have no way of indicating an error here.
        0.0
    }

    unsafe fn setParamNormalized(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        value: vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::tresult {
        if !(0.0..=1.0).contains(&value) {
            return vst3::Steinberg::kInvalidArgument;
        }
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow_mut().as_mut().unwrap()
        {
            let ParameterStore {
                unhash,
                infos,
                values,
                listener,
                ..
            } = &mut *store.store.borrow_mut();
            if let Some(id) = unhash.get(&id) {
                if let Some(value) = match infos.get(id) {
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Numeric { valid_range, .. },
                        ..
                    }) => Some(parameters::InternalValue::Numeric(convert_numeric(
                        value,
                        valid_range,
                    ))),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Enum { values, .. },
                        ..
                    }) => Some(parameters::InternalValue::Enum(convert_enum(
                        value,
                        values.len().try_into().unwrap(),
                    ))),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { .. },
                        ..
                    }) => Some(parameters::InternalValue::Switch(convert_switch(value))),
                    _ => None,
                } {
                    values.insert(id.to_string(), value);
                    if let Some(listener) = listener {
                        if let Some(listener) = listener.upgrade() {
                            (*listener).parameter_changed(id, &from_internal(id, value, infos));
                        }
                    }
                    return vst3::Steinberg::kResultOk;
                }
            }
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn setComponentHandler(
        &self,
        handler: *mut vst3::Steinberg::Vst::IComponentHandler,
    ) -> vst3::Steinberg::tresult {
        if let Some(handler) = ComRef::from_raw(handler).map(|handler| handler.to_com_ptr()) {
            if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap()
            {
                store.store.borrow_mut().component_handler = Some(handler);
                return vst3::Steinberg::kResultOk;
            }
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn createView(
        &self,
        name: vst3::Steinberg::FIDString,
    ) -> *mut vst3::Steinberg::IPlugView {
        if std::ffi::CStr::from_ptr(name).to_str() == Ok("editor") {
            if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap()
            {
                return view::create(
                    store.clone(),
                    get_current_bundle_info()
                        .expect("Could not find bundle info")
                        .identifier
                        .clone(),
                    self.ui_initial_size,
                )
                .into_raw();
            }
        }
        std::ptr::null_mut()
    }
}

impl IMidiMappingTrait for EditController {
    unsafe fn getMidiControllerAssignment(
        &self,
        bus_index: vst3::Steinberg::int32,
        channel_index: vst3::Steinberg::int16,
        midi_controller_number: vst3::Steinberg::Vst::CtrlNumber,
        id: *mut vst3::Steinberg::Vst::ParamID,
    ) -> vst3::Steinberg::tresult {
        if bus_index != 0 {
            return vst3::Steinberg::kResultFalse;
        }
        if channel_index != 0 {
            return vst3::Steinberg::kResultFalse;
        }
        match match midi_controller_number.try_into() {
            Ok(vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend) => Some(PITCH_BEND_PARAMETER),
            Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel) => {
                Some(MOD_WHEEL_PARAMETER)
            }
            Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlExpression) => {
                Some(EXPRESSION_PARAMETER)
            }
            Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlSustainOnOff) => {
                Some(SUSTAIN_PARAMETER)
            }
            Ok(vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch) => Some(AFTERTOUCH_PARAMETER),
            _ => None,
        } {
            Some(param_id) => {
                *id = parameters::hash_id(param_id);
                vst3::Steinberg::kResultOk
            }
            _ => vst3::Steinberg::kResultFalse,
        }
    }
}

impl Class for EditController {
    type Interfaces = (IPluginBase, IEditController, IMidiMapping);
}
