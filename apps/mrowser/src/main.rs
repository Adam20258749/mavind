//! Mrowser — Mavind's lightweight browser. Thin GTK4 chrome around WebKitGTK
//! (no custom engine). Tabs, smart address bar, bookmarks, history, downloads,
//! private windows.

mod store;

use gtk4::gdk::Key;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, EventControllerKey, Label,
    ListBox, ListBoxRow, MenuButton, Notebook, Orientation, Popover,
};
use std::cell::RefCell;
use std::rc::Rc;
use webkit6::prelude::*;
use webkit6::{LoadEvent, NetworkSession, WebView};

const APP_ID: &str = "os.mavind.Mrowser";
const HOME: &str = "https://duckduckgo.com/";
const SEARCH: &str = "https://duckduckgo.com/?q=";

struct Ctx {
    win: ApplicationWindow,
    nb: Notebook,
    url: Entry,
    back: Button,
    fwd: Button,
    session: NetworkSession,
    private: bool,
    bookmarks: RefCell<store::List>,
    history: RefCell<store::List>,
}
type Shared = Rc<Ctx>;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(|app| {
        build(app, None);
    });
    app.connect_open(|app, files, _| {
        let first = files.first().map(|f| f.uri().to_string());
        build(app, first);
    });
    app.run()
}

fn build(app: &Application, open_url: Option<String>) {
    let private = std::env::args().any(|a| a == "--private" || a == "-p");

    let session = if private {
        NetworkSession::new_ephemeral()
    } else {
        let base = format!("{}/mrowser/webkit", data_home());
        let cache = format!("{}/mrowser/webkit", cache_home());
        let _ = std::fs::create_dir_all(&base);
        let _ = std::fs::create_dir_all(&cache);
        NetworkSession::new(Some(base.as_str()), Some(cache.as_str()))
    };
    wire_downloads(&session);

    // ---- toolbar ------------------------------------------------------
    let back = icon_btn("go-previous-symbolic", "Back");
    let fwd = icon_btn("go-next-symbolic", "Forward");
    let reload = icon_btn("view-refresh-symbolic", "Reload");
    let newtab = icon_btn("tab-new-symbolic", "New tab");
    let star = icon_btn("bookmark-new-symbolic", "Bookmark this page");
    let url = Entry::builder().hexpand(true).build();
    url.set_placeholder_text(Some("Search DuckDuckGo or type a URL"));
    let menu = MenuButton::builder().icon_name("open-menu-symbolic").build();

    let bar = GtkBox::new(Orientation::Horizontal, 4);
    bar.set_margin_top(5);
    bar.set_margin_bottom(5);
    bar.set_margin_start(6);
    bar.set_margin_end(6);
    for w in [&back, &fwd, &reload, &newtab] {
        bar.append(w);
    }
    bar.append(&url);
    bar.append(&star);
    bar.append(&menu);

    let nb = Notebook::builder().scrollable(true).show_border(false).build();

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&bar);
    root.append(&nb);

    let win = ApplicationWindow::builder()
        .application(app)
        .title(if private { "Mrowser (Private)" } else { "Mrowser" })
        .default_width(1024)
        .default_height(720)
        .child(&root)
        .build();
    if private {
        win.add_css_class("private");
    }

    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        nb: nb.clone(),
        url: url.clone(),
        back: back.clone(),
        fwd: fwd.clone(),
        session,
        private,
        bookmarks: RefCell::new(store::load(store::Kind::Bookmarks)),
        history: RefCell::new(store::load(store::Kind::History)),
    });

    // ---- wiring -----------------------------------------------------
    {
        let c = ctx.clone();
        back.connect_clicked(move |_| {
            if let Some(v) = current(&c) {
                v.go_back();
            }
        });
    }
    {
        let c = ctx.clone();
        fwd.connect_clicked(move |_| {
            if let Some(v) = current(&c) {
                v.go_forward();
            }
        });
    }
    {
        let c = ctx.clone();
        reload.connect_clicked(move |_| {
            if let Some(v) = current(&c) {
                v.reload();
            }
        });
    }
    {
        let c = ctx.clone();
        newtab.connect_clicked(move |_| new_tab(&c, None));
    }
    {
        let c = ctx.clone();
        star.connect_clicked(move |_| add_bookmark(&c));
    }
    {
        let c = ctx.clone();
        url.connect_activate(move |e| navigate(&c, &e.text()));
    }
    {
        let c = ctx.clone();
        nb.connect_switch_page(move |_, _, _| {
            // page arg isn't the WebView reliably before realize; sync from state
            let c = c.clone();
            glib::idle_add_local_once(move || sync_chrome(&c));
        });
    }
    {
        let c = ctx.clone();
        menu.set_create_popup_func(move |mb| mb.set_popover(Some(&menu_popover(&c))));
    }

    // keyboard shortcuts
    let keys = EventControllerKey::new();
    {
        let c = ctx.clone();
        keys.connect_key_pressed(move |_, key, _, mods| {
            let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let alt = mods.contains(gtk4::gdk::ModifierType::ALT_MASK);
            match (ctrl, alt, key) {
                (true, _, Key::t) => new_tab(&c, None),
                (true, _, Key::w) => close_tab(&c, c.nb.current_page()),
                (true, _, Key::l) => {
                    c.url.grab_focus();
                    c.url.select_region(0, -1);
                }
                (true, _, Key::r) | (_, _, Key::F5) => {
                    if let Some(v) = current(&c) {
                        v.reload();
                    }
                }
                (true, _, Key::q) => c.win.close(),
                (_, true, Key::Left) => {
                    if let Some(v) = current(&c) {
                        v.go_back();
                    }
                }
                (_, true, Key::Right) => {
                    if let Some(v) = current(&c) {
                        v.go_forward();
                    }
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
    }
    win.add_controller(keys);

    win.present();
    new_tab(&ctx, open_url.as_deref().or(Some(HOME)));
}

// ---- tabs -----------------------------------------------------------
fn new_tab(c: &Shared, uri: Option<&str>) {
    let view = WebView::builder().network_session(&c.session).build();

    let tab = GtkBox::new(Orientation::Horizontal, 4);
    let label = Label::new(Some("New tab"));
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_width_chars(16);
    label.set_max_width_chars(20);
    let close = Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.set_has_frame(false);
    tab.append(&label);
    tab.append(&close);

    let idx = c.nb.append_page(&view, Some(&tab));
    c.nb.set_tab_reorderable(&view, true);
    c.nb.set_current_page(Some(idx));

    // per-view signals -> update this tab's label + (if current) the chrome
    {
        let c = c.clone();
        let label = label.clone();
        let view2 = view.clone();
        view.connect_title_notify(move |v| {
            let t = v.title().map(|s| s.to_string()).unwrap_or_default();
            label.set_text(if t.is_empty() { "…" } else { &t });
            if is_current(&c, &view2) {
                c.win.set_title(Some(&format!(
                    "{}{}",
                    if t.is_empty() { "Mrowser".into() } else { t },
                    if c.private { "  —  Private" } else { "" }
                )));
            }
        });
    }
    {
        let c = c.clone();
        let view2 = view.clone();
        view.connect_uri_notify(move |v| {
            if is_current(&c, &view2) {
                if let Some(u) = v.uri() {
                    c.url.set_text(&u);
                }
                sync_chrome(&c);
            }
        });
    }
    {
        let c = c.clone();
        let view2 = view.clone();
        view.connect_load_changed(move |v, ev| {
            if ev == LoadEvent::Finished {
                if let (Some(u), t) = (v.uri(), v.title().map(|s| s.to_string())) {
                    if !c.private && !u.starts_with("about:") {
                        c.history
                            .borrow_mut()
                            .push_capped(&t.unwrap_or_default(), &u, 800);
                        store::save(store::Kind::History, &c.history.borrow());
                    }
                }
            }
            if is_current(&c, &view2) {
                sync_chrome(&c);
            }
        });
    }
    {
        let c = c.clone();
        let view2 = view.clone();
        close.connect_clicked(move |_| {
            let n = c.nb.page_num(&view2);
            close_tab(&c, n);
        });
    }

    match uri {
        Some(u) if !u.is_empty() => view.load_uri(u),
        _ => view.load_uri(HOME),
    }
}

fn close_tab(c: &Shared, page: Option<u32>) {
    let Some(n) = page else { return };
    c.nb.remove_page(Some(n));
    if c.nb.n_pages() == 0 {
        c.win.close();
    }
}

fn current(c: &Shared) -> Option<WebView> {
    let idx = c.nb.current_page()?;
    c.nb.nth_page(Some(idx))?.downcast::<WebView>().ok()
}

fn is_current(c: &Shared, v: &WebView) -> bool {
    current(c).map(|cur| cur == *v).unwrap_or(false)
}

fn sync_chrome(c: &Shared) {
    if let Some(v) = current(c) {
        c.back.set_sensitive(v.can_go_back());
        c.fwd.set_sensitive(v.can_go_forward());
        if !c.url.has_focus() {
            if let Some(u) = v.uri() {
                c.url.set_text(&u);
            }
        }
    }
}

// ---- navigation ---------------------------------------------------
fn navigate(c: &Shared, text: &str) {
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    let target = if t.contains("://") {
        t.to_string()
    } else if t == "localhost" || (!t.contains(' ') && t.contains('.') && !t.ends_with('.')) {
        format!("https://{t}")
    } else {
        format!("{SEARCH}{}", glib::Uri::escape_string(t, None, false))
    };
    if let Some(v) = current(c) {
        v.load_uri(&target);
        v.grab_focus();
    } else {
        new_tab(c, Some(&target));
    }
}

// ---- bookmarks / history menu ------------------------------------
fn add_bookmark(c: &Shared) {
    if let Some(v) = current(c) {
        if let Some(u) = v.uri() {
            let title = v.title().map(|s| s.to_string()).unwrap_or_default();
            c.bookmarks.borrow_mut().push_unique(&title, &u);
            store::save(store::Kind::Bookmarks, &c.bookmarks.borrow());
        }
    }
}

fn menu_popover(c: &Shared) -> Popover {
    let col = GtkBox::new(Orientation::Vertical, 2);
    col.set_margin_top(6);
    col.set_margin_bottom(6);
    col.set_margin_start(6);
    col.set_margin_end(6);

    let mk = |label: &str| {
        let b = Button::with_label(label);
        b.set_halign(Align::Fill);
        b.set_has_frame(false);
        b
    };

    let b_new = mk("New Tab");
    let b_priv = mk("New Private Window");
    let b_dl = mk("Open Downloads Folder");
    {
        let c = c.clone();
        b_new.connect_clicked(move |_| new_tab(&c, None));
    }
    b_priv.connect_clicked(|_| {
        let _ = std::process::Command::new("mrowser").arg("--private").spawn();
    });
    b_dl.connect_clicked(|_| {
        let _ = std::process::Command::new("xdg-open")
            .arg(dirs_download())
            .spawn();
    });
    col.append(&b_new);
    col.append(&b_priv);
    col.append(&b_dl);
    col.append(&sep());

    col.append(&section_label("Bookmarks"));
    col.append(&link_list(c, &c.bookmarks.borrow()));
    col.append(&sep());
    col.append(&section_label("Recent history"));
    col.append(&link_list(c, &tail(&c.history.borrow(), 20)));

    Popover::builder().child(&col).build()
}

fn link_list(c: &Shared, items: &store::List) -> ListBox {
    let lb = ListBox::new();
    lb.set_selection_mode(gtk4::SelectionMode::None);
    if items.0.is_empty() {
        let r = ListBoxRow::new();
        let l = Label::new(Some("(none)"));
        l.add_css_class("dim");
        r.set_child(Some(&l));
        lb.append(&r);
        return lb;
    }
    for it in items.0.iter().rev().take(25) {
        let row = ListBoxRow::new();
        let l = Label::new(Some(if it.title.is_empty() { &it.url } else { &it.title }));
        l.set_halign(Align::Start);
        l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        l.set_max_width_chars(40);
        l.set_tooltip_text(Some(&it.url));
        row.set_child(Some(&l));
        let c = c.clone();
        let url = it.url.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| navigate(&c, &url));
        row.add_controller(gesture);
        lb.append(&row);
    }
    lb
}

fn tail(l: &store::List, n: usize) -> store::List {
    let start = l.0.len().saturating_sub(n);
    store::List(l.0[start..].to_vec())
}

// ---- downloads --------------------------------------------------
fn wire_downloads(session: &NetworkSession) {
    session.connect_download_started(|_, download| {
        // Set a destination up front (basename of the URL) so WebKit doesn't
        // prompt. Server-suggested names via Content-Disposition are skipped for
        // simplicity — good enough for a lightweight browser.
        let name = download
            .request()
            .and_then(|r| r.uri())
            .map(|u| u.to_string())
            .and_then(|u| {
                u.split(['?', '#'])
                    .next()
                    .and_then(|p| p.rsplit('/').next())
                    .map(|s| s.to_string())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "download".into());
        download.set_destination(&format!("{}/{}", dirs_download(), name));

        download.connect_finished(|dl| {
            if let Some(dest) = dl.destination() {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "Download finished",
                        dest.trim_start_matches("file://"),
                    ])
                    .spawn();
            }
        });
    });
}

// ---- small helpers -------------------------------------------
fn icon_btn(icon: &str, tip: &str) -> Button {
    let b = Button::from_icon_name(icon);
    b.set_tooltip_text(Some(tip));
    b
}
fn sep() -> gtk4::Separator {
    gtk4::Separator::new(Orientation::Horizontal)
}
fn section_label(t: &str) -> Label {
    let l = Label::new(Some(t));
    l.set_halign(Align::Start);
    l.add_css_class("dim");
    l.set_margin_top(4);
    l
}
fn dirs_download() -> String {
    let d = format!("{}/Downloads", home());
    let _ = std::fs::create_dir_all(&d);
    d
}
fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".into())
}
fn data_home() -> String {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|p| p.starts_with('/'))
        .unwrap_or_else(|| format!("{}/.local/share", home()))
}
fn cache_home() -> String {
    std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|p| p.starts_with('/'))
        .unwrap_or_else(|| format!("{}/.cache", home()))
}
