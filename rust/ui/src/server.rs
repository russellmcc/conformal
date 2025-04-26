use std::cell::RefCell;
use std::collections::HashSet;

use super::protocol;
use conformal_component::parameters;
use conformal_preferences::Store as PreferenceStore;

/// It is the job of the server to connect the UI to the state of the plug-in.
pub struct Server<S, R> {
    param_store: S,
    pref_store: Box<RefCell<dyn PreferenceStore>>,
    response_sender: R,
    subscriptions: HashSet<String>,
}

pub trait ResponseSender {
    fn send(&mut self, response: protocol::Response);
    fn on_pref_update(&mut self, unique_id: &str, value: &conformal_preferences::Value);
}

impl<S: super::ParameterStore, R: ResponseSender> Server<S, R> {
    pub fn new(
        param_store: S,
        pref_store: Box<RefCell<dyn PreferenceStore>>,
        response_sender: R,
    ) -> Self {
        Server {
            param_store,
            pref_store,
            response_sender,
            subscriptions: Default::default(),
        }
    }

    /// Handle a request from the UI.
    ///
    /// Note that this _may_ call `send` on the `response_sender` passed to `new`
    pub fn handle_request(&mut self, request: &protocol::Request) {
        match request {
            protocol::Request::Subscribe { path } => {
                if let Some(parameter) = path.strip_prefix("params/") {
                    if let Some(value) = self.param_store.get(parameter) {
                        self.subscriptions.insert(path.clone());
                        self.response_sender.send(protocol::Response::Values {
                            values: [(path.clone(), value.into())].into(),
                        });
                        return;
                    }
                } else if let Some(parameter) = path.strip_prefix("params-info/") {
                    if let Some(info) = self.param_store.get_info(parameter) {
                        self.response_sender.send(protocol::Response::Values {
                            values: [(
                                path.clone(),
                                protocol::serialize_as_bytes(
                                    &Into::<protocol::parameter_info::Info>::into(info),
                                )
                                .into(),
                            )]
                            .into(),
                        });
                        return;
                    }
                } else if let Some(preference) = path.strip_prefix("prefs/") {
                    if let Ok(value) = self.pref_store.borrow().get(preference) {
                        self.subscriptions.insert(path.clone());
                        self.response_sender.send(protocol::Response::Values {
                            values: [(path.clone(), value.into())].into(),
                        });
                        return;
                    }
                } else if path == "ui-state" {
                    self.subscriptions.insert(path.clone());
                    self.response_sender.send(protocol::Response::Values {
                        values: [(path.clone(), self.param_store.get_ui_state().into())].into(),
                    });
                    return;
                }
                self.response_sender
                    .send(protocol::Response::SubscribeValueError { path: path.clone() });
            }
            protocol::Request::Unsubscribe { path } => {
                self.subscriptions.remove(path);
            }
            protocol::Request::Set { path, value } => {
                if let Some(parameter) = path.strip_prefix("params/") {
                    if let Ok(v) = value.clone().try_into() {
                        if self.param_store.set(parameter, v).is_err() {
                            // HACK - eventually we should probably send an
                            // error to the client - for now we just respond
                            // with the current value, if subscribed.
                            if let Some(value) = self.param_store.get(parameter) {
                                self.update_parameter(parameter, &value);
                            }
                        }
                    }
                } else if let Some(parameter) = path.strip_prefix("params-grabbed/") {
                    if let protocol::Value::Bool(b) = value {
                        let _ = self.param_store.set_grabbed(parameter, *b);
                    }
                } else if let Some(preference) = path.strip_prefix("prefs/") {
                    if let Ok(v) = value.clone().try_into() {
                        if self.pref_store.borrow_mut().set(preference, v).is_ok() {
                            // Same hack as note above.
                            let ret = self.pref_store.borrow().get(preference);
                            if let Ok(value) = ret {
                                self.update_preference(preference, &value);
                            }
                        }
                    }
                } else if path == "ui-state" {
                    if let protocol::Value::Bytes(bytes) = value {
                        self.param_store.set_ui_state(bytes);
                    }
                }
            }
        }
    }

    /// Handle a change to a parameter in the store.
    /// Note that this _may_ call `send` on the `response_sender` passed to `new`.
    pub fn update_parameter(&mut self, unique_id: &str, value: &parameters::Value) {
        let path = format!("params/{unique_id}");
        if self.subscriptions.contains(&path) {
            self.response_sender.send(protocol::Response::Values {
                values: [(path, value.clone().into())].into(),
            });
        }
    }

    pub fn update_preference(&mut self, unique_id: &str, value: &conformal_preferences::Value) {
        let path = format!("prefs/{unique_id}");
        if self.subscriptions.contains(&path) {
            self.response_sender.send(protocol::Response::Values {
                values: [(path, value.clone().into())].into(),
            });
        }
        self.response_sender.on_pref_update(unique_id, value);
    }

    pub fn update_ui_state(&mut self, state: &[u8]) {
        let path = "ui-state";
        if self.subscriptions.contains(path) {
            self.response_sender.send(protocol::Response::Values {
                values: [(path.to_owned(), state.to_owned().into())].into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    //! Note - this module assumes a _synchronous_ server. This is not mandated by
    //! the client, it's just how the current server is implemented. In the future,
    //! we may want to change the server to be asynchronous, in which case we should
    //! re-write or delete the tests.

    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        rc::Rc,
    };

    use crate::{
        ParameterStore,
        protocol::{self, Request, Response},
    };
    use conformal_component::parameters::Value;
    use conformal_core::parameters::store::{SetError, SetGrabbedError};

    use super::{ResponseSender, Server};
    struct ResponseSenderSpy<'a> {
        sent: &'a RefCell<Vec<Response>>,
        pref_updates: &'a RefCell<Vec<(String, conformal_preferences::Value)>>,
    }

    impl<'a> ResponseSender for ResponseSenderSpy<'a> {
        fn send(&mut self, response: Response) {
            self.sent.borrow_mut().push(response);
        }
        fn on_pref_update(&mut self, unique_id: &str, value: &conformal_preferences::Value) {
            self.pref_updates
                .borrow_mut()
                .push((unique_id.to_string(), value.clone()));
        }
    }

    #[derive(Clone, Default)]
    struct StubStoreData {
        values: HashMap<String, conformal_component::parameters::Value>,
        grabbed: HashSet<String>,
    }

    impl<I: IntoIterator<Item = (String, conformal_component::parameters::Value)>> From<I>
        for StubStoreData
    {
        fn from(values: I) -> Self {
            StubStoreData {
                values: values.into_iter().collect(),
                grabbed: Default::default(),
            }
        }
    }

    #[derive(Clone)]
    struct StubStore {
        values: Rc<RefCell<StubStoreData>>,
        ui_state: Rc<RefCell<Vec<u8>>>,
    }

    impl crate::ParameterStore for StubStore {
        fn get(&self, unique_id: &str) -> Option<Value> {
            self.values.borrow().values.get(unique_id).cloned()
        }

        fn set(&mut self, unique_id: &str, value: Value) -> Result<(), SetError> {
            self.values
                .borrow_mut()
                .values
                .insert(unique_id.to_string(), value);
            Ok(())
        }

        fn set_grabbed(&mut self, unique_id: &str, grabbed: bool) -> Result<(), SetGrabbedError> {
            if grabbed {
                self.values
                    .borrow_mut()
                    .grabbed
                    .insert(unique_id.to_string());
            } else {
                self.values.borrow_mut().grabbed.remove(unique_id);
            }
            Ok(())
        }

        fn get_info(&self, unique_id: &str) -> Option<conformal_component::parameters::Info> {
            if unique_id == "a" {
                Some(conformal_component::parameters::Info {
                    title: "Test Title".to_string(),
                    short_title: "Test Short Title".to_string(),
                    unique_id: "a".to_string(),
                    flags: conformal_component::parameters::Flags { automatable: true },
                    type_specific: conformal_component::parameters::TypeSpecificInfo::Numeric {
                        default: 1.0,
                        valid_range: 0.0..=10.0,
                        units: Some("Hz".to_string()),
                    },
                })
            } else {
                None
            }
        }

        fn get_ui_state(&self) -> Vec<u8> {
            self.ui_state.borrow().clone()
        }

        fn set_ui_state(&mut self, state: &[u8]) {
            self.ui_state.borrow_mut().clear();
            self.ui_state.borrow_mut().extend_from_slice(state);
        }
    }

    #[test]
    fn subscribing_to_parameter() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "params/a".to_string(),
        });
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values
                    .iter()
                    .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(1.0)),
                _ => false,
            }
        }));
        sent.borrow_mut().clear();
        server.update_parameter("a", &2.0.into());
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values
                    .iter()
                    .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(2.0)),
                _ => false,
            }
        }));

        server.handle_request(&Request::Unsubscribe {
            path: "params/a".to_string(),
        });
        sent.borrow_mut().clear();

        // Note that this behavior cannot depended on by the client - but for now
        // we try not to send updates to unsubscribed parameters.
        server.update_parameter("a", &3.0.into());
        assert!(sent.borrow().iter().all(|m| {
            !match m {
                Response::Values { values } => values
                    .iter()
                    .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(3.0)),
                _ => false,
            }
        }));
    }

    #[test]
    fn subscribing_to_ui_state() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "ui-state".to_string(),
        });
        println!("sent: {:?}", sent.borrow());
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values
                    .iter()
                    .any(|(p, v)| p == "ui-state" && v == &protocol::Value::Bytes(vec![])),
                _ => false,
            }
        }));
        sent.borrow_mut().clear();
        server.param_store.set_ui_state(&[1, 2, 3]);
        server.update_ui_state(&[1, 2, 3]);
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values
                    .iter()
                    .any(|(p, v)| p == "ui-state" && v == &protocol::Value::Bytes(vec![1, 2, 3])),
                _ => false,
            }
        }));
    }

    #[test]
    fn defends_against_subscriptions_to_non_existing_paths() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        let nonsense_path = "nonsense path that does not exist".to_string();
        server.handle_request(&Request::Subscribe {
            path: nonsense_path.clone(),
        });
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::SubscribeValueError { path } => path == &nonsense_path,
                _ => false,
            }
        }));
    }

    #[test]
    fn defends_against_subscription_to_parameter_with_invalid_path() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "params/b".to_string(),
        });
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::SubscribeValueError { path } => path == "params/b",
                _ => false,
            }
        }));
    }

    #[test]
    fn set_does_not_echo_value() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "params/a".to_string(),
        });
        sent.borrow_mut().clear();
        server.handle_request(&Request::Set {
            path: "params/a".to_string(),
            value: protocol::Value::Numeric(2.0),
        });
        assert!(sent.borrow().is_empty());
    }

    #[test]
    fn set_changes_store() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "params/a".to_string(),
        });
        sent.borrow_mut().clear();
        server.handle_request(&Request::Set {
            path: "params/a".to_string(),
            value: protocol::Value::Numeric(2.0),
        });
        assert_eq!(
            store.values.borrow().values.get("a"),
            Some(&conformal_component::parameters::Value::Numeric(2.0))
        );
    }

    #[test]
    fn grab_basics() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Set {
            path: "params-grabbed/a".to_string(),
            value: protocol::Value::Bool(true),
        });
        assert!(store.values.borrow().grabbed.contains("a"));
        server.handle_request(&Request::Set {
            path: "params-grabbed/a".to_string(),
            value: protocol::Value::Bool(false),
        });
        assert!(!store.values.borrow().grabbed.contains("a"));
    }

    #[test]
    fn get_info() {
        let sent = RefCell::new(Vec::new());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &RefCell::new(Default::default()),
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(Default::default()),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "params-info/a".to_string(),
        });
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values.iter().any(|(p, v)| {
                    if p != "params-info/a" {
                        return false;
                    }
                    if let protocol::Value::Bytes(b) = v {
                        if let Ok(i) =
                            protocol::deserialize_from_bytes::<protocol::parameter_info::Info>(b)
                        {
                            assert_eq!(i.title, "Test Title");
                            if let protocol::parameter_info::TypeSpecific::Numeric {
                                default,
                                valid_range,
                                units,
                            } = i.type_specific
                            {
                                assert_eq!(default, 1.0);
                                assert_eq!(valid_range, (0.0, 10.0));
                                assert_eq!(units, "Hz");
                                true
                            } else {
                                panic!();
                            }
                        } else {
                            panic!();
                        }
                    } else {
                        panic!();
                    }
                }),
                _ => false,
            }
        }));
    }

    #[test]
    fn get_set_preferences() {
        let sent = RefCell::new(Vec::new());
        let pref_updates = RefCell::new(Default::default());
        let sender = ResponseSenderSpy {
            sent: &sent,
            pref_updates: &pref_updates,
        };
        let store = StubStore {
            values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
            ui_state: Rc::new(RefCell::new(vec![])),
        };
        let mut server = Server::new(
            store.clone(),
            Box::new(RefCell::new(
                conformal_preferences::create_with_fake_os_store(HashMap::from_iter([(
                    "a".to_string(),
                    conformal_preferences::Value::Switch(false),
                )])),
            )),
            sender,
        );
        server.handle_request(&Request::Subscribe {
            path: "prefs/a".to_string(),
        });
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values.iter().any(|(p, v)| {
                    if p != "prefs/a" {
                        return false;
                    }
                    if let protocol::Value::Bool(b) = v {
                        *b == false
                    } else {
                        false
                    }
                }),
                _ => false,
            }
        }));
        sent.borrow_mut().clear();
        server.handle_request(&Request::Set {
            path: "prefs/a".to_string(),
            value: protocol::Value::Bool(true),
        });
        assert_eq!(
            pref_updates.borrow().as_slice(),
            &[("a".to_string(), conformal_preferences::Value::Switch(true))]
        );
        assert!(sent.borrow().iter().any(|m| {
            match m {
                Response::Values { values } => values.iter().any(|(p, v)| {
                    if p != "prefs/a" {
                        return false;
                    }
                    if let protocol::Value::Bool(b) = v {
                        *b == true
                    } else {
                        false
                    }
                }),
                _ => false,
            }
        }));
    }
}
