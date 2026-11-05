use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, MSG, WH_MOUSE_LL, WM_MOUSEWHEEL,
};

static mut MOUSE_HOOK: Option<HHOOK> = None;
static IS_SCROLL_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut SCROLL_CALLBACK: Option<Box<dyn Fn() + Send + Sync>> = None;

unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL {
        if IS_SCROLL_MODE_ACTIVE.load(Ordering::Relaxed) {
            unsafe {
                if let Some(ref cb) = SCROLL_CALLBACK {
                    cb();
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

pub struct MouseWheelMonitor {
    _is_running: Arc<AtomicBool>,
}

impl MouseWheelMonitor {
    pub fn new<F>(on_scroll_debounced: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        // Channel for notifying debounce thread
        let (tx, rx) = std::sync::mpsc::channel::<()>();

        // Set global callback
        unsafe {
            SCROLL_CALLBACK = Some(Box::new(move || {
                let _ = tx.send(());
            }));
        }

        // Spawn debouncing worker thread
        std::thread::spawn(move || {
            let debounce_time = Duration::from_millis(260);
            while is_running_clone.load(Ordering::Relaxed) {
                if let Ok(()) = rx.recv() {
                    // Drain any subsequent wheel ticks during continuous scroll
                    while let Ok(()) = rx.recv_timeout(debounce_time) {}
                    // Once scrolling paused for 260ms, fire the callback!
                    on_scroll_debounced();
                }
            }
        });

        // Spawn Windows Hook thread (requires a message loop)
        std::thread::spawn(move || unsafe {
            let hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_proc),
                None,
                0,
            );

            if let Ok(h) = hook {
                MOUSE_HOOK = Some(h);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if let Some(h) = MOUSE_HOOK {
                    let _ = UnhookWindowsHookEx(h);
                }
            }
        });

        Self { _is_running: is_running }
    }

    pub fn set_active(&self, active: bool) {
        IS_SCROLL_MODE_ACTIVE.store(active, Ordering::SeqCst);
    }

    pub fn is_active(&self) -> bool {
        IS_SCROLL_MODE_ACTIVE.load(Ordering::SeqCst)
    }
}
