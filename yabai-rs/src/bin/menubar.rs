//! yabai-rs-menubar — native macOS menu-bar GUI for yabai-rs.
//!
//! Shows a status-bar icon with:
//!  • Workspace switcher (1–9)
//!  • Layout toggle (Tiles / Accordion)
//!  • Setup Wizard (multi-step guided config)
//!  • Raw config editor (NSTextView)
//!  • Reload / Quit controls
//!
//! Build:  cargo build --bin yabai-rs-menubar --release
//! Run:    ./target/release/yabai-rs-menubar &

fn main() {
    #[cfg(target_os = "macos")]
    unsafe {
        macos::run();
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("yabai-rs-menubar is macOS only");
        std::process::exit(1);
    }
}

// ─── macOS implementation ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    #[path = "../wizard_impl.rs"]
    mod wizard;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel, YES, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    const WORKSPACE_COUNT: usize = 9;
    const CONFIG_PATH_SUFFIX: &str = ".config/yabai-rs/yabai-rs.toml";
    const SOCKET_PATH: &str = "/tmp/yabai-rs.sock";

    // ── Helpers (also used by wizard submodule via super::) ───────────────────

    pub(super) unsafe fn nsstring(s: &str) -> *mut Object {
        let cls = class!(NSString);
        let obj: *mut Object = msg_send![cls, alloc];
        let bytes = s.as_ptr() as *const c_char;
        msg_send![obj,
            initWithBytes: bytes
            length: s.len()
            encoding: 4u64  // NSUTF8StringEncoding
        ]
    }

    unsafe fn autorelease_pool<F: FnOnce()>(f: F) {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        f();
        let _: () = msg_send![pool, drain];
    }

    pub(super) fn send_ipc(command: &str) {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
            let _ = stream.write_all(format!("{command}\n").as_bytes());
        }
    }

    pub(super) fn config_path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        format!("{home}/{CONFIG_PATH_SUFFIX}")
    }

    // ── Delegate class ────────────────────────────────────────────────────────

    static DELEGATE_CLASS: OnceLock<&'static objc::runtime::Class> = OnceLock::new();

    unsafe fn get_delegate_class() -> &'static objc::runtime::Class {
        DELEGATE_CLASS.get_or_init(|| {
            let mut decl = ClassDecl::new("YabaiRsDelegate", class!(NSObject)).unwrap();
            decl.add_method(sel!(menuItemClicked:), menu_item_clicked as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(saveConfig:),      save_config       as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(quitApp:),         quit_app          as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(quitDaemon:),      quit_daemon       as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_ivar::<usize>("_textView");
            decl.register()
        })
    }

    extern "C" fn menu_item_clicked(this: &Object, _: Sel, sender: *mut Object) {
        unsafe {
            let represented: *mut Object = msg_send![sender, representedObject];
            if represented.is_null() { return; }
            let bytes: *const c_char = msg_send![represented, UTF8String];
            if bytes.is_null() { return; }
            let cmd = std::ffi::CStr::from_ptr(bytes).to_str().unwrap_or("").to_string();

            match cmd.as_str() {
                "open-config"  => open_config_window(this as *const Object as *mut Object),
                "open-wizard"  => wizard::open_wizard(),
                other          => send_ipc(other),
            }
        }
    }

    extern "C" fn save_config(this: &Object, _: Sel, _: *mut Object) {
        unsafe {
            let tv_ptr: usize = *this.get_ivar("_textView");
            if tv_ptr == 0 { return; }
            let tv = tv_ptr as *mut Object;
            let ts: *mut Object = msg_send![tv, textStorage];
            let ns_str: *mut Object = msg_send![ts, string];
            let bytes: *const c_char = msg_send![ns_str, UTF8String];
            if bytes.is_null() { return; }
            let content = std::ffi::CStr::from_ptr(bytes).to_str().unwrap_or("").to_string();

            let path = config_path();
            if let Some(p) = std::path::Path::new(&path).parent() { let _ = std::fs::create_dir_all(p); }
            let _ = std::fs::write(&path, content);
            send_ipc("reload");

            let alert: *mut Object = msg_send![class!(NSAlert), new];
            let _: () = msg_send![alert, setMessageText:     nsstring("Saved")];
            let _: () = msg_send![alert, setInformativeText: nsstring("Config saved and reload requested.")];
            let _: () = msg_send![alert, runModal];
        }
    }

    extern "C" fn quit_app(_: &Object, _: Sel, _: *mut Object) {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, terminate: std::ptr::null::<Object>()];
        }
    }

    extern "C" fn quit_daemon(_: &Object, _: Sel, _: *mut Object) {
        send_ipc("quit");
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, terminate: std::ptr::null::<Object>()];
        }
    }

    // ── Config editor window ──────────────────────────────────────────────────

    #[repr(C)] #[derive(Copy, Clone)]
    struct NSRect { x: f64, y: f64, width: f64, height: f64 }

    unsafe fn open_config_window(delegate: *mut Object) {
        let content = std::fs::read_to_string(config_path()).unwrap_or_default();
        let style: usize = 1 | 2 | 4 | 8;
        let win: *mut Object = msg_send![class!(NSWindow), alloc];
        let win: *mut Object = msg_send![win,
            initWithContentRect: NSRect { x: 100.0, y: 100.0, width: 700.0, height: 500.0 }
            styleMask: style
            backing: 2usize
            defer: NO
        ];
        let _: () = msg_send![win, setTitle: nsstring("yabai-rs — Config Editor")];
        let _: () = msg_send![win, setReleasedWhenClosed: NO];

        let scroll: *mut Object = msg_send![class!(NSScrollView), alloc];
        let _: () = msg_send![scroll, initWithFrame: NSRect { x: 0.0, y: 40.0, width: 700.0, height: 460.0 }];
        let _: () = msg_send![scroll, setAutohidesScrollers: YES];
        let _: () = msg_send![scroll, setHasVerticalScroller: YES];

        let tv: *mut Object = msg_send![class!(NSTextView), alloc];
        let _: () = msg_send![tv, initWithFrame: NSRect { x: 0.0, y: 0.0, width: 700.0, height: 10000.0 }];
        let font: *mut Object = msg_send![class!(NSFont), fontWithName: nsstring("Menlo") size: 13.0f64];
        if !font.is_null() { let _: () = msg_send![tv, setFont: font]; }
        let _: () = msg_send![tv, setString: nsstring(&content)];
        let _: () = msg_send![scroll, setDocumentView: tv];

        let btn: *mut Object = msg_send![class!(NSButton), alloc];
        let _: () = msg_send![btn, initWithFrame: NSRect { x: 590.0, y: 8.0, width: 100.0, height: 26.0 }];
        let _: () = msg_send![btn, setTitle:      nsstring("Save & Reload")];
        let _: () = msg_send![btn, setBezelStyle: 1usize];
        let _: () = msg_send![btn, setTarget:     delegate];
        let _: () = msg_send![btn, setAction:     sel!(saveConfig:)];

        (*delegate).set_ivar("_textView", tv as usize);

        let cv: *mut Object = msg_send![win, contentView];
        let _: () = msg_send![cv, addSubview: scroll];
        let _: () = msg_send![cv, addSubview: btn];
        let _: () = msg_send![win, makeKeyAndOrderFront: std::ptr::null::<Object>()];
    }

    // ── Menu construction ─────────────────────────────────────────────────────

    unsafe fn menu_item(title: &str, key: &str, command: &str, target: *mut Object) -> *mut Object {
        let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let item: *mut Object = msg_send![item,
            initWithTitle: nsstring(title)
            action: sel!(menuItemClicked:)
            keyEquivalent: nsstring(key)
        ];
        let _: () = msg_send![item, setRepresentedObject: nsstring(command)];
        let _: () = msg_send![item, setTarget: target];
        item
    }

    unsafe fn separator() -> *mut Object {
        msg_send![class!(NSMenuItem), separatorItem]
    }

    unsafe fn header(title: &str) -> *mut Object {
        let item: *mut Object = msg_send![class!(NSMenuItem), new];
        let _: () = msg_send![item, setTitle:   nsstring(title)];
        let _: () = msg_send![item, setEnabled: NO];
        item
    }

    unsafe fn build_menu(delegate: *mut Object) -> *mut Object {
        let menu: *mut Object = msg_send![class!(NSMenu), new];
        let _: () = msg_send![menu, setAutoenablesItems: NO];

        // Workspaces
        let _: () = msg_send![menu, addItem: header("Workspaces")];
        for i in 1..=WORKSPACE_COUNT {
            let _: () = msg_send![menu, addItem:
                menu_item(&format!("  Workspace {i}"), &i.to_string(), &format!("workspace {i}"), delegate)
            ];
        }

        // Layout
        let _: () = msg_send![menu, addItem: separator()];
        let _: () = msg_send![menu, addItem: header("Layout")];
        let _: () = msg_send![menu, addItem: menu_item("  Tiles",     "", "layout tiles",     delegate)];
        let _: () = msg_send![menu, addItem: menu_item("  Accordion", "", "layout accordion", delegate)];

        // Setup & Config
        let _: () = msg_send![menu, addItem: separator()];
        let _: () = msg_send![menu, addItem: menu_item("Setup Wizard...", "w", "open-wizard",  delegate)];
        let _: () = msg_send![menu, addItem: menu_item("Edit Config...", ",",  "open-config",  delegate)];
        let _: () = msg_send![menu, addItem: menu_item("Reload Config",  "r",  "reload",       delegate)];

        // Quit
        let _: () = msg_send![menu, addItem: separator()];

        let quit_bar: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let quit_bar: *mut Object = msg_send![quit_bar,
            initWithTitle: nsstring("Quit Menu Bar")
            action: sel!(quitApp:)
            keyEquivalent: nsstring("q")
        ];
        let _: () = msg_send![quit_bar, setTarget: delegate];
        let _: () = msg_send![menu, addItem: quit_bar];

        let quit_all: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let quit_all: *mut Object = msg_send![quit_all,
            initWithTitle: nsstring("Quit yabai-rs")
            action: sel!(quitDaemon:)
            keyEquivalent: nsstring("Q")
        ];
        let _: () = msg_send![quit_all, setTarget: delegate];
        let _: () = msg_send![menu, addItem: quit_all];

        menu
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    pub unsafe fn run() {
        autorelease_pool(|| {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setActivationPolicy: 1i64]; // accessory — no Dock icon

            let cls      = get_delegate_class();
            let delegate: *mut Object = msg_send![cls, new];
            let menu     = build_menu(delegate);

            let status_bar: *mut Object = msg_send![class!(NSStatusBar), systemStatusBar];
            let item: *mut Object = msg_send![status_bar, statusItemWithLength: -1.0f64];
            let button: *mut Object = msg_send![item, button];
            let _: () = msg_send![button, setTitle: nsstring("⬛ WM")];
            let _: () = msg_send![item, setMenu: menu];

            let _: () = msg_send![app, run];
        });
    }
}
