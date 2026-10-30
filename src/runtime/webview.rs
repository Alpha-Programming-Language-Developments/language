use std::path::Path;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::WebViewBuilder;

use crate::diagnostics::error::{AlphaError, Span};
use crate::runtime::interpreter::Interpreter;
use crate::runtime::value::Value;

pub const WEBALPH_INIT_SCRIPT: &str = r#"
(function() {
    window.__webalph_callbacks = {};
    window.__webalph_listeners = {};
    let __webalph_next_id = 1;

    window.webalph = {
        invoke: function(cmd, args) {
            return new Promise((resolve, reject) => {
                const id = __webalph_next_id++;
                window.__webalph_callbacks[id] = { resolve, reject };
                window.ipc.postMessage(JSON.stringify({
                    type: "invoke",
                    cmd: cmd,
                    id: id,
                    args: args || {}
                }));
            });
        },
        emit: function(event, payload) {
            window.ipc.postMessage(JSON.stringify({
                type: "emit",
                event: event,
                payload: payload
            }));
        },
        listen: function(event, handler) {
            if (!window.__webalph_listeners[event]) {
                window.__webalph_listeners[event] = [];
            }
            window.__webalph_listeners[event].push(handler);
        }
    };

    window.__webalph_resolve = function(id, data, is_error) {
        const cb = window.__webalph_callbacks[id];
        if (cb) {
            delete window.__webalph_callbacks[id];
            if (is_error) cb.reject(data);
            else cb.resolve(data);
        }
    };

    window.__webalph_dispatch = function(event, data) {
        const list = window.__webalph_listeners[event];
        if (list) {
            list.forEach(fn => {
                try { fn(data); } catch (e) { console.error("WebAlph listener error:", e); }
            });
        }
    };
})();
"#;

#[derive(Clone, Debug)]
pub struct WebviewWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub html: Option<String>,
    pub url: Option<String>,
}

impl Default for WebviewWindowConfig {
    fn default() -> Self {
        Self {
            title: "WebAlph Application".to_string(),
            width: 900,
            height: 650,
            resizable: true,
            html: None,
            url: None,
        }
    }
}

pub fn resolve_webview_url(url: &str, working_dir: &Path) -> Result<String, String> {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("file://")
        || url.starts_with("data:")
        || url.starts_with("ws://")
        || url.starts_with("wss://")
    {
        return Ok(url.to_string());
    }

    let mut candidates = Vec::new();
    let p = Path::new(url);
    if p.is_absolute() {
        candidates.push(p.to_path_buf());
    }

    candidates.push(working_dir.join(url));

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(url));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join(url));
        }
    }

    if let Some(parent) = working_dir.parent() {
        candidates.push(parent.join(url));
    }

    if let Some(root) = crate::driver::project::find_project_root(working_dir) {
        candidates.push(root.join(url));
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = crate::driver::project::find_project_root(&cwd) {
            candidates.push(root.join(url));
        }
    }

    // Common asset subdirectories
    candidates.push(working_dir.join("ui").join(url));
    candidates.push(working_dir.join("src").join(url));

    for cand in candidates {
        if cand.is_file() {
            let canon = std::fs::canonicalize(&cand).unwrap_or(cand);
            let path_str = canon.to_string_lossy().replace('\\', "/");
            let clean = path_str.trim_start_matches("//?/");
            return Ok(format!("file:///{}", clean));
        }
    }

    Err(format!(
        "File '{}' not found. Searched in {:?} and project root.",
        url, working_dir
    ))
}

pub fn run_webview_loop(interp: &mut Interpreter, span: Span) -> Result<Value, AlphaError> {
    let cfg = interp.webview_config.clone();
    let mut event_loop = EventLoopBuilder::<String>::with_user_event().build();

    let window = WindowBuilder::new()
        .with_title(&cfg.title)
        .with_inner_size(LogicalSize::new(cfg.width as f64, cfg.height as f64))
        .with_resizable(cfg.resizable)
        .build(&event_loop)
        .map_err(|e| AlphaError::runtime(format!("Failed to create window: {}", e), span))?;

    let proxy = event_loop.create_proxy();
    let mut builder = WebViewBuilder::new()
        .with_initialization_script(WEBALPH_INIT_SCRIPT)
        .with_ipc_handler(move |req| {
            let _ = proxy.send_event(req.body().clone());
        });

    if let Some(html) = &cfg.html {
        builder = builder.with_html(html);
    } else if let Some(url) = &cfg.url {
        let resolved = resolve_webview_url(url, interp.working_dir())
            .map_err(|e| AlphaError::runtime(format!("Failed to build WebView: {}", e), span))?;
        builder = builder.with_url(&resolved);
    } else {
        builder = builder.with_html("<!DOCTYPE html><html><body><h1>WebAlph</h1></body></html>");
    }

    let webview = builder
        .build(&window)
        .map_err(|e| {
            let msg = e.to_string();
            let hint = if msg.contains("0x80070057") || msg.contains("Параметр задан неверно") {
                " (WebView2 invalid parameter - ensure URL or HTML file path is valid and accessible)"
            } else {
                ""
            };
            AlphaError::runtime(format!("Failed to build WebView: {}{}", msg, hint), span)
        })?;

    let on_msg_cb = interp.webview_on_message.clone();
    let pending_evals_rc = interp.webview_pending_evals.clone();

    // Drain any pending evals queued before run()
    {
        let mut queue = pending_evals_rc.borrow_mut();
        for code in queue.drain(..) {
            let _ = webview.evaluate_script(&code);
        }
    }

    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(msg) => {
                if let Some(cb) = &on_msg_cb {
                    match interp.call_function(cb, &[Value::String(msg)], span) {
                        Ok(Value::String(reply)) if !reply.is_empty() => {
                            let _ = webview.evaluate_script(&reply);
                        }
                        Err(e) => {
                            eprintln!("WebAlph error in on_message: {}", e.message);
                        }
                        _ => {}
                    }
                }

                // Check pending evals
                let mut queue = pending_evals_rc.borrow_mut();
                for code in queue.drain(..) {
                    let _ = webview.evaluate_script(&code);
                }
            }
            Event::MainEventsCleared => {
                let mut queue = pending_evals_rc.borrow_mut();
                if !queue.is_empty() {
                    for code in queue.drain(..) {
                        let _ = webview.evaluate_script(&code);
                    }
                }
            }
            _ => {}
        }
    });

    Ok(Value::Nothing)
}
