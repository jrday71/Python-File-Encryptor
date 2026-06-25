//! yabai-rs-menubar — native macOS menu-bar GUI for yabai-rs.
//!
//! Shows a status-bar icon with:
//!  • Workspace switcher (1–9, current one is checked)
//!  • Layout toggle (Tiles / Accordion)
//!  • Config editor window (NSTextView backed by yabai-rs.toml)
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
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel, YES, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    const WORKSPACE_COUNT: usize = 9;
    const CONFIG_PATH_SUFFIX: &str = ".config/yabai-rs/yabai-rs.toml";
    const SOCKET_PATH: &str = "/tmp/yabai-rs.sock";

    // ── Helpers ───────────────────────────────────────────────────────────────

    unsafe fn nsstring(s: &str) -> *mut Object {
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

    fn send_ipc(command: &str) {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
            let _ = stream.write_all(format!("{command}\n").as_bytes());
        }
    }

    fn config_path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        format!("{home}/{CONFIG_PATH_SUFFIX}")
    }

    // ── Delegate class ────────────────────────────────────────────────────────
    // We register a single Objective-C class "YabaiRsDelegate" that:
    //   • handles menu-item actions by reading representedObject (the IPC command)
    //   • implements applicationDidFinishLaunching: to set up the status bar
    //   • saves config text when the Save button is clicked

    static DELEGATE_CLASS: OnceLock<&'static objc::runtime::Class> = OnceLock::new();

    unsafe fn get_delegate_class() -> &'static objc::runtime::Class {
        DELEGATE_CLASS.get_or_init(|| {
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new("YabaiRsDelegate", superclass).unwrap();

            decl.add_method(
                sel!(menuItemClicked:),
                menu_item_clicked as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(saveConfig:),
                save_config as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(quitApp:),
                quit_app as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(quitDaemon:),
                quit_daemon as extern "C" fn(&Object, Sel, *mut Object),
            );
            // ivar to store a reference to the text view for config editing
            decl.add_ivar::<usize>("_textView");

            decl.register()
        })
    }

    extern "C" fn menu_item_clicked(this: &Object, _cmd: Sel, sender: *mut Object) {
        unsafe {
            let represented: *mut Object = msg_send![sender, representedObject];
            if represented.is_null() {
                return;
            }
            let bytes: *const c_char = msg_send![represented, UTF8String];
            if bytes.is_null() {
                return;
            }
            let cmd = std::ffi::CStr::from_ptr(bytes)
                .to_str()
                .unwrap_or("")
                .to_string();

            // "open-config" is a special pseudo-command handled locally
            if cmd == "open-config" {
                open_config_window(this as *const Object as *mut Object);
            } else {
                send_ipc(&cmd);
            }
        }
    }

    extern "C" fn save_config(this: &Object, _cmd: Sel, _sender: *mut Object) {
        unsafe {
            let tv_ptr: usize = *this.get_ivar("_textView");
            if tv_ptr == 0 {
                return;
            }
            let text_view = tv_ptr as *mut Object;
            let text_storage: *mut Object = msg_send![text_view, textStorage];
            let ns_str: *mut Object = msg_send![text_storage, string];
            let bytes: *const c_char = msg_send![ns_str, UTF8String];
            if bytes.is_null() {
                return;
            }
            let content = std::ffi::CStr::from_ptr(bytes)
                .to_str()
                .unwrap_or("")
                .to_string();

            let path = config_path();
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, content);
            send_ipc("reload");

            // Flash "Saved!" briefly via alert
            let alert: *mut Object = msg_send![class!(NSAlert), new];
            let title = nsstring("Saved");
            let msg = nsstring("Config saved and reload requested.");
            let _: () = msg_send![alert, setMessageText: title];
            let _: () = msg_send![alert, setInformativeText: msg];
            let _: () = msg_send![alert, runModal];
        }
    }

    extern "C" fn quit_app(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, terminate: std::ptr::null::<Object>()];
        }
    }

    extern "C" fn quit_daemon(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        send_ipc("quit");
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, terminate: std::ptr::null::<Object>()];
        }
    }

    // ── Config editor window ──────────────────────────────────────────────────

    unsafe fn open_config_window(delegate: *mut Object) {
        let path = config_path();
        let content = std::fs::read_to_string(&path).unwrap_or_default();

        // Window
        let frame = NSRect { x: 100.0, y: 100.0, width: 700.0, height: 500.0 };
        let style: usize = 1 | 2 | 4 | 8; // titled | closable | miniaturizable | resizable
        let win: *mut Object = msg_send![class!(NSWindow), alloc];
        let win: *mut Object = msg_send![win,
            initWithContentRect: frame
            styleMask: style
            backing: 2usize   // NSBackingStoreBuffered
            defer: NO
        ];
        let title = nsstring("yabai-rs — Config Editor");
        let _: () = msg_send![win, setTitle: title];
        let _: () = msg_send![win, setReleasedWhenClosed: NO];

        // ScrollView + TextView
        let scroll_frame = NSRect { x: 0.0, y: 40.0, width: 700.0, height: 460.0 };
        let scroll: *mut Object = msg_send![class!(NSScrollView), alloc];
        let scroll: *mut Object = msg_send![scroll, initWithFrame: scroll_frame];
        let _: () = msg_send![scroll, setAutohidesScrollers: YES];
        let _: () = msg_send![scroll, setHasVerticalScroller: YES];

        let text_frame = NSRect { x: 0.0, y: 0.0, width: 700.0, height: 10000.0 };
        let tv: *mut Object = msg_send![class!(NSTextView), alloc];
        let tv: *mut Object = msg_send![tv, initWithFrame: text_frame];
        let _: () = msg_send![tv, setAutoresizingMask: 2u64 | 16u64]; // width + height

        // Set monospace font
        let font_name = nsstring("Menlo");
        let font: *mut Object = msg_send![class!(NSFont), fontWithName:font_name size:13.0f64];
        if !font.is_null() {
            let _: () = msg_send![tv, setFont: font];
        }

        let text_ns = nsstring(&content);
        let _: () = msg_send![tv, setString: text_ns];

        let _: () = msg_send![scroll, setDocumentView: tv];

        // Save button
        let btn_frame = NSRect { x: 590.0, y: 8.0, width: 100.0, height: 26.0 };
        let btn: *mut Object = msg_send![class!(NSButton), alloc];
        let btn: *mut Object = msg_send![btn, initWithFrame: btn_frame];
        let btn_title = nsstring("Save & Reload");
        let _: () = msg_send![btn, setTitle: btn_title];
        let _: () = msg_send![btn, setBezelStyle: 1usize]; // NSBezelStyleRounded
        let _: () = msg_send![btn, setTarget: delegate];
        let _: () = msg_send![btn, setAction: sel!(saveConfig:)];

        // Store text view pointer in delegate ivar
        let tv_ptr = tv as usize;
        (*delegate).set_ivar("_textView", tv_ptr);

        // Add subviews to content view
        let content_view: *mut Object = msg_send![win, contentView];
        let _: () = msg_send![content_view, addSubview: scroll];
        let _: () = msg_send![content_view, addSubview: btn];

        let _: () = msg_send![win, makeKeyAndOrderFront: std::ptr::null::<Object>()];
    }

    // ── Menu construction ─────────────────────────────────────────────────────

    unsafe fn make_menu_item(
        title: &str,
        key: &str,
        command: &str,
        target: *mut Object,
    ) -> *mut Object {
        let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let t = nsstring(title);
        let k = nsstring(key);
        let item: *mut Object = msg_send![item,
            initWithTitle: t
            action: sel!(menuItemClicked:)
            keyEquivalent: k
        ];
        let repr = nsstring(command);
        let _: () = msg_send![item, setRepresentedObject: repr];
        let _: () = msg_send![item, setTarget: target];
        item
    }

    unsafe fn build_menu(delegate: *mut Object) -> *mut Object {
        let menu: *mut Object = msg_send![class!(NSMenu), new];
        let _: () = msg_send![menu, setAutoenablesItems: NO];

        // ── Workspaces ──────────────────────────────────────────────────────
        let header: *mut Object = msg_send![class!(NSMenuItem), new];
        let ht = nsstring("Workspaces");
        let _: () = msg_send![header, setTitle: ht];
        let _: () = msg_send![header, setEnabled: NO];
        let _: () = msg_send![menu, addItem: header];

        for i in 1..=WORKSPACE_COUNT {
            let title = format!("  Workspace {i}");
            let key = i.to_string();
            let cmd = format!("workspace {i}");
            let item = make_menu_item(&title, &key, &cmd, delegate);
            let _: () = msg_send![menu, addItem: item];
        }

        // ── Layout ─────────────────────────────────────────────────────────
        let sep1: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep1];

        let layout_hdr: *mut Object = msg_send![class!(NSMenuItem), new];
        let lht = nsstring("Layout");
        let _: () = msg_send![layout_hdr, setTitle: lht];
        let _: () = msg_send![layout_hdr, setEnabled: NO];
        let _: () = msg_send![menu, addItem: layout_hdr];

        let tiles = make_menu_item("  Tiles", "", "layout tiles", delegate);
        let _: () = msg_send![menu, addItem: tiles];

        let accordion = make_menu_item("  Accordion", "", "layout accordion", delegate);
        let _: () = msg_send![menu, addItem: accordion];

        // ── Config ─────────────────────────────────────────────────────────
        let sep2: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep2];

        let cfg = make_menu_item("Edit Config...", ",", "open-config", delegate);
        let _: () = msg_send![menu, addItem: cfg];

        let reload = make_menu_item("Reload Config", "r", "reload", delegate);
        let _: () = msg_send![menu, addItem: reload];

        // ── Quit ───────────────────────────────────────────────────────────
        let sep3: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep3];

        // "Quit Menu Bar" — stops this GUI process, daemon keeps running
        let quit_bar: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let qbt = nsstring("Quit Menu Bar");
        let qbk = nsstring("q");
        let quit_bar: *mut Object = msg_send![quit_bar,
            initWithTitle: qbt
            action: sel!(quitApp:)
            keyEquivalent: qbk
        ];
        let _: () = msg_send![quit_bar, setTarget: delegate];
        let _: () = msg_send![menu, addItem: quit_bar];

        // "Quit yabai-rs" — stops daemon + this GUI
        let quit_all: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let qat = nsstring("Quit yabai-rs");
        let qak = nsstring("Q");
        let quit_all: *mut Object = msg_send![quit_all,
            initWithTitle: qat
            action: sel!(quitDaemon:)
            keyEquivalent: qak
        ];
        let _: () = msg_send![quit_all, setTarget: delegate];
        let _: () = msg_send![menu, addItem: quit_all];

        menu
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct NSPoint { x: f64, y: f64 }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct NSSize { width: f64, height: f64 }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct NSRect { x: f64, y: f64, width: f64, height: f64 }

    pub unsafe fn run() {
        autorelease_pool(|| {
            // NSApplication (UIElement — no Dock icon)
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setActivationPolicy: 1i64]; // NSApplicationActivationPolicyAccessory

            // Register and instantiate our delegate
            let cls = get_delegate_class();
            let delegate: *mut Object = msg_send![cls, new];

            // Build the menu
            let menu = build_menu(delegate);

            // Create status bar item
            let status_bar: *mut Object = msg_send![class!(NSStatusBar), systemStatusBar];
            let item: *mut Object = msg_send![status_bar, statusItemWithLength: -1.0f64]; // NSVariableStatusItemLength

            // Set button title/image
            let button: *mut Object = msg_send![item, button];
            let title_str = nsstring("⬛ WM");
            let _: () = msg_send![button, setTitle: title_str];

            // Attach menu
            let _: () = msg_send![item, setMenu: menu];

            // Run the application event loop
            let _: () = msg_send![app, run];
        });
    }
}
