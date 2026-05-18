mod backend;
mod backends;
mod catalog;
mod commands;
mod downloader;
mod state;

use std::sync::{atomic::Ordering, OnceLock};
use tauri::{AppHandle, Manager, RunEvent};

use commands::inference::{cancel_inference, load_model, send_message, unload_model};
use commands::models::{
    cancel_download, download_hf_model, download_model, fetch_hf_quants, get_model_file_size,
    get_models_dir, list_catalog, list_installed, remove_model, reveal_models_dir,
};
use commands::sessions::{delete_session, get_session, list_sessions, save_session};
use state::AppState;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

fn leak_loaded_model_on_exit(state: &AppState) {
    if let Some(model) = state.model.lock().unwrap().take() {
        // On macOS Metal, dropping llama.cpp GPU-backed models during AppKit
        // termination can trip ggml's residency-set assert. This is shutdown-only,
        // so leak the model and let the OS reclaim its memory when the process exits.
        std::mem::forget(model);
    }
}

#[cfg(target_os = "macos")]
fn terminate_process_now() -> ! {
    unsafe extern "C" {
        fn _exit(status: i32) -> !;
    }

    unsafe { _exit(0) }
}

#[cfg(not(target_os = "macos"))]
fn terminate_process_now() -> ! {
    std::process::exit(0)
}

fn request_shutdown(app: AppHandle) {
    eprintln!("localagent: request_shutdown");
    let state = app.state::<AppState>();
    if state
        .shutdown_started
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        eprintln!("localagent: request_shutdown ignored; already shutting down");
        return;
    }

    if let Some(flag) = state.inference_cancel.lock().unwrap().clone() {
        eprintln!("localagent: canceling inference");
        flag.store(true, Ordering::Relaxed);
    }

    for window in app.webview_windows().values() {
        let _ = window.hide();
    }

    if *state.inference_running.lock().unwrap() {
        eprintln!("localagent: waiting for inference to stop");
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                if !*state.inference_running.lock().unwrap() {
                    break;
                }
            }
            leak_loaded_model_on_exit(&state);
            eprintln!("localagent: terminating process after inference stop");
            terminate_process_now();
        });
    } else {
        leak_loaded_model_on_exit(&state);
        eprintln!("localagent: terminating process immediately");
        terminate_process_now();
    }
}

#[cfg(target_os = "macos")]
fn install_macos_terminate_hook() {
    use objc2::ffi::class_replaceMethod;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::sel;
    use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};
    use objc2_foundation::MainThreadMarker;

    unsafe extern "C-unwind" fn intercept_should_terminate(
        _this: &AnyObject,
        _cmd: Sel,
        _sender: &NSApplication,
    ) -> NSApplicationTerminateReply {
        eprintln!("localagent: applicationShouldTerminate intercepted");
        if let Some(app) = APP_HANDLE.get() {
            request_shutdown(app.clone());
        } else {
            eprintln!("localagent: no AppHandle available during terminate intercept");
        }
        NSApplicationTerminateReply::TerminateCancel
    }

    let mtm = MainThreadMarker::new().expect("must install terminate hook on main thread");
    let ns_app = NSApplication::sharedApplication(mtm);
    let delegate = ns_app
        .delegate()
        .expect("NSApplication delegate must exist");
    let delegate_obj: &AnyObject = AsRef::<AnyObject>::as_ref(&*delegate);
    let cls = delegate_obj.class();

    eprintln!(
        "localagent: installing terminate hook on delegate class {}",
        cls.name().to_string_lossy()
    );

    let imp: objc2::runtime::Imp = unsafe { std::mem::transmute(intercept_should_terminate as *const ()) };
    let types = b"Q@:@\0";
    unsafe {
        class_replaceMethod(
            cls as *const _ as *mut _,
            sel!(applicationShouldTerminate:),
            imp,
            types.as_ptr().cast(),
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())

        .manage(AppState::new())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                request_shutdown(window.app_handle().clone());
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Catalog & model management
            list_catalog,
            list_installed,
            download_model,
            cancel_download,
            remove_model,
            get_model_file_size,
            get_models_dir,
            reveal_models_dir,
            fetch_hf_quants,
            download_hf_model,
            // Inference
            load_model,
            unload_model,
            send_message,
            cancel_inference,
            // Sessions
            list_sessions,
            get_session,
            save_session,
            delete_session,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    let _ = APP_HANDLE.set(app.handle().clone());
    #[cfg(target_os = "macos")]
    install_macos_terminate_hook();

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            eprintln!("localagent: RunEvent::ExitRequested");
            api.prevent_exit();
            request_shutdown(app_handle.clone());
        }
    });
}
