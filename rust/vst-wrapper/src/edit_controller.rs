// Note that previous versions of the SDK had incorrect typing for the `ParameterFlags` enum,
// and we make a best-effort attempt to support these older versions. This requires casts
// that are unnecessary in newer versions.
#![allow(clippy::unnecessary_cast)]

use crate::{
    i32_to_enum,
    parameters::{
        AFTERTOUCH_PARAMETER, CONTROLLER_PARAMETERS, EXPRESSION_PEDAL_PARAMETER,
        MOD_WHEEL_PARAMETER, PITCH_BEND_PARAMETER, SUSTAIN_PARAMETER, TIMBRE_PARAMETER,
    },
    u32_to_enum,
};
use std::{
    cell::RefCell,
    collections::{HashMap, hash_map},
    io::{Read, Write},
    rc,
};

use conformal_component::parameters::{self, InfoRef, TypeSpecificInfo, TypeSpecificInfoRef};
use conformal_core::parameters::serialization::{DeserializationError, ReadInfoRef};
use conformal_core::parameters::store;

use conformal_ui::Size;
use vst3::{
    Class, ComPtr, ComRef,
    Steinberg::{
        IPluginBase, IPluginBaseTrait,
        Vst::{
            IComponentHandler, IComponentHandler2, IComponentHandler2Trait, IComponentHandlerTrait,
            IConnectionPoint, IConnectionPointTrait, IEditController, IEditControllerTrait,
            IHostApplication, IMidiMapping, IMidiMappingTrait, INoteExpressionController,
            INoteExpressionControllerTrait, INoteExpressionPhysicalUIMapping,
            INoteExpressionPhysicalUIMappingTrait, NoteExpressionTypeID, NoteExpressionTypeInfo,
            NoteExpressionValue,
        },
    },
};

use crate::{
    ParameterModel,
    io::StreamWrite,
    mpe::quirks::{aftertouch_param_id, pitch_param_id, timbre_param_id},
};

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
    unhash: parameters::IdHashMap<String>,

    /// Parameters actually exposed by the component
    component_parameter_infos: HashMap<String, parameters::Info>,

    /// All parameters we expose to the host.  This includes:
    ///  - component parameters
    ///  - controller parameters (in Conformal, we don't think of these as "parameters",
    ///    but they are VST3 paramaeters. This includes e.g., mod wheel.)
    ///  - quirks parameters that are only needed for buggy implementations of note expression in some host
    host_parameter_infos: HashMap<String, parameters::Info>,

    values: HashMap<String, parameters::InternalValue>,
    order: Vec<String>,

    component_handler: Option<ComPtr<IComponentHandler>>,

    ui_state: Vec<u8>,

    // Note that unsized weak types can't dangle, so we use Option here to allow dangling.
    listener: Option<rc::Weak<dyn store::Listener>>,
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
    hash_to_id: &parameters::IdHashMap<String>,
    values: &'a HashMap<String, T>,
) -> Option<&'a T> {
    hash_to_id.get(&hash).and_then(|id| values.get(id))
}

enum State {
    ReadyForInitialization(ParameterModel, String),
    Initialized(Initialized),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Synth(),
    Effect { bypass_id: &'static str },
}

struct EditController {
    s: RefCell<Option<State>>,
    host: RefCell<Option<ComPtr<IHostApplication>>>,
    ui_initial_size: Size,
    kind: Kind,
}

// Brought out to a separate function for ease of testing
fn create_internal(
    parameter_model: ParameterModel,
    pref_domain: String,
    ui_initial_size: Size,
    kind: Kind,
) -> EditController {
    EditController {
        s: Some(State::ReadyForInitialization(parameter_model, pref_domain)).into(),
        host: Default::default(),
        ui_initial_size,
        kind,
    }
}

#[cfg(target_os = "macos")]
fn get_pref_domain() -> String {
    use conformal_core::mac_bundle_utils::get_current_bundle_info;

    get_current_bundle_info()
        .expect("Could not find bundle info")
        .identifier
}

#[cfg(target_os = "windows")]
fn get_pref_domain() -> String {
    use conformal_core::windows_dll_utils::get_current_dll_info;

    let info = get_current_dll_info().expect("Could not find DLL info");
    format!("{}.{}", info.company_name, info.internal_name)
}

pub fn create(
    parameter_model: ParameterModel,
    ui_initial_size: Size,
    kind: Kind,
) -> impl Class<
    Interfaces = (
        IPluginBase,
        IEditController,
        IMidiMapping,
        IConnectionPoint,
        INoteExpressionController,
        INoteExpressionPhysicalUIMapping,
    ),
> + IEditControllerTrait
+ IMidiMappingTrait
+ IConnectionPointTrait
+ INoteExpressionControllerTrait
+ INoteExpressionPhysicalUIMappingTrait
+ 'static {
    create_internal(parameter_model, get_pref_domain(), ui_initial_size, kind)
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
            parameters::Value::Enum(values[v as usize].clone())
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

impl store::Store for SharedStore {
    fn get(&self, id: &str) -> Option<parameters::Value> {
        self.store
            .borrow()
            .values
            .get(id)
            .map(|v| from_internal(id, *v, &self.store.borrow().host_parameter_infos))
    }

    fn set_listener(&mut self, listener: rc::Weak<dyn store::Listener>) {
        self.store.borrow_mut().listener = Some(listener);
    }

    fn set(&mut self, unique_id: &str, value: parameters::Value) -> Result<(), store::SetError> {
        let maybe_set = if let ParameterStore {
            component_handler: Some(component_handler),
            host_parameter_infos,
            values,
            ..
        } = &mut (*self.store.borrow_mut())
        {
            (match (&value, host_parameter_infos.get(unique_id)) {
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
                        Err(store::SetError::InvalidValue)
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
                    .ok_or(store::SetError::InvalidValue),
                (
                    parameters::Value::Switch(value),
                    Some(parameters::Info {
                        type_specific: TypeSpecificInfo::Switch { .. },
                        ..
                    }),
                ) => Ok(normalize_switch(*value)),
                (_, Some(_)) => Err(store::SetError::WrongType),
                (_, None) => Err(store::SetError::NotFound),
            })
            .map(|v| {
                values.insert(
                    unique_id.to_string(),
                    to_internal(unique_id, &value, host_parameter_infos),
                );
                (component_handler.clone(), parameters::hash_id(unique_id), v)
            })
        } else {
            Err(store::SetError::InternalError)
        };
        maybe_set.map(|(component_handler, hash, v)| {
            unsafe {
                component_handler.performEdit(hash.internal_hash(), v);
            };
        })
    }

    fn set_grabbed(
        &mut self,
        unique_id: &str,
        grabbed: bool,
    ) -> Result<(), store::SetGrabbedError> {
        let maybe_set = if let ParameterStore {
            component_handler: Some(component_handler),
            host_parameter_infos,
            ..
        } = &(*self.store.borrow())
        {
            if host_parameter_infos.contains_key(unique_id) {
                Ok((component_handler.clone(), parameters::hash_id(unique_id)))
            } else {
                Err(store::SetGrabbedError::NotFound)
            }
        } else {
            Err(store::SetGrabbedError::InternalError)
        };
        maybe_set.map(|(component_handler, hashed)| {
            if grabbed {
                unsafe {
                    component_handler.beginEdit(hashed.internal_hash());
                }
            } else {
                unsafe {
                    component_handler.endEdit(hashed.internal_hash());
                }
            }
        })
    }

    fn get_info(&self, unique_id: &str) -> Option<parameters::Info> {
        self.store
            .borrow()
            .host_parameter_infos
            .get(unique_id)
            .cloned()
    }

    fn set_ui_state(&mut self, state: &[u8]) {
        // Early exit if the state is the same
        if self.store.borrow().ui_state == state {
            return;
        }

        // Update the UI state
        {
            let mut store = self.store.borrow_mut();
            store.ui_state.clear();
            store.ui_state.extend_from_slice(state);
        }

        // Notify the listener
        if let Some(listener) = self.store.borrow_mut().listener.as_ref()
            && let Some(listener) = listener.upgrade()
        {
            listener.ui_state_changed(state);
        }

        // Tell the host
        if let ParameterStore {
            component_handler: Some(component_handler),
            ..
        } = &(*self.store.borrow())
            && let Some(component_handler2) = component_handler.cast::<IComponentHandler2>()
        {
            unsafe {
                component_handler2.setDirty(1);
            }
        }
    }

    fn get_ui_state(&self) -> Vec<u8> {
        self.store.borrow().ui_state.clone()
    }
}

/// For testing only.
#[cfg(test)]
trait GetStore {
    type Store: store::Store;
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
        unsafe {
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
                        if Kind::Synth() == self.kind {
                            infos.extend(
                                CONTROLLER_PARAMETERS
                                    .iter()
                                    .map(parameters::Info::from)
                                    .chain(crate::mpe::quirks::parameters()),
                            );
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
                    if let Kind::Effect { bypass_id } = self.kind {
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
                    let component_parameters = parameters
                        .iter()
                        .filter(|(id, _)| {
                            crate::parameters::should_include_parameter_in_snapshot(id)
                        })
                        .map(|(id, info)| (id.clone(), info.clone()))
                        .collect();
                    let s = State::Initialized(Initialized {
                    store: SharedStore {store: rc::Rc::new(RefCell::new(ParameterStore {
                        unhash: hash_parameter_ids(parameter_infos.iter().map(Into::into)).expect("Duplicate parameter ID hash! This could be caused by duplicate parameter IDs or a hash collision."),
                        host_parameter_infos: parameters,
                        component_parameter_infos: component_parameters,
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
                        ui_state: Default::default(),
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
) -> Option<parameters::IdHashMap<String>> {
    let mut hash_to_id = parameters::IdHashMap::default();
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
        unsafe {
            if let State::Initialized(Initialized { store, .. }) =
                self.s.borrow_mut().as_mut().unwrap()
            {
                let &mut ParameterStore {
                    component_parameter_infos: ref infos,
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
                                    snapshot.values.iter().map(|(k, v)| {
                                        (k.as_str(), to_internal(k.as_str(), v, infos))
                                    }),
                                    values,
                                );

                                for (id, value) in &snapshot.values {
                                    if let Some(listener) = listener
                                        && let Some(listener) = listener.upgrade()
                                    {
                                        listener.parameter_changed(id.as_str(), value);
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
    }

    unsafe fn setState(&self, state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        unsafe {
            if let State::Initialized(Initialized { store, .. }) =
                self.s.borrow_mut().as_mut().unwrap()
            {
                let ParameterStore {
                    ui_state, listener, ..
                } = &mut *store.store.borrow_mut();
                if let Some(com_stream) = ComRef::from_raw(state) {
                    let mut new_state = Vec::new();
                    if StreamRead::new(com_stream)
                        .read_to_end(&mut new_state)
                        .is_ok()
                    {
                        *ui_state = new_state;
                        if let Some(listener) = listener
                            && let Some(listener) = listener.upgrade()
                        {
                            listener.ui_state_changed(ui_state);
                        }
                        return vst3::Steinberg::kResultOk;
                    }
                    return vst3::Steinberg::kInternalError;
                }
            }
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getState(&self, state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        unsafe {
            if let State::Initialized(Initialized { store, .. }) =
                self.s.borrow_mut().as_mut().unwrap()
            {
                let ParameterStore { ui_state, .. } = &*store.store.borrow();

                if let Some(com_state) = ComRef::from_raw(state) {
                    let mut writer = StreamWrite::new(com_state);
                    if writer.write_all(ui_state).is_ok() {
                        return vst3::Steinberg::kResultOk;
                    }
                    return vst3::Steinberg::kInternalError;
                }
            }
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getParameterCount(&self) -> vst3::Steinberg::int32 {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self.s.borrow().as_ref().unwrap() {
            State::Initialized(s) => {
                i32::try_from(s.store.store.borrow().host_parameter_infos.len()).unwrap()
            }
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
        unsafe {
            if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap()
            {
                let ParameterStore {
                    host_parameter_infos: infos,
                    order,
                    ..
                } = &*store.store.borrow();
                if param_index < 0 || param_index as usize >= order.len() {
                    return vst3::Steinberg::kInvalidArgument;
                }
                let param_id = order[param_index as usize].clone();
                let param_hash = parameters::hash_id(&param_id);
                let info = infos.get(&param_id).unwrap();

                let info_out = &mut *info_out;
                info_out.id = param_hash.internal_hash();
                info_out.unitId = 0;
                to_utf16(&info.title, &mut info_out.title);
                to_utf16(&info.short_title, &mut info_out.shortTitle);
                info_out.flags = if info.flags.automatable {
                    vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate as i32
                } else {
                    0
                } | if let Kind::Effect { bypass_id } = self.kind {
                    if param_id == bypass_id {
                        vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass as i32
                    } else {
                        0
                    }
                } else {
                    0
                };

                match &info.type_specific {
                    TypeSpecificInfo::Enum { default, values } => {
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
                        units,
                        ..
                    } => {
                        info_out.stepCount = 0;
                        info_out.defaultNormalizedValue = f64::from(
                            (default - valid_range.start())
                                / (valid_range.end() - valid_range.start()),
                        );
                        to_utf16(
                            units.as_ref().map_or("", |x| x.as_str()),
                            &mut info_out.units,
                        );
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
    }

    unsafe fn getParamStringByValue(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        value_normalized: vst3::Steinberg::Vst::ParamValue,
        string: *mut vst3::Steinberg::Vst::String128,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap()
            {
                let ParameterStore {
                    unhash,
                    host_parameter_infos: infos,
                    ..
                } = &*store.store.borrow();
                match lookup_by_hash(parameters::id_hash_from_internal_hash(id), unhash, infos) {
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
                        let index = (value_normalized
                            * f64::from(u32::try_from(values.len()).unwrap()))
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
    }

    unsafe fn getParamValueByString(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        string: *mut vst3::Steinberg::Vst::TChar,
        value_normalized: *mut vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            // Note that VST3 doesn't put a limit on string sizes here,
            // so we make a reasonable size up.
            const MAX_STRING_SIZE: usize = 2049;

            if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap()
            {
                let ParameterStore {
                    unhash,
                    host_parameter_infos: infos,
                    ..
                } = &*store.store.borrow();
                if let Some(string) = from_utf16_ptr(string, MAX_STRING_SIZE) {
                    match lookup_by_hash(parameters::id_hash_from_internal_hash(id), unhash, infos)
                    {
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
        let id = parameters::id_hash_from_internal_hash(id);
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow().as_ref().unwrap() {
            let ParameterStore {
                unhash,
                host_parameter_infos: infos,
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
        let id = parameters::id_hash_from_internal_hash(id);
        if !(0.0..=1.0).contains(&value) {
            return vst3::Steinberg::kInvalidArgument;
        }
        if let State::Initialized(Initialized { store, .. }) = self.s.borrow_mut().as_mut().unwrap()
        {
            let ParameterStore {
                unhash,
                host_parameter_infos: infos,
                values,
                listener,
                ..
            } = &mut *store.store.borrow_mut();
            if let Some(id) = unhash.get(&id)
                && let Some(value) = match infos.get(id) {
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
                }
            {
                values.insert(id.clone(), value);
                if let Some(listener) = listener
                    && let Some(listener) = listener.upgrade()
                {
                    (*listener).parameter_changed(id, &from_internal(id, value, infos));
                }
                return vst3::Steinberg::kResultOk;
            }
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn setComponentHandler(
        &self,
        handler: *mut vst3::Steinberg::Vst::IComponentHandler,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(handler) = ComRef::from_raw(handler).map(|handler| handler.to_com_ptr())
                && let State::Initialized(Initialized { store, .. }) =
                    self.s.borrow().as_ref().unwrap()
            {
                store.store.borrow_mut().component_handler = Some(handler);
                return vst3::Steinberg::kResultOk;
            }
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn createView(
        &self,
        name: vst3::Steinberg::FIDString,
    ) -> *mut vst3::Steinberg::IPlugView {
        unsafe {
            if std::ffi::CStr::from_ptr(name).to_str() == Ok("editor")
                && let State::Initialized(Initialized { store, .. }) =
                    self.s.borrow().as_ref().unwrap()
            {
                return view::create(store.clone(), get_pref_domain(), self.ui_initial_size)
                    .into_raw();
            }
            std::ptr::null_mut()
        }
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
        unsafe {
            if let State::Initialized(Initialized { .. }) = self.s.borrow().as_ref().unwrap() {
                // Effects don't have midi mappings
                if let Kind::Effect { .. } = self.kind {
                    return vst3::Steinberg::kResultFalse;
                }
                if bus_index != 0 {
                    return vst3::Steinberg::kResultFalse;
                }
                if channel_index != 0 {
                    // Supply MPE quirks mappings for hosts like Ableton that use "quirks" MPE.
                    (match i32_to_enum(midi_controller_number.into()) {
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend) => {
                            Some(pitch_param_id(channel_index))
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlFilterResonance) => {
                            Some(timbre_param_id(channel_index))
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch) => {
                            Some(aftertouch_param_id(channel_index))
                        }
                        _ => None,
                    })
                    .map_or(vst3::Steinberg::kResultFalse, |param_id| {
                        *id = parameters::hash_id(&param_id).internal_hash();
                        vst3::Steinberg::kResultOk
                    })
                } else {
                    (match i32_to_enum(midi_controller_number.into()) {
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend) => {
                            Some(PITCH_BEND_PARAMETER)
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel) => {
                            Some(MOD_WHEEL_PARAMETER)
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlExpression) => {
                            Some(EXPRESSION_PEDAL_PARAMETER)
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlSustainOnOff) => {
                            Some(SUSTAIN_PARAMETER)
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch) => {
                            Some(AFTERTOUCH_PARAMETER)
                        }
                        Ok(vst3::Steinberg::Vst::ControllerNumbers_::kCtrlFilterResonance) => {
                            Some(TIMBRE_PARAMETER)
                        }
                        _ => None,
                    })
                    .map_or(vst3::Steinberg::kResultFalse, |param_id| {
                        *id = parameters::hash_id(param_id).internal_hash();
                        vst3::Steinberg::kResultOk
                    })
                }
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }
}

impl IConnectionPointTrait for EditController {
    unsafe fn connect(&self, _: *mut IConnectionPoint) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }

    unsafe fn disconnect(&self, _: *mut IConnectionPoint) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }

    unsafe fn notify(&self, _: *mut vst3::Steinberg::Vst::IMessage) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }
}

impl INoteExpressionControllerTrait for EditController {
    unsafe fn getNoteExpressionCount(&self, bus_index: i32, channel: i16) -> i32 {
        if !matches!(self.s.borrow().as_ref().unwrap(), State::Initialized(_)) {
            // Note - it is a host error to call this function if we are not intialized.
            // However, this function has no way to return an error message, so we just return 0.
            return 0;
        }
        if bus_index != 0 {
            return 0;
        }
        if channel != 0 {
            return 0;
        }
        3
    }

    unsafe fn getNoteExpressionInfo(
        &self,
        bus_index: i32,
        channel: i16,
        note_expression_index: i32,
        info_out: *mut NoteExpressionTypeInfo,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if !matches!(self.s.borrow().as_ref().unwrap(), State::Initialized(_)) {
                return vst3::Steinberg::kInvalidArgument;
            }
            if bus_index != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            if channel != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            let info_out = &mut *info_out;

            match note_expression_index {
                0 => {
                    info_out.typeId = vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID;
                    to_utf16("Pitch Bend", &mut info_out.title);
                    to_utf16("Pitch", &mut info_out.shortTitle);
                    to_utf16("semitones", &mut info_out.units);
                    info_out.unitId = 0;
                    // It's not clear from docs if this is necessary for a pre-defined tuning type.
                    info_out.valueDesc = vst3::Steinberg::Vst::NoteExpressionValueDescription {
                        defaultValue: 0.5,
                        minimum: 0.0,
                        maximum: 1.0,
                        stepCount: 0, // Continuous
                    };
                    info_out.flags = vst3::Steinberg::Vst::NoteExpressionTypeInfo_::NoteExpressionTypeFlags_::kIsBipolar as i32;
                    vst3::Steinberg::kResultOk
                }
                1 => {
                    info_out.typeId = crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID;
                    to_utf16("Timbre", &mut info_out.title);
                    to_utf16("Timbre", &mut info_out.shortTitle);
                    to_utf16("", &mut info_out.units);
                    info_out.unitId = 0;
                    info_out.valueDesc = vst3::Steinberg::Vst::NoteExpressionValueDescription {
                        defaultValue: 0.0,
                        minimum: 0.0,
                        maximum: 1.0,
                        stepCount: 0, // Continuous
                    };
                    info_out.flags = 0;

                    vst3::Steinberg::kResultOk
                }
                2 => {
                    info_out.typeId = crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID;
                    to_utf16("Aftertouch", &mut info_out.title);
                    to_utf16("Aftertouch", &mut info_out.shortTitle);
                    to_utf16("", &mut info_out.units);
                    info_out.unitId = 0;
                    info_out.valueDesc = vst3::Steinberg::Vst::NoteExpressionValueDescription {
                        defaultValue: 0.0,
                        minimum: 0.0,
                        maximum: 1.0,
                        stepCount: 0, // Continuous
                    };
                    info_out.flags = 0;

                    vst3::Steinberg::kResultOk
                }
                _ => vst3::Steinberg::kInvalidArgument,
            }
        }
    }

    unsafe fn getNoteExpressionStringByValue(
        &self,
        bus_index: i32,
        channel: i16,
        id: NoteExpressionTypeID,
        value_normalized: NoteExpressionValue,
        string: *mut vst3::Steinberg::Vst::String128,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if !matches!(self.s.borrow().as_ref().unwrap(), State::Initialized(_)) {
                return vst3::Steinberg::kInvalidArgument;
            }
            if bus_index != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            if channel != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            match id {
                vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID => {
                    let value = (value_normalized - 0.5) * 240.0;
                    to_utf16(&format!("{value:.2}"), &mut *string);
                    vst3::Steinberg::kResultOk
                }
                crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID
                | crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID => {
                    to_utf16(&format!("{value_normalized:.2}"), &mut *string);
                    vst3::Steinberg::kResultOk
                }
                _ => vst3::Steinberg::kInvalidArgument,
            }
        }
    }

    unsafe fn getNoteExpressionValueByString(
        &self,
        bus_index: i32,
        channel: i16,
        id: NoteExpressionTypeID,
        string: *const vst3::Steinberg::Vst::TChar,
        value_normalized: *mut NoteExpressionValue,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            // Note that VST3 doesn't put a limit on string sizes here,
            // so we make a reasonable size up.
            const MAX_STRING_SIZE: usize = 2049;

            if !matches!(self.s.borrow().as_ref().unwrap(), State::Initialized(_)) {
                return vst3::Steinberg::kInvalidArgument;
            }
            if bus_index != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            if channel != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            if let Some(value) = (|| -> Option<f64> {
                let string = from_utf16_ptr(string, MAX_STRING_SIZE)?;
                let value = string.parse::<f64>().ok()?;
                match id {
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID => {
                        Some((value / 240.0) + 0.5)
                    }
                    crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID
                    | crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID => Some(value),
                    _ => None,
                }
            })() {
                *value_normalized = value;
                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kResultFalse
            }
        }
    }
}

impl INoteExpressionPhysicalUIMappingTrait for EditController {
    unsafe fn getPhysicalUIMapping(
        &self,
        bus_index: i32,
        channel: i16,
        list: *mut vst3::Steinberg::Vst::PhysicalUIMapList,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if !matches!(self.s.borrow().as_ref().unwrap(), State::Initialized(_)) {
                return vst3::Steinberg::kInvalidArgument;
            }
            if bus_index != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }
            if channel != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }

            let list = &mut *list;
            for idx in 0..list.count {
                let item = &mut (*list.map.offset(idx as isize));
                match u32_to_enum(item.physicalUITypeID) {
                    Ok(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIXMovement) => {
                        item.noteExpressionTypeID =
                            vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID;
                    }
                    Ok(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIYMovement) => {
                        item.noteExpressionTypeID =
                            crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID;
                    }
                    Ok(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIPressure) => {
                        item.noteExpressionTypeID =
                            crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID;
                    }
                    _ => {}
                }
            }
            vst3::Steinberg::kResultOk
        }
    }
}

impl Class for EditController {
    type Interfaces = (
        IPluginBase,
        IEditController,
        IMidiMapping,
        IConnectionPoint,
        INoteExpressionController,
        INoteExpressionPhysicalUIMapping,
    );
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc;

    use conformal_component::synth::{HandleEventsContext, ProcessContext};
    use vst3::Class;
    use vst3::Steinberg::Vst::{
        IAudioProcessorTrait, IComponentHandler, IComponentHandlerTrait, IComponentTrait,
        IHostApplication, IMidiMappingTrait, INoteExpressionControllerTrait,
        INoteExpressionPhysicalUIMappingTrait, PhysicalUIMap,
    };
    use vst3::Steinberg::{IBStreamTrait, IPluginBaseTrait};
    use vst3::{ComWrapper, Steinberg::Vst::IEditControllerTrait};

    use super::GetStore;
    use crate::fake_ibstream::Stream;
    use crate::parameters::{
        AFTERTOUCH_PARAMETER, EXPRESSION_PEDAL_PARAMETER, MOD_WHEEL_PARAMETER,
        PITCH_BEND_PARAMETER, SUSTAIN_PARAMETER,
    };
    use crate::processor::test_utils::{
        ParameterValueQueueImpl, ParameterValueQueuePoint, mock_no_audio_process_data, setup_proc,
    };
    use crate::{HostInfo, enum_to_u32};
    use crate::{ParameterModel, processor};
    use crate::{dummy_host, from_utf16_buffer, to_utf16};
    use assert_approx_eq::assert_approx_eq;
    use conformal_component::audio::BufferMut;
    use conformal_component::parameters::{self, Flags, StaticInfoRef, hash_id};
    use conformal_component::{
        Component, ProcessingEnvironment, Processor,
        parameters::{InfoRef, TypeSpecificInfoRef},
        synth::Synth,
    };
    use conformal_core::parameters::store;
    use conformal_core::parameters::store::Store;

    #[derive(Default)]
    struct DummyComponent {}

    #[derive(Default)]
    struct DummySynth {}

    impl Processor for DummySynth {
        fn set_processing(&mut self, _processing: bool) {}
    }

    impl Synth for DummySynth {
        fn handle_events(&mut self, _context: &impl HandleEventsContext) {}

        fn process(&mut self, _context: &impl ProcessContext, _output: &mut impl BufferMut) {}
    }

    static DEFAULT_NUMERIC: f32 = 2.0;
    static MIN_NUMERIC: f32 = 1.0;
    static MAX_NUMERIC: f32 = 10.0;
    static NUMERIC_EPSILON: f64 = 1e-7;

    static NUMERIC_ID: &str = "numeric";
    static ENUM_ID: &str = "enum";
    static SWITCH_ID: &str = "switch";

    fn numeric_hash() -> u32 {
        parameters::hash_id(NUMERIC_ID).internal_hash()
    }

    fn enum_hash() -> u32 {
        parameters::hash_id(ENUM_ID).internal_hash()
    }

    fn switch_hash() -> u32 {
        parameters::hash_id(SWITCH_ID).internal_hash()
    }

    static PARAMETERS: [InfoRef<'static, &'static str>; 3] = [
        InfoRef {
            title: "Test Numeric",
            short_title: "Num",
            unique_id: NUMERIC_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Numeric {
                default: DEFAULT_NUMERIC,
                valid_range: MIN_NUMERIC..=MAX_NUMERIC,
                units: Some("Hz"),
            },
        },
        InfoRef {
            title: "Test Enum",
            short_title: "Enum",
            unique_id: ENUM_ID,
            flags: Flags { automatable: false },
            type_specific: TypeSpecificInfoRef::Enum {
                default: 0,
                values: &["A", "B", "C"],
            },
        },
        InfoRef {
            title: "Test Switch",
            short_title: "Switch",
            unique_id: SWITCH_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Switch { default: false },
        },
    ];

    impl Component for DummyComponent {
        type Processor = DummySynth;

        fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
            Default::default()
        }

        fn parameter_infos(&self) -> Vec<parameters::Info> {
            parameters::to_infos(&PARAMETERS)
        }
    }

    static INCOMPATIBLE_PARAMETERS: [StaticInfoRef; 1] = [InfoRef {
        title: "Test Numeric",
        short_title: "Num",
        unique_id: SWITCH_ID, // This is incompatible since the previous version used this ID for a switch
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=MAX_NUMERIC,
            units: Some("Hz"),
        },
    }];

    #[derive(Default)]
    struct IncompatibleComponent {}

    impl Component for IncompatibleComponent {
        type Processor = DummySynth;

        fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
            Default::default()
        }

        fn parameter_infos(&self) -> Vec<parameters::Info> {
            parameters::to_infos(&INCOMPATIBLE_PARAMETERS)
        }
    }

    static NEWER_PARAMETERS: [StaticInfoRef; 3] = [
        InfoRef {
            title: "Test Numeric",
            short_title: "Num",
            unique_id: NUMERIC_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Numeric {
                default: DEFAULT_NUMERIC,
                valid_range: MIN_NUMERIC..=20.0,
                units: Some("Hz"),
            },
        },
        InfoRef {
            title: "Test Enum",
            short_title: "Enum",
            unique_id: ENUM_ID,
            flags: Flags { automatable: false },
            type_specific: TypeSpecificInfoRef::Enum {
                default: 0,
                values: &["A", "B", "C"],
            },
        },
        InfoRef {
            title: "Test Switch",
            short_title: "Switch",
            unique_id: SWITCH_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Switch { default: false },
        },
    ];

    #[derive(Default)]
    struct NewerComponent {}

    impl Component for NewerComponent {
        type Processor = DummySynth;

        fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
            Default::default()
        }

        fn parameter_infos(&self) -> Vec<parameters::Info> {
            parameters::to_infos(&NEWER_PARAMETERS)
        }
    }

    static DUPLICATE_PARAMETERS: [StaticInfoRef; 2] = [
        InfoRef {
            title: "Test Numeric",
            short_title: "Num",
            unique_id: NUMERIC_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Numeric {
                default: DEFAULT_NUMERIC,
                valid_range: MIN_NUMERIC..=20.0,
                units: Some("Hz"),
            },
        },
        InfoRef {
            title: "Test Numeric",
            short_title: "Num",
            unique_id: NUMERIC_ID,
            flags: Flags { automatable: true },
            type_specific: TypeSpecificInfoRef::Numeric {
                default: DEFAULT_NUMERIC,
                valid_range: MIN_NUMERIC..=20.0,
                units: Some("Hz"),
            },
        },
    ];

    fn create_parameter_model<F: Fn(&HostInfo) -> Vec<parameters::Info> + 'static>(
        f: F,
    ) -> ParameterModel {
        ParameterModel {
            parameter_infos: Box::new(f),
        }
    }

    fn dummy_edit_controller() -> impl IEditControllerTrait + IMidiMappingTrait + GetStore {
        super::create_internal(
            create_parameter_model(|_: &HostInfo| parameters::to_infos(&PARAMETERS)),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: SWITCH_ID,
            },
        )
    }

    fn dummy_processor() -> impl IComponentTrait + IAudioProcessorTrait {
        processor::create_synth(
            |_: &HostInfo| -> DummyComponent { Default::default() },
            [4; 16],
        )
    }

    #[test]
    fn defends_against_initializing_twice() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
        unsafe {
            assert_ne!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn defends_against_termination_before_initialization() {
        let ec = dummy_edit_controller();
        unsafe { assert_ne!(ec.terminate(), vst3::Steinberg::kResultOk) }
    }

    #[test]
    fn allow_initialize_twice_with_terminate() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
        unsafe { assert_eq!(ec.terminate(), vst3::Steinberg::kResultOk) }
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn parameter_basics() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
        let num_params = unsafe { ec.getParameterCount() };
        assert_eq!(num_params, i32::try_from(PARAMETERS.len()).unwrap());

        let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
            id: 0,
            title: [0; 128],
            shortTitle: [0; 128],
            units: [0; 128],
            stepCount: 0,
            defaultNormalizedValue: 0f64,
            unitId: 0,
            flags: 0,
        };

        unsafe {
            assert_eq!(
                ec.getParameterInfo(0, &mut param_info),
                vst3::Steinberg::kResultOk
            );
        }

        assert_eq!(param_info.id, numeric_hash());
        assert_eq!(
            from_utf16_buffer(&param_info.title),
            Some(PARAMETERS[0].title.to_string())
        );
        assert_eq!(
            from_utf16_buffer(&param_info.shortTitle),
            Some(PARAMETERS[0].short_title.to_string())
        );
        assert_eq!(
            param_info.flags,
            vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate as i32
        );
        assert_eq!(param_info.stepCount, 0);
        assert_eq!(from_utf16_buffer(&param_info.units), Some("Hz".to_string()));

        unsafe {
            assert_eq!(
                ec.getParameterInfo(1, &mut param_info),
                vst3::Steinberg::kResultOk
            );
        }

        assert_eq!(param_info.id, enum_hash());
        assert_eq!(
            from_utf16_buffer(&param_info.title),
            Some(PARAMETERS[1].title.to_string())
        );
        assert_eq!(
            from_utf16_buffer(&param_info.shortTitle),
            Some(PARAMETERS[1].short_title.to_string())
        );
        assert_eq!(
            param_info.flags,
            vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsList as i32
        );
        assert_eq!(param_info.stepCount, 2);
        assert_eq!(from_utf16_buffer(&param_info.units), Some("".to_string()));
        assert_eq!(param_info.defaultNormalizedValue, 0.0);

        unsafe {
            assert_eq!(
                ec.getParameterInfo(2, &mut param_info),
                vst3::Steinberg::kResultOk
            );
        }

        assert_eq!(param_info.id, switch_hash());
        assert_eq!(
            from_utf16_buffer(&param_info.title),
            Some(PARAMETERS[2].title.to_string())
        );
        assert_eq!(
            from_utf16_buffer(&param_info.shortTitle),
            Some(PARAMETERS[2].short_title.to_string())
        );
        assert_eq!(
            param_info.flags,
            (vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate
                | vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass)
                as i32
        );
        assert_eq!(param_info.stepCount, 1);
        assert_eq!(from_utf16_buffer(&param_info.units), Some("".to_string()));
        assert_eq!(param_info.defaultNormalizedValue, 0.0);
    }

    #[test]
    fn defends_against_count_without_initialize() {
        let ec = dummy_edit_controller();
        unsafe {
            assert_eq!(ec.getParameterCount(), 0);
        }
    }

    #[test]
    fn defends_against_get_param_info_without_initialize() {
        let ec = dummy_edit_controller();
        let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
            id: 0,
            title: [0; 128],
            shortTitle: [0; 128],
            units: [0; 128],
            stepCount: 0,
            defaultNormalizedValue: 0f64,
            unitId: 0,
            flags: 0,
        };
        assert_ne!(
            unsafe { ec.getParameterInfo(0, &mut param_info) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_bad_param_index() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            )
        }

        let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
            id: 0,
            title: [0; 128],
            shortTitle: [0; 128],
            units: [0; 128],
            stepCount: 0,
            defaultNormalizedValue: 0f64,
            unitId: 0,
            flags: 0,
        };
        unsafe {
            assert_ne!(
                ec.getParameterInfo(-1, &mut param_info),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getParameterInfo(77, &mut param_info),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn normalization_basics() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            )
        }

        // Test numeric parameter normalized -> plain.
        // Note that this should be the identity function (see note in implementation)
        unsafe {
            assert_eq!(ec.normalizedParamToPlain(numeric_hash(), 0.0), 0.0);
            assert_eq!(ec.normalizedParamToPlain(numeric_hash(), 1.0), 1.0);
            assert!((ec.normalizedParamToPlain(numeric_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
        }

        // Test numeric parameter plain -> normalized
        unsafe {
            assert_eq!(ec.plainParamToNormalized(numeric_hash(), 0.0 as f64), 0.0);
            assert_eq!(ec.plainParamToNormalized(numeric_hash(), 1.0 as f64), 1.0);
            assert!((ec.plainParamToNormalized(numeric_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
        }

        // Test enum parameter normalized -> plain
        unsafe {
            assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.0), 0.0);
            assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.8), 0.8);
            assert_eq!(ec.normalizedParamToPlain(enum_hash(), 1.0), 1.0);
            assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.5), 0.5);
        }

        // Test enum parameter plain -> normalized
        unsafe {
            assert_eq!(ec.plainParamToNormalized(enum_hash(), 0.0), 0.0);
            assert_eq!(ec.plainParamToNormalized(enum_hash(), 1.0), 1.0);
            assert!((ec.plainParamToNormalized(enum_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
        }

        // Test switch parameter normalized -> plain
        unsafe {
            assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.0), 0.0);
            assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.8), 0.8);
            assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.5), 0.5);
        }

        // Test switch parameter plain -> normalized
        unsafe {
            assert_eq!(ec.plainParamToNormalized(switch_hash(), 0.0), 0.0);
            assert_eq!(ec.plainParamToNormalized(switch_hash(), 1.0), 1.0);
        }
    }

    #[test]
    fn value_to_string_basics() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            )
        }
        let mut string = [0; 128];

        // Test numeric parameter
        unsafe {
            assert_eq!(
                ec.getParamStringByValue(
                    numeric_hash(),
                    f64::from((5.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)),
                    string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
                ),
                vst3::Steinberg::kResultOk
            );
        }
        assert_eq!(
            from_utf16_buffer(&string),
            Some(format!("{:.2}", 5.0).to_string())
        );

        // Test enum parameter
        unsafe {
            assert_eq!(
                ec.getParamStringByValue(
                    enum_hash(),
                    0.5,
                    string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
                ),
                vst3::Steinberg::kResultOk
            );
        }
        assert_eq!(from_utf16_buffer(&string), Some("B".to_string()));

        // Test switch parameter
        unsafe {
            assert_eq!(
                ec.getParamStringByValue(
                    switch_hash(),
                    1.0,
                    string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
                ),
                vst3::Steinberg::kResultOk
            );
        }

        assert_eq!(from_utf16_buffer(&string), Some("On".to_string()));
    }

    #[test]
    fn defends_against_value_to_string_without_initialize() {
        let ec = dummy_edit_controller();
        let mut string = [0; 128];
        assert_ne!(
            unsafe {
                ec.getParamStringByValue(
                    0,
                    0.0,
                    string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
                )
            },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn string_to_value_basics() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            )
        }
        let mut string = [0; 128];

        // Test numeric
        to_utf16("5", &mut string);

        let mut value = 0.0;
        unsafe {
            assert_eq!(
                ec.getParamValueByString(numeric_hash(), string.as_mut_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
            assert!(
                (value - f64::from((5.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC))).abs()
                    < NUMERIC_EPSILON
            );
        }

        // Test enum
        to_utf16("B", &mut string);

        unsafe {
            assert_eq!(
                ec.getParamValueByString(enum_hash(), string.as_mut_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
            assert!((value - 0.5).abs() < NUMERIC_EPSILON);
        }

        // Test switch
        to_utf16("On", &mut string);

        unsafe {
            assert_eq!(
                ec.getParamValueByString(switch_hash(), string.as_mut_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(value, 1.0);
        }
    }

    #[test]
    fn defends_against_get_param_normalized_called_too_early() {
        let ec = dummy_edit_controller();
        assert_eq!(unsafe { ec.getParamNormalized(numeric_hash()) }, 0.0);
    }

    #[test]
    fn defends_against_get_param_normalized_invalid_parameter() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(ec.initialize(host.cast().unwrap().as_ptr()), 0);
        }
        assert_eq!(unsafe { ec.getParamNormalized(400) }, 0.0);
    }

    #[test]
    fn get_param_normalized_starts_default() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            // Test numeric
            let param_hash = numeric_hash();
            assert!(
                (ec.getParamNormalized(param_hash)
                    - f64::from((DEFAULT_NUMERIC - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)))
                .abs()
                    < NUMERIC_EPSILON
            );
        }
    }

    #[test]
    fn defends_against_set_param_normalized_called_too_early() {
        let ec = dummy_edit_controller();
        assert_ne!(
            unsafe { ec.setParamNormalized(numeric_hash(), 0.5) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_set_param_normalized_called_out_of_range() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();

        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }

        assert_ne!(
            unsafe { ec.setParamNormalized(numeric_hash(), -0.5) },
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            unsafe { ec.setParamNormalized(numeric_hash(), 1.5) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_set_param_normalized_called_on_bad_parameter() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();

        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }

        assert_ne!(
            unsafe { ec.setParamNormalized(400, 0.5) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn set_param_normalized_can_change_value() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();

        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }

        unsafe {
            assert_eq!(
                ec.setParamNormalized(numeric_hash(), 0.5),
                vst3::Steinberg::kResultOk
            );
            assert!((ec.getParamNormalized(numeric_hash()) - 0.5).abs() < NUMERIC_EPSILON);
        }
    }

    #[test]
    fn set_param_normalized_switch_and_enum() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();

        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }

        unsafe {
            assert_eq!(
                ec.setParamNormalized(enum_hash(), 0.6),
                vst3::Steinberg::kResultOk
            );
            assert!((ec.getParamNormalized(enum_hash()) - 0.5).abs() < NUMERIC_EPSILON);
        }
        unsafe {
            assert_eq!(
                ec.setParamNormalized(switch_hash(), 0.6),
                vst3::Steinberg::kResultOk
            );
            assert!((ec.getParamNormalized(switch_hash()) - 1.0).abs() < NUMERIC_EPSILON);
        }
    }

    #[test]
    fn defends_against_calling_set_component_state_too_early() {
        let ec = dummy_edit_controller();
        let stream = ComWrapper::new(Stream::new([]));
        assert_ne!(
            unsafe {
                ec.setComponentState(
                    stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr(),
                )
            },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_calling_set_component_state_null() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default())
            .to_com_ptr::<IHostApplication>()
            .unwrap();

        unsafe {
            assert_eq!(
                ec.initialize(host.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
        assert_ne!(
            unsafe { ec.setComponentState(std::ptr::null_mut()) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn set_component_state_basics() {
        let proc = dummy_processor();
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            setup_proc(&proc, &host);

            assert_eq!(
                proc.process(
                    &mut mock_no_audio_process_data(
                        vec![],
                        vec![ParameterValueQueueImpl {
                            param_id: ENUM_ID.to_string(),
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 1.0,
                            }],
                        },],
                    )
                    .process_data
                ),
                vst3::Steinberg::kResultOk
            );
            let stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                proc.getState(
                    stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.getParamNormalized(enum_hash()),
                1.0,
                "Parameter value should be restored"
            );
        }
    }

    #[test]
    fn set_component_incompatible_error() {
        let proc = processor::create_synth(
            |_: &HostInfo| -> IncompatibleComponent { Default::default() },
            [5; 16],
        );
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            setup_proc(&proc, &host);

            let stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                proc.getState(
                    stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );

            assert_ne!(
                ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn set_component_newer_loads_defaults() {
        let proc = processor::create_synth(
            |_: &HostInfo| -> NewerComponent { Default::default() },
            [5; 16],
        );
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            setup_proc(&proc, &host);

            assert_eq!(
                ec.setParamNormalized(numeric_hash(), 1.0),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                proc.process(
                    &mut mock_no_audio_process_data(
                        vec![],
                        vec![ParameterValueQueueImpl {
                            param_id: NUMERIC_ID.to_string(),
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 0.5,
                            }],
                        },],
                    )
                    .process_data
                ),
                vst3::Steinberg::kResultOk
            );

            let stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                proc.getState(
                    stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert!(
                (ec.getParamNormalized(numeric_hash())
                    - ((DEFAULT_NUMERIC - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64)
                    .abs()
                    < NUMERIC_EPSILON
            );
        }
    }

    #[derive(PartialEq)]
    enum ComponentHandlerCalls {
        BeginEdit(u32),
        PerformEdit(u32, f64),
        EndEdit(u32),
        RestartComponent(i32),
    }

    #[derive(Default)]
    struct ComponentHandlerSpy {
        calls: RefCell<Vec<ComponentHandlerCalls>>,
    }

    impl IComponentHandlerTrait for ComponentHandlerSpy {
        unsafe fn beginEdit(&self, id: vst3::Steinberg::Vst::ParamID) -> vst3::Steinberg::tresult {
            self.calls
                .borrow_mut()
                .push(ComponentHandlerCalls::BeginEdit(id));
            vst3::Steinberg::kResultOk
        }

        unsafe fn performEdit(
            &self,
            id: vst3::Steinberg::Vst::ParamID,
            value_normalized: vst3::Steinberg::Vst::ParamValue,
        ) -> vst3::Steinberg::tresult {
            self.calls
                .borrow_mut()
                .push(ComponentHandlerCalls::PerformEdit(id, value_normalized));
            vst3::Steinberg::kResultOk
        }

        unsafe fn endEdit(&self, id: vst3::Steinberg::Vst::ParamID) -> vst3::Steinberg::tresult {
            self.calls
                .borrow_mut()
                .push(ComponentHandlerCalls::EndEdit(id));
            vst3::Steinberg::kResultOk
        }

        unsafe fn restartComponent(
            &self,
            flags: vst3::Steinberg::int32,
        ) -> vst3::Steinberg::tresult {
            self.calls
                .borrow_mut()
                .push(ComponentHandlerCalls::RestartComponent(flags));
            vst3::Steinberg::kResultOk
        }
    }

    impl Class for ComponentHandlerSpy {
        type Interfaces = (IComponentHandler,);
    }

    #[test]
    fn defends_against_set_component_handler_called_too_soon() {
        let ec = dummy_edit_controller();
        let handler = ComWrapper::new(ComponentHandlerSpy::default());
        assert_ne!(
            unsafe { ec.setComponentHandler(handler.as_com_ref().unwrap().as_ptr()) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_set_component_handler_null() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        assert_eq!(
            unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) },
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            unsafe { ec.setComponentHandler(std::ptr::null_mut()) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn set_component_handler_succeeds() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        assert_eq!(
            unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) },
            vst3::Steinberg::kResultOk
        );
        let handler = ComWrapper::new(ComponentHandlerSpy::default());
        assert_eq!(
            unsafe { ec.setComponentHandler(handler.as_com_ref().unwrap().as_ptr()) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn defends_against_create_view_called_with_weird_name() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let weird_name = std::ffi::CString::new("weird name").unwrap();
            assert!(vst3::ComPtr::from_raw(ec.createView(weird_name.as_ptr())).is_none());
        }
    }

    #[test]
    #[should_panic]
    fn panic_on_duplicate_ids() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| parameters::to_infos(&DUPLICATE_PARAMETERS)),
            "test_prefs".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Synth(),
        );
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            ec.initialize(host.as_com_ref().unwrap().as_ptr());
        }
    }

    #[test]
    fn get_parameters_from_store() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }
        let store = ec.get_store();
        assert!(store.is_some());
        let store = store.unwrap();
        assert_eq!(
            store.get(NUMERIC_ID),
            Some(parameters::Value::Numeric(DEFAULT_NUMERIC as f32))
        );
        assert_eq!(
            store.get(ENUM_ID),
            Some(parameters::Value::Enum("A".to_string()))
        );
        assert_eq!(store.get(SWITCH_ID), Some(parameters::Value::Switch(false)));
        assert_eq!(store.get("Invalid"), None);
    }

    #[derive(Default)]
    struct SpyListener {
        param_changes: RefCell<Vec<(String, parameters::Value)>>,
        ui_state_changes: RefCell<Vec<Vec<u8>>>,
    }

    impl store::Listener for SpyListener {
        fn parameter_changed(&self, id: &str, value: &parameters::Value) {
            self.param_changes
                .borrow_mut()
                .push((id.to_string(), value.clone()));
        }

        fn ui_state_changed(&self, state: &[u8]) {
            self.ui_state_changes.borrow_mut().push(state.to_vec());
        }
    }

    #[test]
    fn changing_parameters_in_store() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let store = ec.get_store();
            assert!(store.is_some());
            let mut store = store.unwrap();
            let listener = rc::Rc::new(SpyListener::default());
            store.set_listener(rc::Rc::downgrade(
                &(listener.clone() as rc::Rc<dyn store::Listener>),
            ));
            assert_eq!(
                ec.setParamNormalized(enum_hash(), 1.0),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.get(ENUM_ID),
                Some(parameters::Value::Enum("C".to_string()))
            );
            assert_eq!(
                listener.param_changes.borrow().as_slice(),
                &[(
                    ENUM_ID.to_string(),
                    parameters::Value::Enum("C".to_string())
                )]
            );
        }
    }

    #[test]
    fn set_component_state_sets_params() {
        let proc = dummy_processor();
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let store = ec.get_store();
            assert!(store.is_some());
            let mut store = store.unwrap();
            let listener = rc::Rc::new(SpyListener::default());
            store.set_listener(rc::Rc::downgrade(
                &(listener.clone() as rc::Rc<dyn store::Listener>),
            ));

            setup_proc(&proc, &host);

            assert_eq!(
                proc.process(
                    &mut mock_no_audio_process_data(
                        vec![],
                        vec![ParameterValueQueueImpl {
                            param_id: ENUM_ID.to_string(),
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 1.0,
                            }],
                        },],
                    )
                    .process_data
                ),
                vst3::Steinberg::kResultOk
            );
            let stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                proc.getState(
                    stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.get(ENUM_ID),
                Some(parameters::Value::Enum("C".to_string()))
            );
            assert!(listener.param_changes.borrow().as_slice().contains(&(
                ENUM_ID.to_string(),
                parameters::Value::Enum("C".to_string())
            )),);
        }
    }

    #[test]
    fn set_from_store_forwarded_to_component_handler() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.set(ENUM_ID, parameters::Value::Enum("C".to_string())),
                Ok(())
            );
            assert!(spy.calls.borrow().iter().any(|call| call
                == &ComponentHandlerCalls::PerformEdit(
                    parameters::hash_id(ENUM_ID).internal_hash(),
                    1.0
                )));
        }
    }

    #[test]
    fn invalid_id_fails_set() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.set("Not a real ID", parameters::Value::Enum("C".to_string())),
                Err(store::SetError::NotFound)
            );
        }
    }

    #[test]
    fn no_component_handler_fails_set() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                store.set(ENUM_ID, parameters::Value::Enum("C".to_string())),
                Err(store::SetError::InternalError)
            );
        }
    }

    #[test]
    fn invalid_enum_fails_set() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.set(
                    ENUM_ID,
                    parameters::Value::Enum("Not a real value".to_string())
                ),
                Err(store::SetError::InvalidValue)
            );
        }
    }

    #[test]
    fn out_of_range_numeric_fails_set() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.set(NUMERIC_ID, parameters::Value::Numeric(MAX_NUMERIC + 1.0)),
                Err(store::SetError::InvalidValue)
            );
        }
    }

    #[test]
    fn wrong_type_fails_set() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                store.set(NUMERIC_ID, parameters::Value::Switch(false)),
                Err(store::SetError::WrongType)
            );
        }
    }

    #[test]
    fn set_grabbed_from_store_forwarded_to_component_handler() {
        let ec = dummy_edit_controller();

        let host = ComWrapper::new(dummy_host::Host::default());
        let spy = ComWrapper::new(ComponentHandlerSpy::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let mut store = ec.get_store().unwrap();
            assert_eq!(
                ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(store.set_grabbed(ENUM_ID, true), Ok(()));
            assert_eq!(store.set_grabbed(ENUM_ID, false), Ok(()));
            assert!(spy.calls.borrow().iter().any(|call| call
                == &ComponentHandlerCalls::BeginEdit(
                    parameters::hash_id(ENUM_ID).internal_hash()
                )));
            assert!(spy.calls.borrow().iter().any(|call| call
                == &ComponentHandlerCalls::EndEdit(parameters::hash_id(ENUM_ID).internal_hash())));
        }
    }

    #[test]
    fn get_info_basics() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            let store = ec.get_store().unwrap();
            assert_eq!(store.get_info(ENUM_ID), Some((&PARAMETERS[1]).into()));
        }
    }

    #[test]
    fn defends_against_get_info_bad_id() {
        let ec = dummy_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
            let store = ec.get_store().unwrap();
            assert_eq!(store.get_info("Not a real ID"), None);
        }
    }

    #[test]
    #[should_panic]
    fn defends_against_missing_bypass_param() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| parameters::to_infos(&PARAMETERS)),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: "missing",
            },
        );

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
    }

    #[test]
    #[should_panic]
    fn defends_against_non_switch_bypass_param() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| parameters::to_infos(&PARAMETERS)),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: NUMERIC_ID,
            },
        );

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
    }

    #[test]
    #[should_panic]
    fn defends_against_default_on_bypass_param() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| {
                parameters::to_infos(&[InfoRef {
                    title: "Test Switch",
                    short_title: "Switch",
                    unique_id: SWITCH_ID,
                    flags: Flags { automatable: true },
                    type_specific: TypeSpecificInfoRef::Switch { default: true },
                }])
            }),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: SWITCH_ID,
            },
        );

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
    }

    #[test]
    fn bypass_parameter_exposed() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| {
                parameters::to_infos(&[InfoRef {
                    title: "Test Switch",
                    short_title: "Switch",
                    unique_id: SWITCH_ID,
                    flags: Flags { automatable: true },
                    type_specific: TypeSpecificInfoRef::Switch { default: false },
                }])
            }),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: SWITCH_ID,
            },
        );

        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
        }

        let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
            id: 0,
            title: [0; 128],
            shortTitle: [0; 128],
            units: [0; 128],
            stepCount: 0,
            defaultNormalizedValue: 0f64,
            unitId: 0,
            flags: 0,
        };

        unsafe {
            assert_eq!(
                ec.getParameterInfo(0, &mut param_info),
                vst3::Steinberg::kResultOk
            );
        }

        assert!(
            param_info.flags
                & vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass as i32
                != 0
        );
    }

    fn dummy_synth_edit_controller() -> impl IPluginBaseTrait
    + IEditControllerTrait
    + IMidiMappingTrait
    + INoteExpressionControllerTrait
    + INoteExpressionPhysicalUIMappingTrait
    + GetStore {
        super::create_internal(
            create_parameter_model(|_: &HostInfo| parameters::to_infos(&[])),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Synth(),
        )
    }

    #[test]
    fn synth_control_parameters_exposed() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);

            let check_assignment = |vst_id: crate::DefaultEnumType, param_id| {
                let mut id: vst3::Steinberg::Vst::ParamID = 0;
                assert_eq!(
                    ec.getMidiControllerAssignment(
                        0,
                        0,
                        vst_id.try_into().unwrap(),
                        &mut id as *mut _
                    ),
                    vst3::Steinberg::kResultTrue
                );

                assert_eq!(hash_id(param_id).internal_hash(), id);
            };
            check_assignment(
                vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend,
                PITCH_BEND_PARAMETER,
            );
            check_assignment(
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel,
                MOD_WHEEL_PARAMETER,
            );
            check_assignment(
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlExpression,
                EXPRESSION_PEDAL_PARAMETER,
            );
            check_assignment(
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlSustainOnOff,
                SUSTAIN_PARAMETER,
            );
            check_assignment(
                vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch,
                AFTERTOUCH_PARAMETER,
            );
            {
                // due to mpe quirks we should have _some_ mapping to aftertouch
                let mut id: vst3::Steinberg::Vst::ParamID = 0;
                assert_eq!(
                    ec.getMidiControllerAssignment(
                        0,
                        1,
                        vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch as i16,
                        &mut id
                    ),
                    vst3::Steinberg::kResultTrue
                );
            }

            let store = ec.get_store().unwrap();
            assert_eq!(
                store.get(PITCH_BEND_PARAMETER),
                Some(parameters::Value::Numeric(0.0))
            );
        }
    }

    #[test]
    fn midi_mapping_bad_context_false() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
            let mut id: vst3::Steinberg::Vst::ParamID = 0;
            assert_eq!(
                ec.getMidiControllerAssignment(
                    1,
                    0,
                    vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel
                        .try_into()
                        .unwrap(),
                    &mut id as *mut _
                ),
                vst3::Steinberg::kResultFalse
            );
            assert_eq!(
                ec.getMidiControllerAssignment(
                    0,
                    0,
                    // This test will have to change if we ever support kCtrlGPC8...
                    vst3::Steinberg::Vst::ControllerNumbers_::kCtrlGPC8
                        .try_into()
                        .unwrap(),
                    &mut id as *mut _
                ),
                vst3::Steinberg::kResultFalse
            );
        }
    }

    #[test]
    fn get_note_expression_count() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            assert_eq!(ec.getNoteExpressionCount(0, 0), 0);
            assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
            assert_eq!(ec.getNoteExpressionCount(0, 0), 3);
            assert_eq!(ec.getNoteExpressionCount(1, 0), 0);
            assert_eq!(ec.getNoteExpressionCount(0, 1), 0);
            assert_eq!(ec.getNoteExpressionCount(1, 1), 0);
        }
    }

    #[test]
    fn get_note_expression_info() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            let mut info = vst3::Steinberg::Vst::NoteExpressionTypeInfo {
                typeId: 0,
                title: [0; 128],
                shortTitle: [0; 128],
                units: [0; 128],
                unitId: 0,
                valueDesc: vst3::Steinberg::Vst::NoteExpressionValueDescription {
                    defaultValue: 0.0,
                    minimum: 0.0,
                    maximum: 1.0,
                    stepCount: 0,
                },
                associatedParameterId: 0,
                flags: 0,
            };

            assert_ne!(
                ec.getNoteExpressionInfo(0, 0, 0, &mut info),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.getNoteExpressionInfo(0, 0, 0, &mut info),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                info.typeId,
                vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID
            );
            assert_eq!(from_utf16_buffer(&info.title).unwrap(), "Pitch Bend");
            assert_eq!(from_utf16_buffer(&info.shortTitle).unwrap(), "Pitch");
            assert_eq!(from_utf16_buffer(&info.units).unwrap(), "semitones");
            assert_eq!(
                info.flags,
                vst3::Steinberg::Vst::NoteExpressionTypeInfo_::NoteExpressionTypeFlags_::kIsBipolar
                    as i32
            );

            assert_eq!(
                ec.getNoteExpressionInfo(0, 0, 1, &mut info),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(info.typeId, processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID);
            assert_eq!(from_utf16_buffer(&info.title).unwrap(), "Timbre");
            assert_eq!(from_utf16_buffer(&info.shortTitle).unwrap(), "Timbre");
            assert_eq!(from_utf16_buffer(&info.units).unwrap(), "");
            assert_eq!(info.unitId, 0);
            assert_eq!(info.valueDesc.defaultValue, 0.);
            assert_eq!(info.valueDesc.minimum, 0.);
            assert_eq!(info.valueDesc.maximum, 1.0);
            assert_eq!(info.valueDesc.stepCount, 0);
            assert_eq!(info.flags, 0);

            assert_eq!(
                ec.getNoteExpressionInfo(0, 0, 2, &mut info),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(info.typeId, processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID);
            assert_eq!(from_utf16_buffer(&info.title).unwrap(), "Aftertouch");
            assert_eq!(from_utf16_buffer(&info.shortTitle).unwrap(), "Aftertouch");
            assert_eq!(from_utf16_buffer(&info.units).unwrap(), "");
            assert_eq!(info.unitId, 0);
            assert_eq!(info.valueDesc.defaultValue, 0.);
            assert_eq!(info.valueDesc.minimum, 0.);
            assert_eq!(info.valueDesc.maximum, 1.0);
            assert_eq!(info.valueDesc.stepCount, 0);
            assert_eq!(info.flags, 0);

            assert_ne!(
                ec.getNoteExpressionInfo(1, 0, 0, &mut info),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionInfo(0, 1, 0, &mut info),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionInfo(0, 0, 3, &mut info),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn get_note_expression_string_by_value() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            let mut string = [0u16; 128];
            assert_ne!(
                ec.getNoteExpressionStringByValue(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.getNoteExpressionStringByValue(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(from_utf16_buffer(&string).unwrap(), "0.00");
            assert_eq!(
                ec.getNoteExpressionStringByValue(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    1.0,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(from_utf16_buffer(&string).unwrap(), "120.00");

            assert_eq!(
                ec.getNoteExpressionStringByValue(
                    0,
                    0,
                    crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(from_utf16_buffer(&string).unwrap(), "0.50");

            assert_eq!(
                ec.getNoteExpressionStringByValue(
                    0,
                    0,
                    crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(from_utf16_buffer(&string).unwrap(), "0.50");

            assert_ne!(
                ec.getNoteExpressionStringByValue(
                    1,
                    0,
                    0,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionStringByValue(
                    0,
                    1,
                    0,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionStringByValue(
                    1,
                    1,
                    0,
                    0.5,
                    string.as_mut_ptr().cast::<[u16; 128]>()
                ),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn get_note_expression_value_by_string() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            let mut value = 0.0;
            let mut string = [0u16; 128];
            to_utf16("60", &mut string);
            assert_ne!(
                ec.getNoteExpressionValueByString(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    string.as_ptr(),
                    &mut value
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.getNoteExpressionValueByString(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    string.as_ptr(),
                    &mut value
                ),
                vst3::Steinberg::kResultOk
            );
            assert_approx_eq!(value, 0.75);

            to_utf16("120", &mut string);
            assert_eq!(
                ec.getNoteExpressionValueByString(
                    0,
                    0,
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID,
                    string.as_ptr(),
                    &mut value
                ),
                vst3::Steinberg::kResultOk
            );
            assert_approx_eq!(value, 1.0);

            to_utf16("0.5", &mut string);
            assert_eq!(
                ec.getNoteExpressionValueByString(
                    0,
                    0,
                    crate::processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID,
                    string.as_ptr(),
                    &mut value
                ),
                vst3::Steinberg::kResultOk
            );
            assert_approx_eq!(value, 0.5);

            assert_eq!(
                ec.getNoteExpressionValueByString(
                    0,
                    0,
                    crate::processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID,
                    string.as_ptr(),
                    &mut value
                ),
                vst3::Steinberg::kResultOk
            );
            assert_approx_eq!(value, 0.5);

            assert_ne!(
                ec.getNoteExpressionValueByString(1, 0, 0, string.as_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionValueByString(0, 1, 0, string.as_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getNoteExpressionValueByString(1, 1, 0, string.as_ptr(), &mut value),
                vst3::Steinberg::kResultOk
            );
        }
    }

    #[test]
    fn get_physical_ui_mapping() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            let mut map: [PhysicalUIMap; 3] = [PhysicalUIMap {
                physicalUITypeID: 0,
                noteExpressionTypeID: 0,
            }; 3];
            map[0].physicalUITypeID =
                enum_to_u32(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIYMovement).unwrap();
            map[1].physicalUITypeID =
                enum_to_u32(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIXMovement).unwrap();
            map[2].physicalUITypeID =
                enum_to_u32(vst3::Steinberg::Vst::PhysicalUITypeIDs_::kPUIPressure).unwrap();
            let mut physical_ui_mapping = vst3::Steinberg::Vst::PhysicalUIMapList {
                count: 3,
                map: map.as_mut_ptr(),
            };

            assert_ne!(
                ec.getPhysicalUIMapping(0, 0, &mut physical_ui_mapping),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            assert_ne!(
                ec.getPhysicalUIMapping(1, 0, &mut physical_ui_mapping),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getPhysicalUIMapping(0, 1, &mut physical_ui_mapping),
                vst3::Steinberg::kResultOk
            );
            assert_ne!(
                ec.getPhysicalUIMapping(1, 1, &mut physical_ui_mapping),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                ec.getPhysicalUIMapping(0, 0, &mut physical_ui_mapping),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                map[0].noteExpressionTypeID,
                processor::NOTE_EXPRESSION_TIMBRE_TYPE_ID
            );
            assert_eq!(
                map[1].noteExpressionTypeID,
                vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID
            );
            assert_eq!(
                map[2].noteExpressionTypeID,
                processor::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID
            );
        }
    }

    #[test]
    fn get_midi_controller_assignment_effect() {
        let ec = super::create_internal(
            create_parameter_model(|_: &HostInfo| {
                parameters::to_infos(&[InfoRef {
                    title: "Test Switch",
                    short_title: "Switch",
                    unique_id: SWITCH_ID,
                    flags: Flags { automatable: true },
                    type_specific: TypeSpecificInfoRef::Switch { default: false },
                }])
            }),
            "dummy_domain".to_string(),
            conformal_ui::Size {
                width: 0,
                height: 0,
            },
            super::Kind::Effect {
                bypass_id: SWITCH_ID,
            },
        );
        let host = ComWrapper::new(dummy_host::Host::default());
        unsafe {
            let mut id: vst3::Steinberg::Vst::ParamID = 0;
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                ec.getMidiControllerAssignment(
                    0,
                    0,
                    vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend
                        .try_into()
                        .unwrap(),
                    &mut id
                ),
                vst3::Steinberg::kResultFalse
            );
        }
    }

    #[test]
    fn ui_state_is_saved() {
        let ec = dummy_synth_edit_controller();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            assert_eq!(
                ec.initialize(host.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );

            let mut store = ec.get_store().unwrap();
            let listener = rc::Rc::new(SpyListener::default());
            store.set_listener(rc::Rc::downgrade(
                &(listener.clone() as rc::Rc<dyn store::Listener>),
            ));

            let initial_stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                ec.getState(
                    initial_stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );

            store.set_ui_state(&[1]);
            assert_eq!(listener.ui_state_changes.borrow().as_slice(), &[vec![1]]);

            assert_eq!(store.get_ui_state(), vec![1]);

            let modified_stream = ComWrapper::new(Stream::new([]));
            assert_eq!(
                ec.getState(
                    modified_stream
                        .as_com_ref::<vst3::Steinberg::IBStream>()
                        .unwrap()
                        .as_ptr()
                ),
                vst3::Steinberg::kResultOk
            );

            assert_eq!(
                initial_stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                ec.setState(initial_stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                listener.ui_state_changes.borrow().as_slice(),
                &[vec![1], vec![]]
            );

            assert_eq!(
                modified_stream.seek(
                    0,
                    vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                    std::ptr::null_mut(),
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                ec.setState(modified_stream.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                listener.ui_state_changes.borrow().as_slice(),
                &[vec![1], vec![], vec![1]]
            );
        }
    }
}
