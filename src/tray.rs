//! Native Windows tray, isolated from the async runtime on its own message thread.
use std::{
    io,
    ptr::null_mut,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::watch;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
use utilizer::{Result, web::App};
use windows_sys::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, KillTimer, MSG, PostThreadMessageW, SetTimer,
        TranslateMessage, WM_QUIT, WM_TIMER,
    },
};

pub struct Tray {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Tray {
    pub fn start(app: Arc<App>, stop: watch::Sender<bool>, code: String) -> Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("utilizer-tray".into())
            .spawn(move || {
                if let Err(error) = run(app, &stop, &ready, &code) {
                    if ready.send(Err(error.to_string())).is_err() {
                        eprintln!("System tray failed: {error}");
                    }
                    let _ = stop.send(true);
                }
            })?;
        match started.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                Err(io::Error::other(match result {
                    Ok(Err(error)) => format!("Cannot start system tray: {error}"),
                    Err(error) => format!("System tray stopped during startup: {error}"),
                    Ok(Ok(_)) => unreachable!(),
                })
                .into())
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            if !thread.is_finished() {
                // SAFETY: startup completed after creation of this tray thread's
                // queue. WM_QUIT has no pointers and only targets that queue.
                unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
            }
            if thread.join().is_err() {
                eprintln!("System tray thread stopped unexpectedly.");
            }
        }
    }
}

struct NativeTray {
    icon: TrayIcon,
    status: MenuItem,
    timer: usize,
    last_status: &'static str,
}

fn status_label(status: &str) -> &'static str {
    match status {
        "Watching console.log" | "Waiting for console.log" | "Waiting for console output" => {
            "Online"
        }
        "Capture reader retrying — check console" => "Retrying",
        _ => "Starting",
    }
}

fn login_label(requires_auth: bool, code: &str) -> String {
    if requires_auth {
        format!("Login code: {code}")
    } else {
        "Login not required".into()
    }
}

impl NativeTray {
    fn update_status(&mut self, app: &App) -> Result<()> {
        let status = match app.status.try_lock() {
            Ok(status) => status_label(&status),
            Err(_) => return Ok(()),
        };
        if status != self.last_status {
            self.status.set_text(format!("Status: {status}"));
            self.icon
                .set_tooltip(Some(format!("Utilizer — {status}")))?;
            self.last_status = status;
        }
        Ok(())
    }
}

impl Drop for NativeTray {
    fn drop(&mut self) {
        // SAFETY: this timer was created on, and is destroyed on, the tray thread.
        unsafe { KillTimer(null_mut(), self.timer) };
    }
}

fn run(
    app: Arc<App>,
    stop: &watch::Sender<bool>,
    ready: &mpsc::SyncSender<std::result::Result<u32, String>>,
    code: &str,
) -> Result<()> {
    let menu = Menu::new();
    let status = MenuItem::new("Status: Starting", false, None);
    let login = MenuItem::new(login_label(app.requires_auth(), code), false, None);
    let exit = MenuItem::new("Exit", true, None);
    menu.append_items(&[&status, &login, &exit])?;
    let icon = TrayIconBuilder::new()
        .with_tooltip("Utilizer")
        .with_icon(Icon::from_resource(1, None)?)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()?;
    // SAFETY: a thread timer posts WM_TIMER to this thread's native message queue;
    // no callback pointer is installed. It only refreshes status, never log reads.
    let timer = unsafe { SetTimer(null_mut(), 0, 1000, None) };
    if timer == 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut native = NativeTray {
        icon,
        status,
        timer,
        last_status: "",
    };
    let exit_id = exit.id().clone();
    native.update_status(&app)?;
    // SAFETY: the ID identifies this running thread, whose queue was created by
    // TrayIconBuilder and SetTimer before announcing readiness.
    ready.send(Ok(unsafe { GetCurrentThreadId() }))?;
    let stopped = stop.subscribe();
    let mut last_open: Option<Instant> = None;
    // SAFETY: MSG is plain Windows message storage and all pointers passed below
    // remain valid for each call. The loop and tray objects share the same thread.
    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: message is valid writable storage, and a null HWND reads only
        // this thread's queue, including its hidden tray window's messages.
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result == -1 {
            return Err(io::Error::last_os_error().into());
        }
        if result == 0 || *stopped.borrow() {
            break;
        }
        if message.hwnd.is_null() && message.message == WM_TIMER && message.wParam == timer {
            native.update_status(&app)?;
        } else {
            // SAFETY: GetMessageW populated message, and dispatch stays on the
            // same thread that owns the tray window and menu.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let mut open = false;
        for event in TrayIconEvent::receiver().try_iter() {
            open |= matches!(event, TrayIconEvent::Click {
                id, button: MouseButton::Left, button_state: MouseButtonState::Up, ..
            } if id == native.icon.id());
        }
        if open && last_open.is_none_or(|last| last.elapsed() >= Duration::from_secs(1)) {
            last_open = Some(Instant::now());
            if let Err(error) = crate::pwa::open() {
                eprintln!("Cannot open Utilizer window: {error}");
            }
        }
        // Dispatching a native menu command queues its MenuEvent synchronously.
        if MenuEvent::receiver()
            .try_iter()
            .any(|event| event.id == exit_id)
        {
            let _ = stop.send(true);
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{PM_NOREMOVE, PeekMessageW};

    #[test]
    fn login_menu_preserves_leading_zeroes_and_hides_unused_preview_code() {
        assert_eq!(login_label(true, "0123"), "Login code: 0123");
        assert_eq!(login_label(false, "0123"), "Login not required");
    }

    #[test]
    fn status_uses_plain_labels_without_hiding_retrying() {
        for status in [
            "Watching console.log",
            "Waiting for console.log",
            "Waiting for console output",
        ] {
            assert_eq!(status_label(status), "Online");
        }
        assert_eq!(
            status_label("Capture reader retrying — check console"),
            "Retrying"
        );
        assert_eq!(status_label(""), "Starting");
    }

    #[test]
    fn dropping_tray_stops_and_joins_its_native_message_thread() {
        let (ready, started) = mpsc::sync_channel(1);
        let (finished, done) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            // SAFETY: native queue storage is valid on this test-owned thread.
            // PeekMessageW creates its queue without opening any window or icon.
            let mut message: MSG = unsafe { std::mem::zeroed() };
            unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) };
            ready.send(unsafe { GetCurrentThreadId() }).unwrap();
            assert_eq!(unsafe { GetMessageW(&mut message, null_mut(), 0, 0) }, 0);
            assert_eq!(message.message, WM_QUIT);
            finished.send(()).unwrap();
        });
        let tray = Tray {
            thread_id: started.recv_timeout(Duration::from_secs(5)).unwrap(),
            thread: Some(thread),
        };
        let cleanup = thread::spawn(move || drop(tray));
        done.recv_timeout(Duration::from_secs(5)).unwrap();
        cleanup.join().unwrap();
    }
}
