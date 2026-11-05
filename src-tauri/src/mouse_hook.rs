use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_KEYDOWN, WM_MOUSEWHEEL,
};

static mut MOUSE_HOOK: Option<HHOOK> = None;
static mut KEYBOARD_HOOK: Option<HHOOK> = None;

static IS_SCROLL_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static IS_OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);

static mut SCROLL_CALLBACK: Option<Box<dyn Fn() + Send + Sync>> = None;
static mut ESCAPE_CALLBACK: Option<Box<dyn Fn() + Send + Sync>> = None;

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

unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && w_param.0 as u32 == WM_KEYDOWN {
        let kbd = unsafe { *(l_param.0 as *const KBDLLHOOKSTRUCT) };
        if kbd.vkCode == 0x1B { // VK_ESCAPE
            if IS_OVERLAY_VISIBLE.load(Ordering::Relaxed) {
                unsafe {
                    if let Some(ref cb) = ESCAPE_CALLBACK {
                        cb();
                    }
                }
                // Consume the Escape event so it closes our overlay without side-effects on underlying window
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

pub struct GlobalInputMonitor {
    _is_running: Arc<AtomicBool>,
}

impl GlobalInputMonitor {
    pub fn new<FStart, FEnd, FEsc>(
        on_scroll_start: FStart,
        on_scroll_debounced: FEnd,
        on_escape_pressed: FEsc,
    ) -> Self
    where
        FStart: Fn() + Send + Sync + 'static,
        FEnd: Fn() + Send + Sync + 'static,
        FEsc: Fn() + Send + Sync + 'static,
    {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        let (tx, rx) = std::sync::mpsc::channel::<()>();

        unsafe {
            SCROLL_CALLBACK = Some(Box::new(move || {
                let _ = tx.send(());
            }));
            ESCAPE_CALLBACK = Some(Box::new(move || {
                on_escape_pressed();
            }));
        }

        // Spawn debouncing worker thread
        std::thread::spawn(move || {
            let debounce_time = Duration::from_millis(280);
            while is_running_clone.load(Ordering::Relaxed) {
                if let Ok(()) = rx.recv() {
                    on_scroll_start();
                    while let Ok(()) = rx.recv_timeout(debounce_time) {}
                    on_scroll_debounced();
                }
            }
        });

        // Spawn Windows Hooks thread (handles both mouse wheel and escape key)
        std::thread::spawn(move || unsafe {
            let m_hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_proc),
                None,
                0,
            );
            if let Ok(h) = m_hook {
                MOUSE_HOOK = Some(h);
            }

            let k_hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                None,
                0,
            );
            if let Ok(h) = k_hook {
                KEYBOARD_HOOK = Some(h);
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if let Some(h) = MOUSE_HOOK {
                let _ = UnhookWindowsHookEx(h);
            }
            if let Some(h) = KEYBOARD_HOOK {
                let _ = UnhookWindowsHookEx(h);
            }
        });

        Self { _is_running: is_running }
    }

    pub fn set_scroll_active(&self, active: bool) {
        IS_SCROLL_MODE_ACTIVE.store(active, Ordering::SeqCst);
    }

    pub fn set_overlay_visible(&self, visible: bool) {
        IS_OVERLAY_VISIBLE.store(visible, Ordering::SeqCst);
    }

    pub fn is_scroll_active(&self) -> bool {
        IS_SCROLL_MODE_ACTIVE.load(Ordering::SeqCst)
    }

    pub fn is_overlay_visible(&self) -> bool {
        IS_OVERLAY_VISIBLE.load(Ordering::SeqCst)
    }
}
