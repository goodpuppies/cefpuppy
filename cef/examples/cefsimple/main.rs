use cef::{args::Args, rc::*, sandbox_info::SandboxInfo, *};
use std::sync::{Arc, Mutex};
use std::io::Write;
use lazy_static::lazy_static;
use named_pipe::PipeClient;
use std::time::{Instant, Duration};

lazy_static! {
    static ref PIPE_CONN: Mutex<Option<PipeClient>> = Mutex::new(None);
}

static mut LAST_FPS_LOG: Option<Instant> = None;
static mut FRAME_COUNT: u32 = 0;

fn log_fps() {
    unsafe {
        let now = Instant::now();
        if let Some(last) = LAST_FPS_LOG {
            FRAME_COUNT += 1;
            let elapsed = now.duration_since(last);
            if elapsed >= Duration::from_secs(1) {
                println!("[Rust] FPS: {}", FRAME_COUNT);
                FRAME_COUNT = 0;
                LAST_FPS_LOG = Some(now);
            }
        } else {
            LAST_FPS_LOG = Some(now);
            FRAME_COUNT = 1;
        }
    }
}

fn send_frame_over_pipe(width: i32, height: i32, buffer: &[u8]) {
    let mut conn_guard = PIPE_CONN.lock().unwrap();
    if conn_guard.is_none() {
        match PipeClient::connect(r"\\.\pipe\your-own-name") {
            Ok(pipe) => {
                *conn_guard = Some(pipe);
            }
            Err(_e) => {
                //eprintln!("[Rust] Failed to connect to named pipe: {}", e);
                return;
            }
        }
    }
    if let Some(ref mut pipe) = *conn_guard {
        let total_size = buffer.len() as u32;
        let width = width as u32;
        let height = height as u32;
        let num_chunks = 1u32;
        let chunk_size = total_size;
        let mut header = Vec::with_capacity(16);
        header.extend(&width.to_le_bytes());
        header.extend(&height.to_le_bytes());
        header.extend(&total_size.to_le_bytes());
        header.extend(&num_chunks.to_le_bytes());
        if let Err(e) = pipe.write_all(&header) {
            eprintln!("[Rust] Failed to send header: {}", e);
            *conn_guard = None;
            return;
        }
        if let Err(e) = pipe.write_all(&chunk_size.to_le_bytes()) {
            eprintln!("[Rust] Failed to send chunk size: {}", e);
            *conn_guard = None;
            return;
        }
        if let Err(e) = pipe.write_all(buffer) {
            eprintln!("[Rust] Failed to send frame data: {}", e);
            *conn_guard = None;
            return;
        }
    }
}

struct DemoApp {
    object: *mut RcImpl<cef_dll_sys::_cef_app_t, Self>,
    window: Arc<Mutex<Option<Window>>>,
}

impl DemoApp {
    fn new(window: Arc<Mutex<Option<Window>>>) -> App {
        App::new(Self {
            object: std::ptr::null_mut(),
            window,
        })
    }
}

impl WrapApp for DemoApp {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_app_t, Self>) {
        self.object = object;
    }
}

impl Clone for DemoApp {
    fn clone(&self) -> Self {
        let object = unsafe {
            let rc_impl = &mut *self.object;
            rc_impl.interface.add_ref();
            self.object
        };
        let window = self.window.clone();

        Self { object, window }
    }
}

impl Rc for DemoApp {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.object;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl ImplApp for DemoApp {
    fn get_raw(&self) -> *mut cef_dll_sys::_cef_app_t {
        self.object.cast()
    }

    fn get_browser_process_handler(&self) -> Option<BrowserProcessHandler> {
        Some(DemoBrowserProcessHandler::new(self.window.clone()))
    }
}

struct DemoBrowserProcessHandler {
    object: *mut RcImpl<cef_dll_sys::cef_browser_process_handler_t, Self>,
    window: Arc<Mutex<Option<Window>>>,
}

impl DemoBrowserProcessHandler {
    fn new(window: Arc<Mutex<Option<Window>>>) -> BrowserProcessHandler {
        BrowserProcessHandler::new(Self {
            object: std::ptr::null_mut(),
            window,
        })
    }
}

impl Rc for DemoBrowserProcessHandler {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.object;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl WrapBrowserProcessHandler for DemoBrowserProcessHandler {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_browser_process_handler_t, Self>) {
        self.object = object;
    }
}

impl Clone for DemoBrowserProcessHandler {
    fn clone(&self) -> Self {
        let object = unsafe {
            let rc_impl = &mut *self.object;
            rc_impl.interface.add_ref();
            rc_impl
        };

        let window = self.window.clone();

        Self { object, window }
    }
}

impl ImplBrowserProcessHandler for DemoBrowserProcessHandler {
    fn get_raw(&self) -> *mut cef_dll_sys::_cef_browser_process_handler_t {
        self.object.cast()
    }

    // The real lifespan of cef starts from `on_context_initialized`, so all the cef objects should be manipulated after that.
    fn on_context_initialized(&self) {
        println!("cef context intiialized");

        // --- Windowless/Off-screen Rendering ---
        let mut window_info = WindowInfo::default();
        window_info.windowless_rendering_enabled = 1;
        // Do NOT set parent_window or window handle for off-screen renderin

        // Prepare the client (with your DemoRenderHandler etc)
        let mut client = DemoClient::new();

        // Prepare browser settings
        let mut browser_settings = BrowserSettings::default();
        browser_settings.windowless_frame_rate = 900000; // Set high FPS for off-screen rendering

        // Optionally, set up request context and extra_info if needed, else None
        let request_context: Option<&mut RequestContext> = None;
        let extra_info: Option<&mut DictionaryValue> = None;

        // Create the browser in windowless mode
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from("http://127.0.0.1:5173/index.html")),
            Some(&browser_settings),
            extra_info,
            request_context,
        );
        if browser.is_none() {
            panic!("Failed to create windowless browser");
        }
    }
}

use cef::{ImplBrowser, RenderHandler, WrapRenderHandler, ImplRenderHandler, PaintElementType, Rect};
use cef::{DisplayHandler, WrapDisplayHandler, ImplDisplayHandler, LogSeverity}; // Added DisplayHandler related imports

#[derive(Clone)]
struct DemoRenderHandler {
    object: *mut RcImpl<cef_dll_sys::_cef_render_handler_t, Self>,
}

impl DemoRenderHandler {
    fn new() -> RenderHandler {
        RenderHandler::new(Self {
            object: std::ptr::null_mut(),
        })
    }
}

impl Rc for DemoRenderHandler {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.object;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl WrapRenderHandler for DemoRenderHandler {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_render_handler_t, Self>) {
        self.object = object;
    }
}

fn process_and_flip_buffer(width: i32, height: i32, buffer: &[u8]) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    let stride = width * 4;
    let mut flipped = vec![0u8; buffer.len()];

    for y in 0..height {
        let src_row = &buffer[y * stride..(y + 1) * stride];
        let dst_row = &mut flipped[(height - 1 - y) * stride..(height - y) * stride];
        for x in 0..width {
            let src_idx = x * 4;
            let dst_idx = src_idx;
            let b = src_row[src_idx];
            let g = src_row[src_idx + 1];
            let r = src_row[src_idx + 2];
            let a = src_row[src_idx + 3];
            // If black pixel (R=G=B=0), set alpha to 0
            let new_a = if r == 0 && g == 0 && b == 0 { 0 } else { a };
            dst_row[dst_idx] = b;
            dst_row[dst_idx + 1] = g;
            dst_row[dst_idx + 2] = r;
            dst_row[dst_idx + 3] = new_a;
        }
    }
    flipped
}

impl ImplRenderHandler for DemoRenderHandler {
    fn get_raw(&self) -> *mut cef_dll_sys::_cef_render_handler_t {
        self.object.cast()
    }

    fn get_view_rect(
        &self,
        _browser: Option<&mut impl ImplBrowser>,
        rect: Option<&mut Rect>,
    ) {
        if let Some(rect) = rect {
            rect.x = 0;
            rect.y = 0;
            rect.width = 1200;
            rect.height = 600;
        }
    }

    fn on_paint(
        &self,
        _browser: Option<&mut impl ImplBrowser>,
        _type_: PaintElementType,
        _dirty_rects_count: usize,
        _dirty_rects: Option<&Rect>,
        buffer: *const u8,
        width: ::std::os::raw::c_int,
        height: ::std::os::raw::c_int,
    ) {
        let buffer_len = (width * height * 4) as usize;
        if buffer.is_null() || buffer_len == 0 {
            eprintln!("[Rust] OnPaint: buffer is null or size is zero");
            return;
        }
        let pixel_data = unsafe { std::slice::from_raw_parts(buffer, buffer_len) };
        let processed = process_and_flip_buffer(width, height, pixel_data);
        send_frame_over_pipe(width, height, &processed);
        log_fps();
    }
}

// +++ Add DemoDisplayHandler struct and implementations +++
#[derive(Clone)]
struct DemoDisplayHandler {
    object: *mut RcImpl<cef_dll_sys::_cef_display_handler_t, Self>,
}

impl DemoDisplayHandler {
    fn new() -> DisplayHandler {
        DisplayHandler::new(Self {
            object: std::ptr::null_mut(),
        })
    }
}

impl Rc for DemoDisplayHandler {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.object;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl WrapDisplayHandler for DemoDisplayHandler {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_display_handler_t, Self>) {
        self.object = object;
    }
}

impl ImplDisplayHandler for DemoDisplayHandler {
    fn get_raw(&self) -> *mut cef_dll_sys::_cef_display_handler_t {
        self.object.cast()
    }

    fn on_console_message(
        &self,
        _browser: Option<&mut impl ImplBrowser>,
        _level: LogSeverity,
        message: Option<&CefString>,
        source: Option<&CefString>,
        line: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        if let (Some(msg), Some(src)) = (message, source) {
            println!("[Browser Console] [{}:{}] {}", src, line, msg);
        } else if let Some(msg) = message {
            println!("[Browser Console] {}", msg);
        }
        // Return 0 to allow the default handling, or 1 to suppress it.
        0
    }
}
// --- End of DemoDisplayHandler ---

struct DemoClient(*mut RcImpl<cef_dll_sys::_cef_client_t, Self>);

impl DemoClient {
    fn new() -> Client {
        Client::new(Self(std::ptr::null_mut()))
    }
}

impl WrapClient for DemoClient {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_client_t, Self>) {
        self.0 = object;
    }
}

impl Clone for DemoClient {
    fn clone(&self) -> Self {
        unsafe {
            let rc_impl = &mut *self.0;
            rc_impl.interface.add_ref();
        }

        Self(self.0)
    }
}

impl Rc for DemoClient {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.0;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl ImplClient for DemoClient {
    fn get_raw(&self) -> *mut cef_dll_sys::_cef_client_t {
        self.0.cast()
    }

    fn get_render_handler(&self) -> Option<RenderHandler> {
        Some(DemoRenderHandler::new())
    }

    // +++ Add get_display_handler +++
    fn get_display_handler(&self) -> Option<DisplayHandler> {
        Some(DemoDisplayHandler::new())
    }

    // --- Remove on_console_message from here ---
    /*
    fn on_console_message(
        &self,
        _browser: Option<&mut impl ImplBrowser>,
        _level: LogSeverity,
        message: Option<&CefString>,
        source: Option<&CefString>,
        line: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        if let (Some(msg), Some(src)) = (message, source) {
            println!("[Browser Console] [{}:{}] {}", src, line, msg);
        } else if let Some(msg) = message {
            println!("[Browser Console] {}", msg);
        }
        // Return 0 to allow the default handling, or 1 to suppress it.
        0
    }
    */
}

struct _DemoWindowDelegate {
    base: *mut RcImpl<cef_dll_sys::_cef_window_delegate_t, Self>,
    browser_view: BrowserView,
}

impl _DemoWindowDelegate {
    fn _new(browser_view: BrowserView) -> WindowDelegate {
        WindowDelegate::new(Self {
            base: std::ptr::null_mut(),
            browser_view,
        })
    }
}

impl WrapWindowDelegate for _DemoWindowDelegate {
    fn wrap_rc(&mut self, object: *mut RcImpl<cef_dll_sys::_cef_window_delegate_t, Self>) {
        self.base = object;
    }
}

impl Clone for _DemoWindowDelegate {
    fn clone(&self) -> Self {
        unsafe {
            let rc_impl = &mut *self.base;
            rc_impl.interface.add_ref();
        }

        Self {
            base: self.base,
            browser_view: self.browser_view.clone(),
        }
    }
}

impl Rc for _DemoWindowDelegate {
    fn as_base(&self) -> &cef_dll_sys::cef_base_ref_counted_t {
        unsafe {
            let base = &*self.base;
            std::mem::transmute(&base.cef_object)
        }
    }
}

impl ImplViewDelegate for _DemoWindowDelegate {
    fn on_child_view_changed(
        &self,
        _view: Option<&mut impl ImplView>,
        _added: ::std::os::raw::c_int,
        _child: Option<&mut impl ImplView>,
    ) {
        // view.as_panel().map(|x| x.as_window().map(|w| w.close()));
    }

    fn get_raw(&self) -> *mut cef_dll_sys::_cef_view_delegate_t {
        self.base.cast()
    }
}

impl ImplPanelDelegate for _DemoWindowDelegate {}

impl ImplWindowDelegate for _DemoWindowDelegate {
    fn on_window_created(&self, window: Option<&mut impl ImplWindow>) {
        if let Some(window) = window {
            let mut view = self.browser_view.clone();
            window.add_child_view(Some(&mut view));
            window.show();
        }
    }

    fn on_window_destroyed(&self, _window: Option<&mut impl ImplWindow>) {
        quit_message_loop();
    }

    fn with_standard_window_buttons(
        &self,
        _window: Option<&mut impl ImplWindow>,
    ) -> ::std::os::raw::c_int {
        1
    }

    fn can_resize(&self, _window: Option<&mut impl ImplWindow>) -> ::std::os::raw::c_int {
        1
    }

    fn can_maximize(&self, _window: Option<&mut impl ImplWindow>) -> ::std::os::raw::c_int {
        1
    }

    fn can_minimize(&self, _window: Option<&mut impl ImplWindow>) -> ::std::os::raw::c_int {
        1
    }

    fn can_close(&self, _window: Option<&mut impl ImplWindow>) -> ::std::os::raw::c_int {
        1
    }
}

// FIXME: Rewrite this demo based on cef/tests/cefsimple
fn main() {
    #[cfg(target_os = "macos")]
    let _loader = {
        let loader = library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), false);
        assert!(loader.load());
        loader
    };

    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    let args = Args::new();
    let cmd = args.as_cmd_line().unwrap();

    let sandbox = SandboxInfo::new();

    let switch = CefString::from("type");
    let is_browser_process = cmd.has_switch(Some(&switch)) != 1;

    let window = Arc::new(Mutex::new(None));
    let mut app = DemoApp::new(window.clone());

    let ret = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        sandbox.as_mut_ptr(),
    );

    if is_browser_process {
        println!("launch browser process");
        assert!(ret == -1, "cannot execute browser process");
    } else {
        let process_type = CefString::from(&cmd.get_switch_value(Some(&switch)));
        println!("launch process {process_type}");
        assert!(ret >= 0, "cannot execute non-browser process");
        // non-browser process does not initialize cef
        return;
    }
    let mut settings = Settings::default();
    settings.windowless_rendering_enabled = 1;
    settings.no_sandbox = 1;

    assert_eq!(
        initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            sandbox.as_mut_ptr()
        ),
        1
    );

    run_message_loop();

    // No window to check in windowless mode
    shutdown();
}
