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
mod tests;
