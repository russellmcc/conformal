use std::{
    ops::RangeInclusive,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64},
        mpsc,
    },
};

use vst3::{
    ComRef,
    Steinberg::Vst::{
        IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait,
    },
};

use crate::{
    mpe,
    parameters::{convert_enum, convert_numeric, convert_switch},
};
use conformal_component::{
    parameters as cp,
    synth::{
        NumericGlobalExpression, SwitchGlobalExpression, SynthParamBufferStates, SynthParamStates,
    },
};
use conformal_core::parameters as cc;

use conformal_component::parameters::{
    BufferState, BufferStates, EnumBufferState, NumericBufferState, PiecewiseLinearCurve,
    PiecewiseLinearCurvePoint, States as ParameterStates, SwitchBufferState, TimedEnumValues,
    TimedSwitchValues, TimedValue, TypeSpecificInfoRef,
};

struct NumericParamMetadatum {
    valid_range: RangeInclusive<f32>,
    default: f32,
}

struct EnumParamMetadatum {
    values: Vec<String>,
    default: u32,
}

struct SwitchParamMetadatum {
    default: bool,
}

enum Metadatum {
    Numeric { datum: NumericParamMetadatum },
    Enum { datum: EnumParamMetadatum },
    Switch { datum: SwitchParamMetadatum },
}

enum AtomicValue {
    Numeric(AtomicU32),
    Enum(AtomicU32),
    Switch(AtomicBool),
}

struct SnapshotMessage {
    snapshot: Arc<cc::Snapshot>,
    generation: u64,
}

/// This represents the "main" side of the store (see `create_stores`).
pub struct MainStore {
    /// This lets us convert from hashes back to the original IDs
    ///
    /// Note that this only includes parameters exposed by the component,
    /// that is, controller parameters or parameters added to work around host quirks
    /// are not included.
    unhash_for_snapshot: cp::IdHashMap<String>,

    data: Arc<cp::IdHashMap<AtomicValue>>,
    cached_write_snapshot: Option<Arc<cc::Snapshot>>,
    write_generation: u64,

    /// This is written by `ProcessingContextParameterStore` and read here.
    read_generation: Arc<AtomicU64>,

    metadata: Arc<Metadata>,

    garbage_rx: mpsc::Receiver<Arc<cc::Snapshot>>,
    snapshot_tx: mpsc::SyncSender<SnapshotMessage>,
}

/// This represents the "core" of the processing side of the store.
/// This is separated from the `scratch` for convenience.
struct ProcessingStoreCore {
    data: Arc<cp::IdHashMap<AtomicValue>>,

    /// This is written here.
    read_generation: Arc<AtomicU64>,

    metadata: Arc<Metadata>,

    garbage_tx: mpsc::SyncSender<Arc<cc::Snapshot>>,
    snapshot_rx: mpsc::Receiver<SnapshotMessage>,
}

/// This represents the processing side of the store (see `create_stores`).
pub struct ProcessingStore {
    core: ProcessingStoreCore,
    /// This is a pre-allocated scratch space used to implement
    /// our API without allocating in the processing context.
    scratch: Scratch,
}

/// Get a snapshot of the current value of an atomic value.
fn atomic_get(param: &AtomicValue) -> cp::InternalValue {
    // Note - it's okay to use Relaxed here, since we depend on the host
    // serializing process calls (and we only ever modify these atomics
    // from process calls). Unfortunately vst3 doesn't really get into requirements
    // on hosts when they move processors between threads, but I think it's
    // more than reasonable to expect a happens-before relationship in this case.
    match param {
        AtomicValue::Numeric(n) => {
            cp::InternalValue::Numeric(f32::from_bits(n.load(std::sync::atomic::Ordering::Relaxed)))
        }
        AtomicValue::Enum(n) => {
            cp::InternalValue::Enum(n.load(std::sync::atomic::Ordering::Relaxed))
        }
        AtomicValue::Switch(n) => {
            cp::InternalValue::Switch(n.load(std::sync::atomic::Ordering::Relaxed))
        }
    }
}

impl cp::States for &ProcessingStoreCore {
    fn get_by_hash(&self, id: cp::IdHash) -> Option<cp::InternalValue> {
        self.data.get(&id).map(atomic_get)
    }
}

#[derive(Clone)]
pub struct CoreWithMpe<'a> {
    core: &'a ProcessingStoreCore,
    mpe: &'a mpe::State,
}

impl cp::States for CoreWithMpe<'_> {
    fn get_by_hash(&self, id: cp::IdHash) -> Option<cp::InternalValue> {
        self.core.get_by_hash(id)
    }
}

impl SynthParamStates for CoreWithMpe<'_> {
    fn get_numeric_global_expression(&self, expression: NumericGlobalExpression) -> f32 {
        self.mpe
            .get_numeric_global_expression(expression, &self.core)
    }
    fn get_switch_global_expression(&self, expression: SwitchGlobalExpression) -> bool {
        self.mpe
            .get_switch_global_expression(expression, &self.core)
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: conformal_component::synth::NumericPerNoteExpression,
        note_id: conformal_component::events::NoteID,
    ) -> f32 {
        self.mpe
            .get_numeric_expression_for_note(expression, note_id, &self.core)
    }
}

impl cp::States for &ProcessingStore {
    fn get_by_hash(&self, id: cp::IdHash) -> Option<cp::InternalValue> {
        (&self.core).get_by_hash(id)
    }
}

/// This controls how many load snapshots we allow in our queue.
///
/// If our garbage queue gets full, we will deallocate on the processing thread.
/// If our send queue gets full (which could happen if the processing thread
/// isn't being called by the host), we will drop loads, which is pretty bad.
///
/// We may want to consider doing something like "stealing" the processor
/// and giving it a time slice if this happens.
static CHANNEL_BOUNDS: usize = 50;

fn make_unhash<'a, S: AsRef<str> + 'a, Iter: IntoIterator<Item = cp::InfoRef<'a, S>>>(
    iter: Iter,
) -> cp::IdHashMap<String> {
    let mut unhash = cp::IdHashMap::default();
    for info in iter {
        match unhash.entry(cp::hash_id(info.unique_id)) {
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!(
                    "Duplicate parameter ID hash! This could be caused by duplicate parameter IDs or a hash collision."
                );
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(info.unique_id.to_owned());
            }
        }
    }
    unhash
}

/// This generates two "stores" for the parameters that allow us to implement
/// important parameter-related operations of vst3.
///
/// To support the thread model of the API, we return two different stores,
/// with different capabilities, the `MainStore` and the `ProcessingStore`.
/// It _is_ safe to interact with these concurrently!
///
/// The `ProcessingStore` is designed to support the operations needed by
/// the vst3 `process` call, while the `MainStore` is designed to support
/// the operations needed by the vst3 `setState` and `getState` calls.
/// These calls can happen concurrently, which is why we return two different objects.
pub fn create_stores<
    'a,
    S: AsRef<str> + 'a,
    Iter: IntoIterator<Item = cp::InfoRef<'a, S>> + Clone,
>(
    iter: Iter,
) -> (MainStore, ProcessingStore) {
    let data = Arc::<cp::IdHashMap<AtomicValue>>::new(
        iter.clone()
            .into_iter()
            .map(|info| {
                let value = match info.type_specific {
                    TypeSpecificInfoRef::Enum { default, .. } => {
                        AtomicValue::Enum(AtomicU32::new(default))
                    }
                    TypeSpecificInfoRef::Numeric { default, .. } => {
                        AtomicValue::Numeric(AtomicU32::new(default.to_bits()))
                    }
                    TypeSpecificInfoRef::Switch { default } => {
                        AtomicValue::Switch(AtomicBool::new(default))
                    }
                };
                (cp::hash_id(info.unique_id), value)
            })
            .collect(),
    );

    let unhash_for_snapshot =
        make_unhash(iter.clone().into_iter().filter(|info| {
            crate::parameters::should_include_parameter_in_snapshot(info.unique_id)
        }));
    let metadata = Arc::new(Metadata::new(iter));
    let scratch = Scratch::new(&metadata);
    let (garbage_tx, garbage_rx) = mpsc::sync_channel(CHANNEL_BOUNDS);
    let (snapshot_tx, snapshot_rx) = mpsc::sync_channel(CHANNEL_BOUNDS);
    let read_generation = Arc::new(AtomicU64::new(0));
    (
        MainStore {
            unhash_for_snapshot,

            data: data.clone(),
            cached_write_snapshot: None,
            write_generation: 0,
            read_generation: read_generation.clone(),
            metadata: metadata.clone(),

            garbage_rx,
            snapshot_tx,
        },
        ProcessingStore {
            core: ProcessingStoreCore {
                data,
                read_generation,

                metadata,

                garbage_tx,
                snapshot_rx,
            },
            scratch,
        },
    )
}

fn to_internal(unique_id: &str, value: &cp::Value, metadata: &Metadata) -> cp::InternalValue {
    let metadatum = metadata.data.get(&cp::hash_id(unique_id)).unwrap();
    match (value, metadatum) {
        (cp::Value::Numeric(n), Metadatum::Numeric { .. }) => cp::InternalValue::Numeric(*n),
        (cp::Value::Enum(v), Metadatum::Enum { datum }) => cp::InternalValue::Enum(
            datum
                .values
                .iter()
                .position(|s| s == v)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
        (cp::Value::Switch(n), Metadatum::Switch { .. }) => cp::InternalValue::Switch(*n),
        _ => panic!("Internal error for parameter {unique_id}"),
    }
}

fn from_internal(
    unique_id_hash: cp::IdHash,
    value: cp::InternalValue,
    metadata: &Metadata,
) -> cp::Value {
    let metadatum = metadata.data.get(&unique_id_hash).unwrap();
    match (value, metadatum) {
        (cp::InternalValue::Numeric(n), Metadatum::Numeric { .. }) => cp::Value::Numeric(n),
        (cp::InternalValue::Enum(v), Metadatum::Enum { datum }) => {
            cp::Value::Enum(datum.values[v as usize].clone())
        }
        (cp::InternalValue::Switch(n), Metadatum::Switch { .. }) => cp::Value::Switch(n),
        _ => panic!("Internal error"),
    }
}

impl ProcessingStoreCore {
    fn drop_garbage(&self, snapshot: Arc<cc::Snapshot>) {
        // If we failed to send the garbage down the chute, drop it ourselves!
        // This shouldn't happen in practice
        std::mem::drop(self.garbage_tx.try_send(snapshot));
    }

    /// Must be called on every process call!
    fn sync_from_main_thread(&mut self) {
        let most_recent_data = {
            let mut most_recent_data: Option<SnapshotMessage> = None;
            while let Ok(msg) = self.snapshot_rx.try_recv() {
                if let Some(old_data) = most_recent_data.take() {
                    self.drop_garbage(old_data.snapshot);
                }
                most_recent_data = Some(msg);
            }
            most_recent_data
        };

        if let Some(msg) = most_recent_data {
            for (k, v) in &msg.snapshot.as_ref().values {
                self.set(cp::hash_id(k), to_internal(k, v, &self.metadata));
            }

            self.read_generation
                .store(msg.generation, std::sync::atomic::Ordering::Release);

            self.drop_garbage(msg.snapshot);
        }
    }

    fn set(&self, id: cp::IdHash, new_value: cp::InternalValue) -> bool {
        self.data
            .get(&id)
            .is_some_and(|param| match (param, new_value) {
                (AtomicValue::Numeric(n), cp::InternalValue::Numeric(m)) => {
                    n.store(m.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    true
                }
                (AtomicValue::Enum(n), cp::InternalValue::Enum(m)) => {
                    n.store(m, std::sync::atomic::Ordering::Relaxed);
                    true
                }
                (AtomicValue::Switch(n), cp::InternalValue::Switch(m)) => {
                    n.store(m, std::sync::atomic::Ordering::Relaxed);
                    true
                }
                _ => false,
            })
    }
}

impl ProcessingStore {
    /// Must be called on every process call. This synchronizes any data
    /// changes from the main thread (e.g., setting and loading state)
    pub fn sync_from_main_thread(&mut self) {
        self.core.sync_from_main_thread();
    }
}

pub enum SnapshotError {
    /// The cross-thread queue used to apply snapshots was full. This can
    /// happen if the processing context is not having its parameters
    /// flushed.
    QueueTooFull,

    /// The snapshot was corrupted
    SnapshotCorrupted,
}

impl MainStore {
    fn drop_garbage(&self) {
        while self.garbage_rx.try_recv().is_ok() {}
    }

    /// Get a serializable snapshot of the current value of the parameters.
    ///
    /// Note that if concurrent changes are happening on the processing
    /// thread, will get a "torn" snapshot that may not represent the
    /// state of the parameters at any particular point in time.
    pub fn snapshot_with_tearing(&self) -> cc::serialization::Snapshot {
        self.drop_garbage();

        let lookup = |id: &_| match self.metadata.data.get(&cp::hash_id(id))? {
            Metadatum::Numeric { .. } => Some(cc::serialization::WriteInfoRef::Numeric {}),
            Metadatum::Enum { datum } => Some(cc::serialization::WriteInfoRef::Enum {
                values: datum.values.iter().map(String::as_str),
            }),
            Metadatum::Switch { .. } => Some(cc::serialization::WriteInfoRef::Switch {}),
        };

        // We check if the latest write has been ack'ed by the
        // other thread. If not we just return the cached snapshot,
        // so we don't get weird behavior where read-backs return old
        // data until the other thread gets updated.
        if self
            .read_generation
            .load(std::sync::atomic::Ordering::Acquire)
            != self.write_generation
        {
            // Note that we maintain the invariant that the cache is always valid
            // if we have ever written a snapshot.
            return self
                .cached_write_snapshot
                .as_ref()
                .unwrap()
                .as_ref()
                .clone()
                .into_serialize(lookup)
                .unwrap();
        }

        // We need to read from the live store, which will tear!
        cc::Snapshot {
            values: self
                .data
                .iter()
                .filter_map(|(id, value)| {
                    let unhashed = self.unhash_for_snapshot.get(id)?;
                    Some((
                        unhashed.clone(),
                        from_internal(*id, atomic_get(value), &self.metadata),
                    ))
                })
                .collect(),
        }
        .into_serialize(lookup)
        .unwrap()
    }

    fn get_default_snapshot(&self) -> cc::Snapshot {
        cc::Snapshot {
            values: self
                .metadata
                .data
                .iter()
                .filter_map(|(id, metadatum)| {
                    let unhashed = self.unhash_for_snapshot.get(id)?;
                    Some((
                        unhashed.clone(),
                        match metadatum {
                            Metadatum::Numeric { datum } => cp::Value::Numeric(datum.default),
                            Metadatum::Enum { datum } => {
                                cp::Value::Enum(datum.values[datum.default as usize].clone())
                            }
                            Metadatum::Switch { datum } => cp::Value::Switch(datum.default),
                        },
                    ))
                })
                .collect(),
        }
    }

    /// This tries to apply the given serialization snapshot to the running store.
    ///
    /// Note that this could fail if the queue is full.
    ///
    /// If the snapshot is incompatible (meaning either there was a programmer error,
    /// a data model change that was disallowed by the rules in
    /// `conformal_component::parameters::serialization`, a corrupt snapshot, or a
    /// snapshot from a newer version of the plug-in), we will reset to default
    /// state.
    pub fn apply_snapshot(
        &mut self,
        snapshot: &cc::serialization::Snapshot,
    ) -> Result<(), SnapshotError> {
        self.drop_garbage();

        let decoded =
            Arc::new(match snapshot
                .clone()
                .into_snapshot(self.metadata.data.iter().filter_map(|(id, metadatum)| {
                    let unhashed = self.unhash_for_snapshot.get(id)?;
                    Some((
                        unhashed.as_str(),
                        match metadatum {
                            Metadatum::Numeric { datum } => {
                                cc::serialization::ReadInfoRef::Numeric {
                                    default: datum.default,
                                    valid_range: datum.valid_range.clone(),
                                }
                            }
                            Metadatum::Enum { datum } => cc::serialization::ReadInfoRef::Enum {
                                default: datum.default,
                                values: datum.values.iter().map(String::as_str),
                            },
                            Metadatum::Switch { datum } => cc::serialization::ReadInfoRef::Switch {
                                default: datum.default,
                            },
                        },
                    ))
                })) {
                Ok(decoded) => Ok(decoded),
                Err(cc::serialization::DeserializationError::Corrupted(_)) => {
                    Err(SnapshotError::SnapshotCorrupted)
                }
                Err(cc::serialization::DeserializationError::VersionTooNew()) => {
                    // If the version was too new, we just use the default state
                    Ok(self.get_default_snapshot())
                }
            }?);
        self.cached_write_snapshot = Some(decoded.clone());
        self.write_generation = self.write_generation.wrapping_add(1);
        self.snapshot_tx
            .try_send(SnapshotMessage {
                snapshot: decoded.clone(),
                generation: self.write_generation,
            })
            .map_err(|_| SnapshotError::QueueTooFull)
    }
}

struct QueueImpl {
    initial_value: cp::InternalValue,
    com_ptr: *mut IParamValueQueue,
}

enum ValueOrQueue {
    Value(cp::InternalValue),
    Queue(QueueImpl),
}

struct Scratch {
    data: cp::IdHashMap<Option<ValueOrQueue>>,
}

impl Scratch {
    fn new(metadata: &Metadata) -> Self {
        Self {
            data: metadata.data.keys().map(|k| (*k, None)).collect(),
        }
    }
}

struct Metadata {
    data: cp::IdHashMap<Metadatum>,
}

impl Metadata {
    fn new<'a, S: AsRef<str> + 'a, I: IntoIterator<Item = cp::InfoRef<'a, S>>>(infos: I) -> Self {
        Self {
            data: infos
                .into_iter()
                .map(|info| {
                    let id = cp::hash_id(info.unique_id);
                    let data = match &info.type_specific {
                        TypeSpecificInfoRef::Enum { values, default } => Metadatum::Enum {
                            datum: EnumParamMetadatum {
                                default: *default,
                                values: values.iter().map(|s| s.as_ref().to_string()).collect(),
                            },
                        },
                        TypeSpecificInfoRef::Numeric {
                            valid_range,
                            default,
                            ..
                        } => Metadatum::Numeric {
                            datum: NumericParamMetadatum {
                                default: *default,
                                valid_range: valid_range.clone(),
                            },
                        },
                        TypeSpecificInfoRef::Switch { default } => Metadatum::Switch {
                            datum: SwitchParamMetadatum { default: *default },
                        },
                    };
                    (id, data)
                })
                .collect(),
        }
    }
}

fn convert_value(value: f64, metadatum: &Metadatum) -> cp::InternalValue {
    // Note that we defend against value being outside of [0, 1] in
    // `check_queue`, which is a precondition for this function.
    assert!((0.0..=1.0).contains(&value));

    match metadatum {
        Metadatum::Numeric { datum } => {
            cp::InternalValue::Numeric(convert_numeric(value, &datum.valid_range))
        }
        Metadatum::Enum { datum } => {
            cp::InternalValue::Enum(convert_enum(value, datum.values.len().try_into().unwrap()))
        }
        Metadatum::Switch { .. } => cp::InternalValue::Switch(convert_switch(value)),
    }
}

#[derive(Debug, Copy, Clone)]
enum QueueResult {
    /// A cromulent queue
    Ok {
        initial_value: cp::InternalValue,
        last_value: cp::InternalValue,
    },

    /// A queue that violates some pre-condition
    Invalid,

    /// A valid queue that didn't change anything
    Unchanged { value: cp::InternalValue },

    /// A valid queue that contains a change at 0, but no other changes
    Constant { value: cp::InternalValue },
}

unsafe fn check_queue(
    value_queue: ComRef<'_, IParamValueQueue>,
    metadatum: &Metadatum,
    mut initial_value: cp::InternalValue,
) -> QueueResult {
    let mut last_offset = None;
    let mut last_value = initial_value;

    for RawQueuePoint {
        value: raw_value,
        sample_offset,
    } in raw_iterator_from_queue(value_queue)
    {
        // If we're negative or outside of the buffer, we're invalid!
        if sample_offset < 0 {
            return QueueResult::Invalid;
        }

        if !(0.0..=1.0).contains(&raw_value) {
            return QueueResult::Invalid;
        }
        let value = convert_value(raw_value, metadatum);
        if value == last_value {
            continue;
        }

        last_offset = Some(sample_offset);
        last_value = value;
        if sample_offset == 0 {
            initial_value = value;
        }
    }

    match last_offset {
        Some(0) => QueueResult::Constant { value: last_value },
        None => QueueResult::Unchanged { value: last_value },
        _ => QueueResult::Ok {
            initial_value,
            last_value,
        },
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ChangesStatus {
    NoChanges,
    Changes,
}

// This is a marker type that indicates the scratch data
// has been initialized.
#[derive(Clone)]
struct InitializedScratch<'a> {
    metadata: &'a Metadata,
    data: &'a cp::IdHashMap<Option<ValueOrQueue>>,
    buffer_size: usize,
}

#[derive(Debug, Clone)]
struct RawQueuePoint {
    sample_offset: i32,
    value: f64,
}

fn raw_iterator_from_queue(
    queue: ComRef<'_, IParamValueQueue>,
) -> impl Iterator<Item = RawQueuePoint> + Clone {
    unsafe {
        let point_count = queue.getPointCount().max(0);
        (0..point_count).filter_map(move |idx| {
            let mut value = 0.0;
            let mut sample_offset = 0;
            if queue.getPoint(idx, &raw mut sample_offset, &raw mut value)
                == vst3::Steinberg::kResultOk
            {
                Some(RawQueuePoint {
                    sample_offset,
                    value,
                })
            } else {
                None
            }
        })
    }
}

trait CurveIteratorMetadatum {
    type CurvePoint: Clone;
    type Value;
    fn point_from_raw(&self, raw_point: RawQueuePoint) -> Self::CurvePoint;
    fn initial_point_from_value(&self, value: Self::Value) -> Self::CurvePoint;
}

impl CurveIteratorMetadatum for NumericParamMetadatum {
    type CurvePoint = PiecewiseLinearCurvePoint;
    type Value = f32;
    fn point_from_raw(
        &self,
        RawQueuePoint {
            sample_offset,
            value,
        }: RawQueuePoint,
    ) -> Self::CurvePoint {
        PiecewiseLinearCurvePoint {
            sample_offset: sample_offset.max(0) as usize,
            value: convert_numeric(value, &self.valid_range),
        }
    }

    fn initial_point_from_value(&self, value: Self::Value) -> Self::CurvePoint {
        PiecewiseLinearCurvePoint {
            sample_offset: 0,
            value,
        }
    }
}

fn curve_iterator_from_queue<'a, M: CurveIteratorMetadatum>(
    initial_value: M::Value,
    queue: ComRef<'a, IParamValueQueue>,
    metadatum: &'a M,
) -> impl Iterator<Item = M::CurvePoint> + Clone {
    let mut queue_points = raw_iterator_from_queue(queue).peekable();
    let queue_starts_at_zero = matches!(
        queue_points.peek(),
        Some(RawQueuePoint {
            sample_offset: 0,
            ..
        })
    );
    std::iter::once(metadatum.initial_point_from_value(initial_value))
        .filter_map(move |p| if queue_starts_at_zero { None } else { Some(p) })
        .chain(queue_points.map(|p| metadatum.point_from_raw(p)))
}

impl CurveIteratorMetadatum for EnumParamMetadatum {
    type CurvePoint = TimedValue<u32>;
    type Value = u32;
    fn point_from_raw(
        &self,
        RawQueuePoint {
            sample_offset,
            value,
        }: RawQueuePoint,
    ) -> Self::CurvePoint {
        TimedValue {
            sample_offset: sample_offset.max(0) as usize,
            value: convert_enum(value, self.values.len().try_into().unwrap()),
        }
    }

    fn initial_point_from_value(&self, value: Self::Value) -> Self::CurvePoint {
        TimedValue {
            sample_offset: 0,
            value,
        }
    }
}

impl CurveIteratorMetadatum for SwitchParamMetadatum {
    type CurvePoint = TimedValue<bool>;
    type Value = bool;
    fn point_from_raw(
        &self,
        RawQueuePoint {
            sample_offset,
            value,
        }: RawQueuePoint,
    ) -> Self::CurvePoint {
        TimedValue {
            sample_offset: sample_offset.max(0) as usize,
            value: convert_switch(value),
        }
    }

    fn initial_point_from_value(&self, value: Self::Value) -> Self::CurvePoint {
        TimedValue {
            sample_offset: 0,
            value,
        }
    }
}

impl BufferStates for InitializedScratch<'_> {
    fn get_by_hash(
        &self,
        param_id: cp::IdHash,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        match self.data.get(&param_id)? {
            Some(ValueOrQueue::Value(cp::InternalValue::Numeric(v))) => {
                Some(BufferState::Numeric(NumericBufferState::Constant(*v)))
            }
            Some(ValueOrQueue::Queue(QueueImpl {
                initial_value: cp::InternalValue::Numeric(v),
                com_ptr,
            })) => {
                let queue = unsafe { ComRef::from_raw(*com_ptr) }?;
                let Metadatum::Numeric { datum } = self.metadata.data.get(&param_id)? else {
                    return None;
                };
                Some(BufferState::Numeric(NumericBufferState::PiecewiseLinear(
                    PiecewiseLinearCurve::new(
                        curve_iterator_from_queue(*v, queue, datum),
                        self.buffer_size,
                        datum.valid_range.clone(),
                    )?,
                )))
            }
            Some(ValueOrQueue::Value(cp::InternalValue::Enum(v))) => {
                Some(BufferState::Enum(EnumBufferState::Constant(*v)))
            }
            Some(ValueOrQueue::Queue(QueueImpl {
                initial_value: cp::InternalValue::Enum(v),
                com_ptr,
            })) => {
                let queue = unsafe { ComRef::from_raw(*com_ptr) }?;
                let Metadatum::Enum { datum } = self.metadata.data.get(&param_id)? else {
                    return None;
                };
                Some(BufferState::Enum(EnumBufferState::Varying(
                    TimedEnumValues::new(
                        curve_iterator_from_queue(*v, queue, datum),
                        self.buffer_size,
                        0..(u32::try_from(datum.values.len()).unwrap()),
                    )?,
                )))
            }
            Some(ValueOrQueue::Value(cp::InternalValue::Switch(v))) => {
                Some(BufferState::Switch(SwitchBufferState::Constant(*v)))
            }
            Some(ValueOrQueue::Queue(QueueImpl {
                initial_value: cp::InternalValue::Switch(v),
                com_ptr,
            })) => {
                let queue = unsafe { ComRef::from_raw(*com_ptr) }?;
                let Metadatum::Switch { datum } = self.metadata.data.get(&param_id)? else {
                    return None;
                };
                Some(BufferState::Switch(SwitchBufferState::Varying(
                    TimedSwitchValues::new(
                        curve_iterator_from_queue(*v, queue, datum),
                        self.buffer_size,
                    )?,
                )))
            }
            None => None,
        }
    }
}

#[derive(Clone)]
struct InitializedScratchWithMpe<'a, I> {
    scratch: InitializedScratch<'a>,
    mpe: &'a mpe::State,
    mpe_events: Option<mpe::NoteEvents<I>>,
}

impl<I> BufferStates for InitializedScratchWithMpe<'_, I> {
    fn get_by_hash(
        &self,
        param_id: cp::IdHash,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        self.scratch.get_by_hash(param_id)
    }
}
impl<I: Iterator<Item = mpe::NoteEvent> + Clone> SynthParamBufferStates
    for InitializedScratchWithMpe<'_, I>
{
    fn get_numeric_global_expression(
        &self,
        expression: conformal_component::synth::NumericGlobalExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        self.mpe
            .get_numeric_global_expression_buffer(expression, &self.scratch)
    }

    fn get_switch_global_expression(
        &self,
        expression: conformal_component::synth::SwitchGlobalExpression,
    ) -> SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone> {
        self.mpe
            .get_switch_global_expression_buffer(expression, &self.scratch)
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: conformal_component::synth::NumericPerNoteExpression,
        note_id: conformal_component::events::NoteID,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        self.mpe.get_numeric_expression_for_note_buffer(
            expression,
            note_id,
            &self.scratch,
            self.mpe_events.clone(),
        )
    }
}

#[derive(Clone)]
struct ExistingBufferStates<'a> {
    store: &'a ProcessingStoreCore,
}

impl<'a> ExistingBufferStates<'a> {
    fn new(store: &'a ProcessingStore) -> Self {
        Self { store: &store.core }
    }
}

pub fn existing_buffer_states_from_store(store: &ProcessingStore) -> impl BufferStates + Clone {
    ExistingBufferStates::new(store)
}

pub fn existing_synth_param_buffer_states_from_store<'a>(
    store: &'a ProcessingStore,
    mpe: &'a mpe::State,
    mpe_events: Option<mpe::NoteEvents<impl Iterator<Item = mpe::NoteEvent> + Clone>>,
) -> impl SynthParamBufferStates + Clone {
    ExistingBufferStatesWithMpe {
        existing: ExistingBufferStates::new(store),
        mpe,
        mpe_events,
    }
}

impl BufferStates for ExistingBufferStates<'_> {
    fn get_by_hash(
        &self,
        param_id: cp::IdHash,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        match self.store.get_by_hash(param_id)? {
            cp::InternalValue::Numeric(v) => Some(BufferState::Numeric(NumericBufferState::<
                std::iter::Empty<PiecewiseLinearCurvePoint>,
            >::Constant(v))),
            cp::InternalValue::Enum(v) => Some(BufferState::Enum(EnumBufferState::<
                std::iter::Empty<TimedValue<u32>>,
            >::Constant(v))),
            cp::InternalValue::Switch(v) => Some(BufferState::Switch(SwitchBufferState::<
                std::iter::Empty<TimedValue<bool>>,
            >::Constant(v))),
        }
    }
}

#[derive(Clone)]
struct ExistingBufferStatesWithMpe<'a, I> {
    existing: ExistingBufferStates<'a>,
    mpe: &'a mpe::State,
    mpe_events: Option<mpe::NoteEvents<I>>,
}

impl<I> BufferStates for ExistingBufferStatesWithMpe<'_, I> {
    fn get_by_hash(
        &self,
        param_id: cp::IdHash,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        self.existing.get_by_hash(param_id)
    }
}

impl<I: Iterator<Item = mpe::NoteEvent> + Clone> SynthParamBufferStates
    for ExistingBufferStatesWithMpe<'_, I>
{
    fn get_numeric_global_expression(
        &self,
        expression: conformal_component::synth::NumericGlobalExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        self.mpe
            .get_numeric_global_expression_buffer(expression, &self.existing)
    }
    fn get_switch_global_expression(
        &self,
        expression: conformal_component::synth::SwitchGlobalExpression,
    ) -> SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone> {
        self.mpe
            .get_switch_global_expression_buffer(expression, &self.existing)
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: conformal_component::synth::NumericPerNoteExpression,
        note_id: conformal_component::events::NoteID,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        self.mpe.get_numeric_expression_for_note_buffer(
            expression,
            note_id,
            &self.existing,
            self.mpe_events.clone(),
        )
    }
}

fn check_downstream_invariants(
    initial_value: cp::InternalValue,
    metadatum: &Metadatum,
    queue: ComRef<'_, IParamValueQueue>,
    buffer_size: usize,
) -> bool {
    match (initial_value, metadatum) {
        (cp::InternalValue::Numeric(v), Metadatum::Numeric { datum }) => PiecewiseLinearCurve::new(
            curve_iterator_from_queue(v, queue, datum),
            buffer_size,
            datum.valid_range.clone(),
        )
        .is_some(),
        (cp::InternalValue::Enum(v), Metadatum::Enum { datum }) => TimedEnumValues::new(
            curve_iterator_from_queue(v, queue, datum),
            buffer_size,
            0..(u32::try_from(datum.values.len()).unwrap()),
        )
        .is_some(),
        (cp::InternalValue::Switch(v), Metadatum::Switch { datum }) => {
            TimedSwitchValues::new(curve_iterator_from_queue(v, queue, datum), buffer_size)
                .is_some()
        }
        _ => false,
    }
}

unsafe fn check_changes_and_update_scratch_and_store<'a>(
    changes: ComRef<'a, IParameterChanges>,
    scratch: &'a mut Scratch,
    store: &'a ProcessingStoreCore,
    buffer_size: usize,
) -> Option<(ChangesStatus, InitializedScratch<'a>)> {
    unsafe {
        let param_count = changes.getParameterCount();
        let mut change_status = ChangesStatus::NoChanges;
        if param_count < 0 {
            return None;
        }
        // Clear all the checker flags
        for v in scratch.data.values_mut() {
            *v = None;
        }
        if !(0..param_count).all(|idx| {
            let param_queue = changes.getParameterData(idx);
            ComRef::from_raw(param_queue).is_some_and(|q| {
                let parameter_id = cp::id_hash_from_internal_hash(q.getParameterId());
                let point_count = q.getPointCount();
                if point_count < 0 {
                    return false;
                }
                match (
                    scratch.data.get_mut(&parameter_id),
                    store.metadata.data.get(&parameter_id),
                    store.get_by_hash(parameter_id),
                ) {
                    (Some(scratch_v), Some(metadatum), Some(old_value)) => {
                        if scratch_v.is_some() {
                            return false;
                        }
                        match check_queue(q, metadatum, old_value) {
                            QueueResult::Invalid => {
                                return false;
                            }
                            QueueResult::Ok {
                                initial_value,
                                last_value,
                            } => {
                                // Check downstream invariants for this queue.
                                if !check_downstream_invariants(
                                    initial_value,
                                    metadatum,
                                    q,
                                    buffer_size,
                                ) {
                                    return false;
                                }

                                if !store.set(parameter_id, last_value) {
                                    return false;
                                }

                                *scratch_v = Some(ValueOrQueue::Queue(QueueImpl {
                                    initial_value,
                                    com_ptr: param_queue,
                                }));
                                change_status = ChangesStatus::Changes;
                            }
                            QueueResult::Unchanged { value } => {
                                *scratch_v = Some(ValueOrQueue::Value(value));
                            }
                            QueueResult::Constant { value } => {
                                if !store.set(parameter_id, value) {
                                    return false;
                                }

                                *scratch_v = Some(ValueOrQueue::Value(value));
                                change_status = ChangesStatus::Changes;
                            }
                        }
                        true
                    }
                    _ => false,
                }
            })
        }) {
            return None;
        }

        for (k, v) in &mut scratch.data {
            if v.is_none() {
                // Initialize any unchanged parameters to their current store value.
                *v = Some(ValueOrQueue::Value(store.get_by_hash(*k)?));
            }
        }
        Some((
            change_status,
            InitializedScratch {
                data: &scratch.data,
                metadata: &store.metadata,
                buffer_size,
            },
        ))
    }
}

unsafe fn internal_param_changes_from_vst3<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
    buffer_size: usize,
) -> Option<InitializedScratch<'a>> {
    unsafe {
        let (_, states) = check_changes_and_update_scratch_and_store(
            com_changes,
            &mut store.scratch,
            &store.core,
            buffer_size,
        )?;
        Some(states)
    }
}

pub unsafe fn param_changes_from_vst3<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
    buffer_size: usize,
) -> Option<impl BufferStates + Clone> {
    unsafe { internal_param_changes_from_vst3(com_changes, store, buffer_size) }
}

/// Only safe to call if the store was initialized with extra synth parameters!
pub unsafe fn synth_param_changes_from_vst3<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
    buffer_size: usize,
    mpe: &'a mpe::State,
    mpe_events: Option<mpe::NoteEvents<impl Iterator<Item = mpe::NoteEvent> + Clone>>,
) -> Option<impl SynthParamBufferStates + Clone> {
    let scratch = unsafe { internal_param_changes_from_vst3(com_changes, store, buffer_size) }?;
    Some(InitializedScratchWithMpe {
        scratch,
        mpe,
        mpe_events,
    })
}

unsafe fn no_audio_param_changes_from_vst3_internal<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
) -> Option<(ChangesStatus, &'a ProcessingStoreCore)> {
    unsafe {
        let (status, scratch) = check_changes_and_update_scratch_and_store(
            com_changes,
            &mut store.scratch,
            &store.core,
            0,
        )?;
        for scratch_value in scratch.data.values().flatten() {
            if let ValueOrQueue::Queue(_) = scratch_value {
                return None;
            }
        }
        Some((status, &store.core))
    }
}

pub unsafe fn no_audio_param_changes_from_vst3<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
) -> Option<(ChangesStatus, impl cp::States + Clone)> {
    unsafe { no_audio_param_changes_from_vst3_internal(com_changes, store) }
}

pub unsafe fn no_audio_synth_param_changes_from_vst3<'a>(
    com_changes: ComRef<'a, IParameterChanges>,
    store: &'a mut ProcessingStore,
    mpe: &'a mpe::State,
) -> Option<(ChangesStatus, impl SynthParamStates + Clone)> {
    let (change_status, core) =
        unsafe { no_audio_param_changes_from_vst3_internal(com_changes, store) }?;

    Some((change_status, CoreWithMpe { core, mpe }))
}

pub unsafe fn existing_synth_params<'a>(
    store: &'a mut ProcessingStore,
    mpe: &'a mpe::State,
) -> impl SynthParamStates + Clone {
    CoreWithMpe {
        core: &store.core,
        mpe,
    }
}
