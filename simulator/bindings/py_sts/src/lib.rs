use pyo3::exceptions::{PyAttributeError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::IntoPyObjectExt;
use serde_json::{Map, Value};
use sts_env::{
    parse_seed, DecisionRevision, FairDecision, FairEnvironment, FairRunObservation, FairRunScreen,
    PublicChoice, PublicChoiceRequest,
};

#[pyclass(name = "Record", frozen)]
#[derive(Clone)]
pub struct PyRecord {
    fields: Map<String, Value>,
}

#[pymethods]
impl PyRecord {
    fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        self.fields
            .get(name)
            .cloned()
            .ok_or_else(|| PyAttributeError::new_err(name.to_owned()))
            .and_then(|value| value_to_python(py, value))
    }

    fn __repr__(&self) -> String {
        let names = self.fields.keys().cloned().collect::<Vec<_>>().join(", ");
        format!("Record({names})")
    }
}

#[pyclass(name = "Observation", frozen)]
#[derive(Clone)]
pub struct PyObservation {
    schema_version: u32,
    phase: String,
    kind: String,
    context: PyRecord,
    screen: Option<PyRecord>,
}

#[pymethods]
impl PyObservation {
    #[getter]
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
    #[getter]
    fn phase(&self) -> &str {
        &self.phase
    }
    #[getter]
    fn kind(&self) -> &str {
        &self.kind
    }
    #[getter]
    fn context(&self) -> PyRecord {
        self.context.clone()
    }
    #[getter]
    fn screen(&self) -> Option<PyRecord> {
        self.screen.clone()
    }
}

#[derive(Clone, Copy, Default)]
struct ActionSlots {
    hand: Option<usize>,
    potion: Option<usize>,
    option: Option<usize>,
    target: Option<usize>,
    card: Option<usize>,
    node: Option<usize>,
    reward: Option<usize>,
    shop: Option<usize>,
}

#[pyclass(name = "Action", frozen)]
#[derive(Clone)]
pub struct PyAction {
    choice: PublicChoice,
    revision: DecisionRevision,
    family: &'static str,
    kind: &'static str,
    slots: ActionSlots,
}

#[pymethods]
impl PyAction {
    #[getter]
    fn revision(&self) -> u64 {
        self.revision.get()
    }
    #[getter]
    fn family(&self) -> &'static str {
        self.family
    }
    #[getter]
    fn kind(&self) -> &'static str {
        self.kind
    }
    #[getter]
    fn hand_slot(&self) -> Option<usize> {
        self.slots.hand
    }
    #[getter]
    fn potion_slot(&self) -> Option<usize> {
        self.slots.potion
    }
    #[getter]
    fn option_slot(&self) -> Option<usize> {
        self.slots.option
    }
    #[getter]
    fn target_slot(&self) -> Option<usize> {
        self.slots.target
    }
    #[getter]
    fn card_slot(&self) -> Option<usize> {
        self.slots.card
    }
    #[getter]
    fn node_slot(&self) -> Option<usize> {
        self.slots.node
    }
    #[getter]
    fn reward_slot(&self) -> Option<usize> {
        self.slots.reward
    }
    #[getter]
    fn shop_slot(&self) -> Option<usize> {
        self.slots.shop
    }

    fn __repr__(&self) -> String {
        let mut fields = vec![format!("kind='{}'", self.kind)];
        for (name, value) in [
            ("hand_slot", self.slots.hand),
            ("potion_slot", self.slots.potion),
            ("option_slot", self.slots.option),
            ("target_slot", self.slots.target),
            ("card_slot", self.slots.card),
            ("node_slot", self.slots.node),
            ("reward_slot", self.slots.reward),
            ("shop_slot", self.slots.shop),
        ] {
            if let Some(value) = value {
                fields.push(format!("{name}={value}"));
            }
        }
        format!("Action({})", fields.join(", "))
    }
}

#[pyclass(name = "Decision", frozen)]
#[derive(Clone)]
pub struct PyDecision {
    schema_version: u32,
    revision: u64,
    observation: PyObservation,
    actions: Vec<PyAction>,
}

#[pymethods]
impl PyDecision {
    #[getter]
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
    #[getter]
    fn revision(&self) -> u64 {
        self.revision
    }
    #[getter]
    fn observation(&self) -> PyObservation {
        self.observation.clone()
    }
    #[getter]
    fn actions(&self) -> Vec<PyAction> {
        self.actions.clone()
    }
}

#[pyclass(name = "State")]
#[derive(Clone)]
pub struct PyState {
    env: FairEnvironment,
}

#[pymethods]
impl PyState {
    #[staticmethod]
    #[pyo3(signature = (seed, ascension=0))]
    fn new(seed: &str, ascension: u8) -> PyResult<Self> {
        let seed = parse_seed(seed).map_err(|error| PyValueError::new_err(error.to_string()))?;
        let env = FairEnvironment::new_ironclad(seed, ascension)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(Self { env })
    }

    fn clone(&self) -> Self {
        Clone::clone(self)
    }

    #[getter]
    fn revision(&self) -> u64 {
        self.env.revision().get()
    }

    fn observation(&self) -> PyResult<PyObservation> {
        self.env
            .observation()
            .map_err(public_runtime_error)
            .and_then(py_observation)
    }

    fn legal_actions(&self) -> PyResult<Vec<PyAction>> {
        let revision = self.env.revision();
        self.env
            .legal_choices()
            .map_err(public_runtime_error)
            .map(|choices| {
                choices
                    .into_iter()
                    .map(|choice| py_action(choice, revision))
                    .collect()
            })
    }

    fn decision(&self) -> PyResult<PyDecision> {
        self.env
            .decision()
            .map_err(public_runtime_error)
            .and_then(py_decision)
    }

    fn step(&mut self, action: &PyAction) -> PyResult<PyDecision> {
        self.env
            .step(PublicChoiceRequest {
                revision: action.revision,
                choice: action.choice,
            })
            .map_err(|error| PyValueError::new_err(error.to_string()))
            .and_then(py_decision)
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyState>()?;
    module.add_class::<PyAction>()?;
    module.add_class::<PyDecision>()?;
    module.add_class::<PyObservation>()?;
    module.add_class::<PyRecord>()?;
    Ok(())
}

fn py_decision(decision: FairDecision) -> PyResult<PyDecision> {
    let revision = decision.revision;
    Ok(PyDecision {
        schema_version: decision.schema_version,
        revision: revision.get(),
        observation: py_observation(decision.observation)?,
        actions: decision
            .choices
            .into_iter()
            .map(|choice| py_action(choice, revision))
            .collect(),
    })
}

fn py_observation(observation: FairRunObservation) -> PyResult<PyObservation> {
    let phase = serde_json::to_value(observation.phase)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| PyRuntimeError::new_err("invalid fair observation phase"))?;
    let kind = observation.screen.kind().to_owned();
    let context =
        value_to_record(serde_json::to_value(observation.context).map_err(public_runtime_error)?)?;
    let screen = match observation.screen {
        FairRunScreen::Combat(value) => some_record(value)?,
        FairRunScreen::Map(value) => some_record(value)?,
        FairRunScreen::Event(value) => some_record(value)?,
        FairRunScreen::Reward(value) => some_record(value)?,
        FairRunScreen::Treasure(value) => some_record(value)?,
        FairRunScreen::Rest(value) => some_record(value)?,
        FairRunScreen::Shop(value) => some_record(value)?,
        FairRunScreen::Grid(value) => some_record(value)?,
        FairRunScreen::Idle | FairRunScreen::Complete => None,
    };
    Ok(PyObservation {
        schema_version: observation.schema_version,
        phase,
        kind,
        context,
        screen,
    })
}

fn py_action(choice: PublicChoice, revision: DecisionRevision) -> PyAction {
    PyAction {
        choice,
        revision,
        family: choice.family(),
        kind: choice.kind(),
        slots: action_slots(choice),
    }
}

fn action_slots(choice: PublicChoice) -> ActionSlots {
    let mut slots = ActionSlots::default();
    match choice {
        PublicChoice::PlayHandSlot {
            hand_slot,
            target_slot,
        } => {
            slots.hand = Some(hand_slot.into());
            slots.target = target_slot.map(Into::into);
        }
        PublicChoice::UsePotionSlot {
            potion_slot,
            target_slot,
        } => {
            slots.potion = Some(potion_slot.into());
            slots.target = target_slot.map(Into::into);
        }
        PublicChoice::DiscardPotionSlot { potion_slot } => slots.potion = Some(potion_slot.into()),
        PublicChoice::ChooseEventOption { option_slot }
        | PublicChoice::ToggleVisibleCard { option_slot }
        | PublicChoice::ChooseVisibleOption { option_slot } => {
            slots.option = Some(option_slot.into())
        }
        PublicChoice::ToggleGridCard { card_slot }
        | PublicChoice::RestSmith { card_slot }
        | PublicChoice::RestRemoveCard { card_slot } => slots.card = Some(card_slot.into()),
        PublicChoice::ChooseMapNode { node_slot } => slots.node = Some(node_slot.into()),
        PublicChoice::TakeCardReward { reward_slot }
        | PublicChoice::TakePotionReward { reward_slot }
        | PublicChoice::TakeRelicRewardAt { reward_slot }
        | PublicChoice::ChooseBossRelicReward { reward_slot }
        | PublicChoice::OpenQueuedCardReward { reward_slot } => {
            slots.reward = Some(reward_slot.into())
        }
        PublicChoice::BuyShopCard { shop_slot }
        | PublicChoice::BuyShopRelic { shop_slot }
        | PublicChoice::BuyShopPotion { shop_slot } => slots.shop = Some(shop_slot.into()),
        _ => {}
    }
    slots
}

fn public_runtime_error(error: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn some_record<T: serde::Serialize>(value: T) -> PyResult<Option<PyRecord>> {
    serde_json::to_value(value)
        .map_err(public_runtime_error)
        .and_then(value_to_record)
        .map(Some)
}

fn value_to_record(value: Value) -> PyResult<PyRecord> {
    match value {
        Value::Object(fields) => Ok(PyRecord { fields }),
        _ => Err(PyRuntimeError::new_err("expected a typed record")),
    }
}

fn value_to_python(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(value) => value.into_py_any(py),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_u64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_f64() {
                value.into_py_any(py)
            } else {
                Err(PyRuntimeError::new_err("invalid numeric value"))
            }
        }
        Value::String(value) => value.into_py_any(py),
        Value::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| value_to_python(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(PyTuple::new(py, values)?.into_any().unbind())
        }
        Value::Object(fields) => Py::new(py, PyRecord { fields }).map(|value| value.into_any()),
    }
}
