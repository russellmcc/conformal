use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    rc::{Rc, Weak},
};

use wry::{
    http::{Method, Request, Response, StatusCode, header::CONTENT_TYPE},
    raw_window_handle,
};

use conformal_component::parameters;
#[cfg(target_os = "macos")]
use conformal_core::mac_bundle_utils::get_current_bundle_info;
use conformal_preferences::Store;

use super::{protocol, server};

#[derive(Debug, Clone, PartialEq)]
struct RawWindowHandleWrapper(raw_window_handle::RawWindowHandle);

impl raw_window_handle::HasWindowHandle for RawWindowHandleWrapper {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        unsafe { Ok(raw_window_handle::WindowHandle::borrow_raw(self.0)) }
    }
}

struct ResponseSender {
    web_view: Rc<RefCell<Weak<wry::WebView>>>,
}

const DEV_MODE_KEY: &str = "dev_mode";
const USE_WEB_DEV_SERVER_KEY: &str = "use_web_dev_server";

fn app_url(use_web_dev_server_pref: &conformal_preferences::Value) -> String {
    if *use_web_dev_server_pref == conformal_preferences::Value::Switch(true) {
        "http://localhost:5173".to_owned()
    } else {
        format!("rsrc://{RESOURCES_HOST}")
    }
}

impl server::ResponseSender for ResponseSender {
    fn send(&mut self, response: super::protocol::Response) {
        if let Some(web_view) = self.web_view.borrow().upgrade() {
            let resp = protocol::encode_message(&response);
            let _ = web_view.evaluate_script(&format!(
                "if (window.receiveMessage) {{ window.receiveMessage(\"{resp}\"); }}"
            ));
        }
    }

    fn on_pref_update(&mut self, unique_id: &str, value: &conformal_preferences::Value) {
        if unique_id == USE_WEB_DEV_SERVER_KEY
            && let Some(web_view) = self.web_view.borrow().upgrade()
        {
            let _ = web_view.load_url(&app_url(value));
        }
    }
}

pub struct Ui<S> {
    #[allow(dead_code)]
    // false-positive - we keep web_view alive, but we don't need to read from it.
    web_view: Rc<wry::WebView>,
    server: Rc<RefCell<server::Server<S, ResponseSender>>>,
}

fn make_plain_error(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .header(CONTENT_TYPE, "text/plain")
        .status(status)
        .body(message.as_bytes().to_vec())
        .unwrap()
}

const RESOURCES_HOST: &str = "resources";

fn get_rsrc_response(rsrc_root: &std::path::Path, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return make_plain_error(StatusCode::METHOD_NOT_ALLOWED, "Unsupported method");
    }
    if request.uri().host() != Some(RESOURCES_HOST) {
        return make_plain_error(
            StatusCode::NOT_FOUND,
            format!("Unknown host {}", request.uri().host().unwrap_or("none")).as_str(),
        );
    }
    let path = request.uri().path();
    let path = if path == "/" {
        "index.html"
    } else {
        &path[1..]
    };

    let local_path = rsrc_root.join(path);
    let content = fs::read(&local_path);
    match content {
        Ok(content) => {
            let mime = mime_guess::from_path(&local_path).first_or_octet_stream();
            Response::builder()
                .header(CONTENT_TYPE, mime.essence_str())
                .status(200)
                .body(content)
                .unwrap()
        }
        Err(_) => make_plain_error(
            StatusCode::NOT_FOUND,
            format!("File not found {path}").as_str(),
        ),
    }
}

// Note that we can't do anything without a resource root.
fn get_rsrc_root_or_panic() -> std::path::PathBuf {
    get_current_bundle_info()
        .map(|info| info.resource_path)
        .expect("Could not find bundle resources")
}

fn default_preferences() -> HashMap<String, conformal_preferences::Value> {
    HashMap::from_iter([
        (
            DEV_MODE_KEY.to_owned(),
            conformal_preferences::Value::Switch(false),
        ),
        (
            USE_WEB_DEV_SERVER_KEY.to_owned(),
            conformal_preferences::Value::Switch(false),
        ),
    ])
}

pub enum UiError {
    CouldNotConstruct(wry::Error),
}

/// Size in logical pixels
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl<S: super::ParameterStore + 'static> Ui<S> {
    /// # Errors
    ///
    ///  - returns `UiError::CouldNotConstruct` if the UI could not be constructed.
    pub fn new(
        parent: raw_window_handle::RawWindowHandle,
        store: S,
        domain: &str,
        size: Size,
    ) -> Result<Self, UiError> {
        let server_web_view = Rc::new(RefCell::new(Default::default()));
        let pref_store = Box::new(RefCell::new(conformal_preferences::create_store(
            domain,
            default_preferences(),
        )));
        let dev_mode_enabled =
            pref_store.borrow().get(DEV_MODE_KEY) == Ok(conformal_preferences::Value::Switch(true));
        let web_dev_server = pref_store
            .borrow()
            .get(USE_WEB_DEV_SERVER_KEY)
            .unwrap_or(conformal_preferences::Value::Switch(false));
        let server = Rc::new(RefCell::new(server::Server::new(
            store,
            pref_store,
            ResponseSender {
                web_view: server_web_view.clone(),
            },
        )));
        let server_ipc = server.clone();
        let rsrc_root = get_rsrc_root_or_panic().join("web-ui");
        let web_view = Rc::new(
            wry::WebViewBuilder::new_as_child(&RawWindowHandleWrapper(parent))
                .with_ipc_handler(move |m| {
                    if let Ok(message) = protocol::decode_message(m.body()) {
                        server_ipc.borrow_mut().handle_request(&message);
                    }
                    // We ignore any unknown messages - these could be from
                    // future clients!
                })
                .with_custom_protocol("rsrc".to_string(), move |request| {
                    get_rsrc_response(&rsrc_root, &request).map(Into::into)
                })
                .with_devtools(dev_mode_enabled)
                .with_url(app_url(&web_dev_server))
                .with_accept_first_mouse(true)
                .with_back_forward_navigation_gestures(false)
                .with_drag_drop_handler(|_| true)
                .build()
                .map_err(UiError::CouldNotConstruct)?,
        );

        server_web_view.replace(Rc::downgrade(&web_view));
        let _ = web_view.set_bounds(wry::Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition { x: 0f64, y: 0f64 }),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize {
                width: f64::from(size.width),
                height: f64::from(size.height),
            }),
        });
        Ok(Self { web_view, server })
    }

    /// Any time any parameter changes, this must be called with the new value.
    pub fn update_parameter(&mut self, unique_id: &str, value: &parameters::Value) {
        self.server.borrow_mut().update_parameter(unique_id, value);
    }

    /// This must be called whenever the UI state changes.
    pub fn update_ui_state(&mut self, state: &[u8]) {
        // NOTE - this will be called re-entrantly if a ui state change
        // initiates from the web ui. In this case, we just drop the update,
        // relying on the optimistic behavior of the web ui.
        if let Ok(mut server) = self.server.try_borrow_mut() {
            server.update_ui_state(state);
        }
    }
}
