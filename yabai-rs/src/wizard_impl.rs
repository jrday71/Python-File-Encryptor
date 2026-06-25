//! Setup wizard for yabai-rs — 7-step NSWindow guiding first-time configuration.
//!
//! Steps:
//!   0  Welcome
//!   1  Gaps         (sliders)
//!   2  Layout       (radio buttons)
//!   3  Workspaces   (name text fields)
//!   4  Modifier key (radio buttons)
//!   5  Float apps   (text fields)
//!   6  Review       (generated TOML preview + Finish)
//!
//! This file is only compiled on macOS (included from menubar.rs which is
//! guarded by #[cfg(target_os = "macos")]).

use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, YES, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::os::raw::c_char;
use std::sync::{Mutex, OnceLock};

// ─── Geometry ────────────────────────────────────────────────────────────────

#[repr(C)] #[derive(Copy, Clone)] struct NSRect { x: f64, y: f64, width: f64, height: f64 }

fn r(x: f64, y: f64, w: f64, h: f64) -> NSRect { NSRect { x, y, width: w, height: h } }

// ─── Window dimensions ────────────────────────────────────────────────────────

const WIN_W: f64 = 600.0;
const WIN_H: f64 = 480.0;
const STEP_COUNT: usize = 7;

// Tab view sits between the header (50px) and footer (50px).
const TAB_Y:   f64 = 51.0;
const TAB_H:   f64 = WIN_H - 102.0;  // 378

// ─── Control tags ─────────────────────────────────────────────────────────────
// Value-label for a slider lives at tag = slider_tag + 50.

const TAG_INNER_H:      i64 = 101;
const TAG_INNER_V:      i64 = 102;
const TAG_OUTER:        i64 = 103;
const TAG_ACC_PAD:      i64 = 104;
const TAG_LAYOUT_TILES: i64 = 201;
const TAG_LAYOUT_ACC:   i64 = 202;
const TAG_WS_BASE:      i64 = 300;  // 300-308 → workspace names 0-8
const TAG_MOD_ALT:      i64 = 401;
const TAG_MOD_CMD:      i64 = 402;
const TAG_MOD_CTRL:     i64 = 403;
const TAG_MOD_PREVIEW:  i64 = 450;
const TAG_FLOAT_BASE:   i64 = 500;  // 500-504 → float-app names 0-4
const TAG_REVIEW_TV:    i64 = 601;
const TAG_STEP_LABEL:   i64 = 701;
const TAG_NEXT_BTN:     i64 = 801;
const TAG_BACK_BTN:     i64 = 802;

// ─── Wizard state ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WizardData {
    pub inner_h:          i32,
    pub inner_v:          i32,
    pub outer_all:        i32,
    pub layout_tiles:     bool,
    pub accordion_pad:    i32,
    pub workspace_names:  Vec<String>,
    pub modifier:         String,   // "alt" | "cmd" | "ctrl"
    pub float_apps:       Vec<String>,
}

impl Default for WizardData {
    fn default() -> Self {
        Self {
            inner_h:         10,
            inner_v:         10,
            outer_all:       10,
            layout_tiles:    true,
            accordion_pad:   30,
            workspace_names: (1..=9).map(|i| i.to_string()).collect(),
            modifier:        "alt".into(),
            float_apps:      vec![
                "System Preferences".into(),
                "Calculator".into(),
                "Finder".into(),
                String::new(),
                String::new(),
            ],
        }
    }
}

static WIZARD_STATE: OnceLock<Mutex<WizardData>> = OnceLock::new();

fn state() -> std::sync::MutexGuard<'static, WizardData> {
    WIZARD_STATE
        .get_or_init(|| Mutex::new(WizardData::default()))
        .lock()
        .unwrap()
}

// ─── Delegate class ───────────────────────────────────────────────────────────

static WIZARD_CLASS: OnceLock<&'static objc::runtime::Class> = OnceLock::new();

unsafe fn get_wizard_class() -> &'static objc::runtime::Class {
    WIZARD_CLASS.get_or_init(|| {
        let mut decl = ClassDecl::new("YabaiRsWizard", class!(NSObject)).unwrap();

        decl.add_method(sel!(wizardNext:),     wizard_next     as extern "C" fn(&mut Object, Sel, *mut Object));
        decl.add_method(sel!(wizardBack:),     wizard_back     as extern "C" fn(&mut Object, Sel, *mut Object));
        decl.add_method(sel!(sliderChanged:),  slider_changed  as extern "C" fn(&Object,     Sel, *mut Object));
        decl.add_method(sel!(modifierPicked:), modifier_picked as extern "C" fn(&Object,     Sel, *mut Object));

        decl.add_ivar::<usize>("_step");
        decl.add_ivar::<usize>("_window");
        decl.add_ivar::<usize>("_tab_view");

        decl.register()
    })
}

// ─── Action handlers ──────────────────────────────────────────────────────────

extern "C" fn wizard_next(this: &mut Object, _: Sel, _: *mut Object) {
    unsafe {
        let step: usize = *this.get_ivar("_step");
        let tab_view = *this.get_ivar::<usize>("_tab_view") as *mut Object;
        let current_item: *mut Object = msg_send![tab_view, selectedTabViewItem];
        let page: *mut Object = msg_send![current_item, view];
        collect_page(step, page);

        if step + 1 >= STEP_COUNT {
            finish_wizard(this);
        } else {
            let next = step + 1;
            this.set_ivar("_step", next);
            let _: () = msg_send![tab_view, selectTabViewItemAtIndex: next as i64];
            if next == STEP_COUNT - 1 {
                update_review(tab_view);
            }
            refresh_nav(this, next);
        }
    }
}

extern "C" fn wizard_back(this: &mut Object, _: Sel, _: *mut Object) {
    unsafe {
        let step: usize = *this.get_ivar("_step");
        if step == 0 { return; }
        let tab_view = *this.get_ivar::<usize>("_tab_view") as *mut Object;
        let current_item: *mut Object = msg_send![tab_view, selectedTabViewItem];
        let page: *mut Object = msg_send![current_item, view];
        collect_page(step, page);

        let prev = step - 1;
        this.set_ivar("_step", prev);
        let _: () = msg_send![tab_view, selectTabViewItemAtIndex: prev as i64];
        refresh_nav(this, prev);
    }
}

extern "C" fn slider_changed(_: &Object, _: Sel, sender: *mut Object) {
    unsafe {
        let val: f64 = msg_send![sender, doubleValue];
        let tag: i64 = msg_send![sender, tag];
        let superview: *mut Object = msg_send![sender, superview];
        let label: *mut Object = msg_send![superview, viewWithTag: tag + 50];
        if !label.is_null() {
            let s = nsstring(&format!("{} px", val as i32));
            let _: () = msg_send![label, setStringValue: s];
        }
    }
}

extern "C" fn modifier_picked(_: &Object, _: Sel, sender: *mut Object) {
    unsafe {
        let tag: i64  = msg_send![sender, tag];
        let mod_str   = match tag {
            TAG_MOD_ALT  => "alt",
            TAG_MOD_CMD  => "cmd",
            TAG_MOD_CTRL => "ctrl",
            _            => "alt",
        };
        // Update the preview label
        let superview: *mut Object = msg_send![sender, superview];
        let preview: *mut Object   = msg_send![superview, viewWithTag: TAG_MOD_PREVIEW];
        if !preview.is_null() {
            let sym = match mod_str { "cmd" => "⌘", "ctrl" => "⌃", _ => "⌥" };
            let text = format!(
                "{sym}+h/j/k/l  →  focus left/down/up/right\n\
                 {sym}+Shift+h/j/k/l  →  move window\n\
                 {sym}+1..9  →  switch workspace"
            );
            let _: () = msg_send![preview, setStringValue: nsstring(&text)];
        }
    }
}

// ─── Collect page values into WizardData ─────────────────────────────────────

unsafe fn collect_page(step: usize, page: *mut Object) {
    let mut s = state();
    match step {
        1 => {
            s.inner_h      = slider_val(page, TAG_INNER_H);
            s.inner_v      = slider_val(page, TAG_INNER_V);
            s.outer_all    = slider_val(page, TAG_OUTER);
        }
        2 => {
            let tiles: i64 = msg_send![view_tag(page, TAG_LAYOUT_TILES), state];
            s.layout_tiles = tiles != 0;
            s.accordion_pad = slider_val(page, TAG_ACC_PAD);
        }
        3 => {
            s.workspace_names = (0..9).map(|i| field_val(page, TAG_WS_BASE + i)).collect();
        }
        4 => {
            if radio_on(page, TAG_MOD_CMD)  { s.modifier = "cmd".into(); }
            else if radio_on(page, TAG_MOD_CTRL) { s.modifier = "ctrl".into(); }
            else { s.modifier = "alt".into(); }
        }
        5 => {
            s.float_apps = (0..5).map(|i| field_val(page, TAG_FLOAT_BASE + i)).collect();
        }
        _ => {}
    }
}

unsafe fn slider_val(parent: *mut Object, tag: i64) -> i32 {
    let v = view_tag(parent, tag);
    if v.is_null() { return 0; }
    let f: f64 = msg_send![v, doubleValue];
    f as i32
}

unsafe fn field_val(parent: *mut Object, tag: i64) -> String {
    let v = view_tag(parent, tag);
    if v.is_null() { return String::new(); }
    let ns: *mut Object = msg_send![v, stringValue];
    nsstr_to_rust(ns)
}

unsafe fn radio_on(parent: *mut Object, tag: i64) -> bool {
    let v = view_tag(parent, tag);
    if v.is_null() { return false; }
    let st: i64 = msg_send![v, state];
    st != 0
}

unsafe fn view_tag(parent: *mut Object, tag: i64) -> *mut Object {
    msg_send![parent, viewWithTag: tag]
}

// ─── Review page updater ──────────────────────────────────────────────────────

unsafe fn update_review(tab_view: *mut Object) {
    let item: *mut Object = msg_send![tab_view, tabViewItemAtIndex: (STEP_COUNT - 1) as i64];
    let page: *mut Object = msg_send![item, view];
    let tv: *mut Object   = msg_send![page, viewWithTag: TAG_REVIEW_TV];
    if tv.is_null() { return; }
    let toml = generate_toml(&state());
    let _: () = msg_send![tv, setString: nsstring(&toml)];
}

// ─── Navigation helpers ───────────────────────────────────────────────────────

unsafe fn refresh_nav(this: &Object, step: usize) {
    let win = *this.get_ivar::<usize>("_window") as *mut Object;
    let cv: *mut Object = msg_send![win, contentView];

    let back: *mut Object = msg_send![cv, viewWithTag: TAG_BACK_BTN];
    if !back.is_null() {
        let en = if step > 0 { YES } else { NO };
        let _: () = msg_send![back, setEnabled: en];
    }

    let next: *mut Object = msg_send![cv, viewWithTag: TAG_NEXT_BTN];
    if !next.is_null() {
        let title = if step == STEP_COUNT - 1 { "Finish & Save" } else { "Next  →" };
        let _: () = msg_send![next, setTitle: nsstring(title)];
    }

    let lbl: *mut Object = msg_send![cv, viewWithTag: TAG_STEP_LABEL];
    if !lbl.is_null() {
        let dots: String = (0..STEP_COUNT).map(|i| if i == step { '●' } else { '○' }).collect();
        let names = ["Welcome", "Gaps", "Layout", "Workspaces", "Modifier", "Float Apps", "Review"];
        let text = format!("Step {} of {}  —  {}   {}", step + 1, STEP_COUNT, names[step], dots);
        let _: () = msg_send![lbl, setStringValue: nsstring(&text)];
    }
}

// ─── Finish ───────────────────────────────────────────────────────────────────

unsafe fn finish_wizard(this: &Object) {
    let toml = generate_toml(&state());
    let path = super::config_path();
    if let Some(p) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::write(&path, &toml);
    super::send_ipc("reload");

    // Close wizard window
    let win = *this.get_ivar::<usize>("_window") as *mut Object;
    let _: () = msg_send![win, close];

    // Success alert
    let alert: *mut Object = msg_send![class!(NSAlert), new];
    let _: () = msg_send![alert, setMessageText:    nsstring("Setup complete!")];
    let _: () = msg_send![alert, setInformativeText: nsstring(
        "Your config has been saved to ~/.config/yabai-rs/yabai-rs.toml\nand the daemon has been asked to reload."
    )];
    let _: () = msg_send![alert, runModal];
}

// ─── TOML generator ───────────────────────────────────────────────────────────

pub fn generate_toml(d: &WizardData) -> String {
    let layout = if d.layout_tiles { "tiles" } else { "accordion" };
    let mut s  = format!(
        "# yabai-rs config — generated by Setup Wizard\n\n\
         [gaps]\n\
         inner-horizontal = {}\n\
         inner-vertical   = {}\n\
         outer-left       = {}\n\
         outer-right      = {}\n\
         outer-top        = {}\n\
         outer-bottom     = {}\n\n\
         [mode]\n\
         default-layout    = \"{layout}\"\n\
         accordion-padding = {}\n\n",
        d.inner_h, d.inner_v,
        d.outer_all, d.outer_all, d.outer_all, d.outer_all,
        d.accordion_pad,
    );

    for name in &d.workspace_names {
        if !name.trim().is_empty() {
            s += &format!("[[workspaces]]\nname = \"{name}\"\n\n");
        }
    }

    let nav = [("h","focus left"),("j","focus down"),("k","focus up"),("l","focus right"),
               ("shift-h","move left"),("shift-j","move down"),("shift-k","move up"),("shift-l","move right"),
               ("/","layout tiles"),(",","layout accordion"),("f","fullscreen"),("shift-q","close")];
    for (key, cmd) in nav {
        s += &format!("[[keybindings]]\nkey     = \"{}-{key}\"\ncommand = \"{cmd}\"\n\n", d.modifier);
    }
    for name in &d.workspace_names {
        if !name.trim().is_empty() {
            s += &format!("[[keybindings]]\nkey     = \"{}-{name}\"\ncommand = \"workspace {name}\"\n\n", d.modifier);
            s += &format!("[[keybindings]]\nkey     = \"{}-shift-{name}\"\ncommand = \"move-node-to-workspace {name}\"\n\n", d.modifier);
        }
    }

    for app in &d.float_apps {
        if !app.trim().is_empty() {
            s += &format!("[[on-window-detected]]\napp     = \"{app}\"\ncommand = \"layout floating\"\n\n");
        }
    }
    s
}

// ─── Page builders ────────────────────────────────────────────────────────────

unsafe fn new_page() -> *mut Object {
    let v: *mut Object = msg_send![class!(NSView), alloc];
    msg_send![v, initWithFrame: r(0.0, 0.0, WIN_W, TAB_H)]
}

// Plain label (non-editable NSTextField)
unsafe fn label(parent: *mut Object, text: &str, x: f64, y: f64, w: f64, h: f64, size: f64, bold: bool) -> *mut Object {
    let f: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![f, setFrame:            r(x, y, w, h)];
    let _: () = msg_send![f, setStringValue:       nsstring(text)];
    let _: () = msg_send![f, setBezeled:           NO];
    let _: () = msg_send![f, setDrawsBackground:   NO];
    let _: () = msg_send![f, setEditable:          NO];
    let _: () = msg_send![f, setSelectable:        NO];
    let font: *mut Object = if bold {
        msg_send![class!(NSFont), boldSystemFontOfSize: size]
    } else {
        msg_send![class!(NSFont), systemFontOfSize: size]
    };
    let _: () = msg_send![f, setFont: font];
    let _: () = msg_send![parent, addSubview: f];
    f
}

// Editable text field
unsafe fn text_field(parent: *mut Object, placeholder: &str, default: &str, x: f64, y: f64, w: f64, tag: i64) -> *mut Object {
    let f: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![f, setFrame:        r(x, y, w, 24.0)];
    let _: () = msg_send![f, setStringValue:   nsstring(default)];
    let _: () = msg_send![f, setPlaceholderString: nsstring(placeholder)];
    let _: () = msg_send![f, setTag:           tag];
    let _: () = msg_send![parent, addSubview: f];
    f
}

// Slider row: [label] [————slider————] [val px]
unsafe fn slider_row(parent: *mut Object, title: &str, tag: i64, default: f64, y: f64, delegate: *mut Object) {
    label(parent, title, 20.0, y + 2.0, 150.0, 20.0, 13.0, false);

    let sl: *mut Object = msg_send![class!(NSSlider), new];
    let _: () = msg_send![sl, setFrame:       r(175.0, y, 300.0, 20.0)];
    let _: () = msg_send![sl, setMinValue:    0.0f64];
    let _: () = msg_send![sl, setMaxValue:    80.0f64];
    let _: () = msg_send![sl, setDoubleValue: default];
    let _: () = msg_send![sl, setTag:         tag];
    let _: () = msg_send![sl, setContinuous:  YES];
    let _: () = msg_send![sl, setTarget:      delegate];
    let _: () = msg_send![sl, setAction:      sel!(sliderChanged:)];
    let _: () = msg_send![parent, addSubview: sl];

    let lbl: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![lbl, setFrame:           r(485.0, y + 2.0, 70.0, 18.0)];
    let _: () = msg_send![lbl, setStringValue:      nsstring(&format!("{} px", default as i32))];
    let _: () = msg_send![lbl, setBezeled:          NO];
    let _: () = msg_send![lbl, setDrawsBackground:  NO];
    let _: () = msg_send![lbl, setEditable:         NO];
    let _: () = msg_send![lbl, setTag:              tag + 50];
    let _: () = msg_send![parent, addSubview: lbl];
}

// Radio button
unsafe fn radio(parent: *mut Object, title: &str, x: f64, y: f64, w: f64, tag: i64, on: bool, target: *mut Object, action: Sel) -> *mut Object {
    let b: *mut Object = msg_send![class!(NSButton), new];
    let _: () = msg_send![b, setFrame:      r(x, y, w, 22.0)];
    let _: () = msg_send![b, setButtonType: 4i64]; // NSRadioButton
    let _: () = msg_send![b, setTitle:      nsstring(title)];
    let _: () = msg_send![b, setTag:        tag];
    let _: () = msg_send![b, setState:      if on { 1i64 } else { 0i64 }];
    let _: () = msg_send![b, setTarget:     target];
    let _: () = msg_send![b, setAction:     action];
    let _: () = msg_send![parent, addSubview: b];
    b
}

// ── Page 0: Welcome ──────────────────────────────────────────────────────────

unsafe fn page_welcome() -> *mut Object {
    let p = new_page();
    label(p, "Welcome to yabai-rs", 40.0, 280.0, 520.0, 40.0, 22.0, true);
    label(p, "This wizard will configure your tiling window manager\nin a few simple steps. Click Next to begin.", 40.0, 210.0, 520.0, 60.0, 14.0, false);
    label(p, "What you'll set up:", 40.0, 165.0, 300.0, 20.0, 13.0, true);
    label(p, "  • Window gap sizes\n  • Default tiling layout\n  • Workspace names\n  • Keyboard modifier key\n  • Apps that float instead of tiling", 40.0, 60.0, 400.0, 110.0, 13.0, false);
    p
}

// ── Page 1: Gaps ─────────────────────────────────────────────────────────────

unsafe fn page_gaps(delegate: *mut Object) -> *mut Object {
    let p = new_page();
    label(p, "Window Gaps", 20.0, 328.0, 400.0, 28.0, 18.0, true);
    label(p, "Set the pixel spacing between windows and screen edges.", 20.0, 302.0, 500.0, 20.0, 13.0, false);
    slider_row(p, "Inner horizontal", TAG_INNER_H,  10.0, 248.0, delegate);
    slider_row(p, "Inner vertical",   TAG_INNER_V,  10.0, 198.0, delegate);
    slider_row(p, "Outer padding",    TAG_OUTER,    10.0, 148.0, delegate);
    p
}

// ── Page 2: Layout ───────────────────────────────────────────────────────────

unsafe fn page_layout(delegate: *mut Object) -> *mut Object {
    let p = new_page();
    label(p, "Default Layout", 20.0, 328.0, 400.0, 28.0, 18.0, true);
    label(p, "Choose how windows are arranged on your screen.", 20.0, 302.0, 500.0, 20.0, 13.0, false);

    // Dummy action for radio buttons — they're read during collect_page
    radio(p, "Tiles  —  windows split the screen side by side",  20.0, 265.0, 500.0, TAG_LAYOUT_TILES, true,  std::ptr::null_mut(), sel!(class));
    radio(p, "Accordion  —  windows stack with visible titlebars", 20.0, 235.0, 500.0, TAG_LAYOUT_ACC,  false, std::ptr::null_mut(), sel!(class));

    label(p, "Accordion peek width:", 20.0, 188.0, 160.0, 20.0, 13.0, false);
    slider_row(p, "Accordion peek", TAG_ACC_PAD, 30.0, 148.0, delegate);
    p
}

// ── Page 3: Workspaces ───────────────────────────────────────────────────────

unsafe fn page_workspaces() -> *mut Object {
    let p = new_page();
    label(p, "Workspaces", 20.0, 328.0, 400.0, 28.0, 18.0, true);
    label(p, "Name your workspaces. Leave a field blank to disable that slot.", 20.0, 302.0, 540.0, 20.0, 13.0, false);

    // 3 columns × 3 rows
    let cols = [20.0_f64, 210.0, 400.0];
    let rows = [250.0_f64, 200.0, 150.0];
    let mut idx = 0;
    for &row_y in &rows {
        for &col_x in &cols {
            if idx < 9 {
                let default = format!("{}", idx + 1);
                text_field(p, &format!("Workspace {}", idx + 1), &default, col_x, row_y, 170.0, TAG_WS_BASE + idx as i64);
                idx += 1;
            }
        }
    }
    p
}

// ── Page 4: Modifier key ─────────────────────────────────────────────────────

unsafe fn page_modifier(delegate: *mut Object) -> *mut Object {
    let p = new_page();
    label(p, "Keyboard Modifier", 20.0, 328.0, 400.0, 28.0, 18.0, true);
    label(p, "Choose the modifier key used for all window-manager shortcuts.", 20.0, 302.0, 540.0, 20.0, 13.0, false);

    let act = sel!(modifierPicked:);
    radio(p, "Alt / Option  ⌥   (recommended — doesn't conflict with system shortcuts)", 20.0, 265.0, 530.0, TAG_MOD_ALT,  true,  delegate, act);
    radio(p, "Command  ⌘",                                                                20.0, 235.0, 300.0, TAG_MOD_CMD,  false, delegate, act);
    radio(p, "Control  ⌃",                                                                20.0, 205.0, 300.0, TAG_MOD_CTRL, false, delegate, act);

    // Preview label
    let prev: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![prev, setFrame:           r(20.0, 80.0, 540.0, 80.0)];
    let _: () = msg_send![prev, setStringValue:      nsstring("⌥+h/j/k/l  →  focus left/down/up/right\n⌥+Shift+h/j/k/l  →  move window\n⌥+1..9  →  switch workspace")];
    let _: () = msg_send![prev, setBezeled:          NO];
    let _: () = msg_send![prev, setDrawsBackground:  YES];
    let _: () = msg_send![prev, setEditable:         NO];
    let _: () = msg_send![prev, setSelectable:       NO];
    let font: *mut Object = msg_send![class!(NSFont), fontWithName: nsstring("Menlo") size: 12.0f64];
    if !font.is_null() { let _: () = msg_send![prev, setFont: font]; }
    let _: () = msg_send![prev, setTag: TAG_MOD_PREVIEW];
    let _: () = msg_send![p, addSubview: prev];

    p
}

// ── Page 5: Float apps ───────────────────────────────────────────────────────

unsafe fn page_float_apps() -> *mut Object {
    let p = new_page();
    label(p, "Floating Apps", 20.0, 328.0, 400.0, 28.0, 18.0, true);
    label(p, "These apps will float (not be tiled). Clear a field to remove the rule.", 20.0, 302.0, 540.0, 20.0, 13.0, false);
    label(p, "App name", 20.0, 274.0, 200.0, 18.0, 12.0, true);

    let defaults = ["System Preferences", "Calculator", "Finder", "", ""];
    for (i, &def) in defaults.iter().enumerate() {
        let y = 248.0 - i as f64 * 36.0;
        text_field(p, "App name…", def, 20.0, y, 350.0, TAG_FLOAT_BASE + i as i64);
    }
    p
}

// ── Page 6: Review ───────────────────────────────────────────────────────────

unsafe fn page_review() -> *mut Object {
    let p = new_page();
    label(p, "Review your configuration", 20.0, 340.0, 500.0, 28.0, 18.0, true);
    label(p, "Click \"Finish & Save\" to write this to ~/.config/yabai-rs/yabai-rs.toml", 20.0, 314.0, 540.0, 20.0, 12.0, false);

    let scroll: *mut Object = msg_send![class!(NSScrollView), alloc];
    let _: () = msg_send![scroll, initWithFrame: r(20.0, 10.0, WIN_W - 40.0, 295.0)];
    let _: () = msg_send![scroll, setHasVerticalScroller: YES];
    let _: () = msg_send![scroll, setAutohidesScrollers: YES];

    let tv: *mut Object = msg_send![class!(NSTextView), alloc];
    let _: () = msg_send![tv, initWithFrame: r(0.0, 0.0, WIN_W - 40.0, 8000.0)];
    let _: () = msg_send![tv, setEditable:   NO];
    let _: () = msg_send![tv, setTag:        TAG_REVIEW_TV];
    let font: *mut Object = msg_send![class!(NSFont), fontWithName: nsstring("Menlo") size: 11.0f64];
    if !font.is_null() { let _: () = msg_send![tv, setFont: font]; }

    let _: () = msg_send![scroll, setDocumentView: tv];
    let _: () = msg_send![p, addSubview: scroll];
    p
}

// ─── String helper ────────────────────────────────────────────────────────────

unsafe fn nsstr_to_rust(ns: *mut Object) -> String {
    if ns.is_null() { return String::new(); }
    let bytes: *const c_char = msg_send![ns, UTF8String];
    if bytes.is_null() { return String::new(); }
    std::ffi::CStr::from_ptr(bytes).to_str().unwrap_or("").to_string()
}

unsafe fn nsstring(s: &str) -> *mut Object {
    super::nsstring(s)
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub unsafe fn open_wizard() {
    // Reset state to defaults each time the wizard opens
    *WIZARD_STATE.get_or_init(|| Mutex::new(WizardData::default())).lock().unwrap()
        = WizardData::default();

    let cls      = get_wizard_class();
    let delegate: *mut Object = msg_send![cls, new];

    // ── Window ────────────────────────────────────────────────────────────────
    let style: usize = 1 | 2 | 4; // titled | closable | miniaturizable
    let win: *mut Object = msg_send![class!(NSWindow), alloc];
    let win: *mut Object = msg_send![win,
        initWithContentRect: r(0.0, 0.0, WIN_W, WIN_H)
        styleMask: style
        backing: 2usize
        defer: NO
    ];
    let _: () = msg_send![win, setTitle: nsstring("yabai-rs Setup Wizard")];
    let _: () = msg_send![win, setReleasedWhenClosed: NO];
    let _: () = msg_send![win, center];

    let cv: *mut Object = msg_send![win, contentView];

    // ── Header: step label ────────────────────────────────────────────────────
    let step_lbl: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![step_lbl, setFrame:           r(16.0, WIN_H - 46.0, WIN_W - 32.0, 28.0)];
    let _: () = msg_send![step_lbl, setStringValue:      nsstring("Step 1 of 7  —  Welcome   ● ○ ○ ○ ○ ○ ○")];
    let _: () = msg_send![step_lbl, setBezeled:          NO];
    let _: () = msg_send![step_lbl, setDrawsBackground:  NO];
    let _: () = msg_send![step_lbl, setEditable:         NO];
    let font: *mut Object = msg_send![class!(NSFont), boldSystemFontOfSize: 12.0f64];
    let _: () = msg_send![step_lbl, setFont: font];
    let _: () = msg_send![step_lbl, setTag: TAG_STEP_LABEL];
    let _: () = msg_send![cv, addSubview: step_lbl];

    // ── Tab view (pages) ──────────────────────────────────────────────────────
    let tab: *mut Object = msg_send![class!(NSTabView), alloc];
    let tab: *mut Object = msg_send![tab, initWithFrame: r(0.0, TAB_Y, WIN_W, TAB_H)];
    let _: () = msg_send![tab, setTabViewType: 3i64]; // NSNoTabsNoBorder

    for page in [
        page_welcome(),
        page_gaps(delegate),
        page_layout(delegate),
        page_workspaces(),
        page_modifier(delegate),
        page_float_apps(),
        page_review(),
    ] {
        let item: *mut Object = msg_send![class!(NSTabViewItem), alloc];
        let item: *mut Object = msg_send![item, init];
        let _: () = msg_send![item, setView: page];
        let _: () = msg_send![tab, addTabViewItem: item];
    }
    let _: () = msg_send![tab, selectTabViewItemAtIndex: 0i64];
    let _: () = msg_send![cv, addSubview: tab];

    // ── Footer buttons ────────────────────────────────────────────────────────
    let back: *mut Object = msg_send![class!(NSButton), alloc];
    let back: *mut Object = msg_send![back, initWithFrame: r(16.0, 12.0, 100.0, 28.0)];
    let _: () = msg_send![back, setTitle:      nsstring("←  Back")];
    let _: () = msg_send![back, setBezelStyle: 1usize];
    let _: () = msg_send![back, setTarget:     delegate];
    let _: () = msg_send![back, setAction:     sel!(wizardBack:)];
    let _: () = msg_send![back, setTag:        TAG_BACK_BTN];
    let _: () = msg_send![back, setEnabled:    NO];
    let _: () = msg_send![cv, addSubview: back];

    let next: *mut Object = msg_send![class!(NSButton), alloc];
    let next: *mut Object = msg_send![next, initWithFrame: r(WIN_W - 140.0, 12.0, 124.0, 28.0)];
    let _: () = msg_send![next, setTitle:      nsstring("Next  →")];
    let _: () = msg_send![next, setBezelStyle: 1usize];
    let _: () = msg_send![next, setTarget:     delegate];
    let _: () = msg_send![next, setAction:     sel!(wizardNext:)];
    let _: () = msg_send![next, setTag:        TAG_NEXT_BTN];
    let _: () = msg_send![cv, addSubview: next];

    // Make Next the default (Return key) button
    let next_cell: *mut Object = msg_send![next, cell];
    let _: () = msg_send![win, setDefaultButtonCell: next_cell];

    // ── Store ivars on delegate ───────────────────────────────────────────────
    (*delegate).set_ivar("_step",    0usize);
    (*delegate).set_ivar("_window",  win as usize);
    (*delegate).set_ivar("_tab_view", tab as usize);

    let _: () = msg_send![win, makeKeyAndOrderFront: std::ptr::null::<Object>()];
}
