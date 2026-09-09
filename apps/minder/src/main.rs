//! Minder — Mavind's lightweight file manager.
//! Navigation, copy/move/rename/delete, search, properties, drives.
//!
//! No `unsafe`, no `clone!` macro. Shared state + widgets live in one `Ctx`
//! behind an `Rc`; behaviour is plain free functions that take `&Ctx`.

mod fsops;
mod places;

use fsops::{fmt_size, Entry};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry as GtkEntry, Image, Label,
    ListBox, ListBoxRow, Orientation, Paned, PolicyType, ScrolledWindow, SelectionMode, ToggleButton,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

const APP_ID: &str = "os.mavind.Minder";
const MAX_ROWS: usize = 6000;

#[derive(Default)]
struct State {
    cwd: PathBuf,
    back: Vec<PathBuf>,
    fwd: Vec<PathBuf>,
    hidden: bool,
    filter: String,
    shown: Vec<PathBuf>,  // index-aligned with the file ListBox rows
    places: Vec<PathBuf>, // index-aligned with the places ListBox rows
    clipboard: Option<(Vec<PathBuf>, bool)>, // (paths, cut?)
}

struct Ctx {
    win: ApplicationWindow,
    files: ListBox,
    places_list: ListBox,
    path_entry: GtkEntry,
    status: Label,
    st: RefCell<State>,
}

type Shared = Rc<Ctx>;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_activate(|app| {
        build(app, None);
    });
    app.connect_open(|app, files, _| {
        let start = files.first().and_then(|f| f.path()).map(|p| {
            if p.is_dir() {
                p
            } else {
                p.parent().map(Path::to_path_buf).unwrap_or(p)
            }
        });
        build(app, start);
    });
    app.run()
}

fn build(app: &Application, start_arg: Option<PathBuf>) {
    if let Some(w) = app.active_window() {
        w.present();
        return;
    }
    install_css();

    let start = start_arg
        .filter(|p| p.is_dir())
        .or_else(|| std::env::args().nth(1).map(PathBuf::from).filter(|p| p.is_dir()))
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into())));

    // ---- widgets ------------------------------------------------------
    let b_back = Button::from_icon_name("go-previous-symbolic");
    let b_fwd = Button::from_icon_name("go-next-symbolic");
    let b_up = Button::from_icon_name("go-up-symbolic");
    let b_home = Button::from_icon_name("go-home-symbolic");
    let path_entry = GtkEntry::new();
    path_entry.set_hexpand(true);
    path_entry.add_css_class("path");
    let search = GtkEntry::new();
    search.set_placeholder_text(Some("Search (Enter = recursive)"));
    search.set_width_chars(24);
    let b_refresh = Button::from_icon_name("view-refresh-symbolic");
    let b_newdir = Button::from_icon_name("folder-new-symbolic");
    let t_hidden = ToggleButton::new();
    t_hidden.set_icon_name("view-reveal-symbolic");
    t_hidden.set_tooltip_text(Some("Show hidden files"));

    let bar = strip(6);
    for w in [&b_back, &b_fwd, &b_up, &b_home] {
        bar.append(w);
    }
    bar.append(&path_entry);
    bar.append(&search);
    bar.append(&b_refresh);
    bar.append(&b_newdir);
    bar.append(&t_hidden);

    let b_open = Button::with_label("Open");
    let b_winapp = Button::with_label("Open with Windows Apps");
    let b_copy = Button::with_label("Copy");
    let b_cut = Button::with_label("Cut");
    let b_paste = Button::with_label("Paste");
    let b_rename = Button::with_label("Rename");
    let b_delete = Button::with_label("Delete");
    let b_props = Button::with_label("Properties");
    let ops = strip(4);
    for b in [&b_open, &b_winapp, &b_copy, &b_cut, &b_paste, &b_rename, &b_delete, &b_props] {
        b.add_css_class("flat");
        ops.append(b);
    }

    let places_list = ListBox::new();
    places_list.set_selection_mode(SelectionMode::Single);
    places_list.add_css_class("places");
    let places_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .min_content_width(170)
        .child(&places_list)
        .build();

    let files = ListBox::new();
    files.set_selection_mode(SelectionMode::Multiple);
    files.add_css_class("files");
    let files_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .child(&files)
        .build();

    let status = Label::new(None);
    status.add_css_class("dim");
    status.set_halign(Align::Start);
    status.set_margin_start(8);
    status.set_margin_bottom(4);

    let right = GtkBox::new(Orientation::Vertical, 0);
    right.append(&files_scroll);
    right.append(&status);

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_start_child(Some(&places_scroll));
    paned.set_end_child(Some(&right));
    paned.set_position(180);
    paned.set_vexpand(true);

    let rootbox = GtkBox::new(Orientation::Vertical, 4);
    rootbox.append(&bar);
    rootbox.append(&ops);
    rootbox.append(&paned);

    let win = ApplicationWindow::builder()
        .application(app)
        .title("Minder")
        .default_width(940)
        .default_height(600)
        .child(&rootbox)
        .build();

    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        files: files.clone(),
        places_list: places_list.clone(),
        path_entry: path_entry.clone(),
        status: status.clone(),
        st: RefCell::new(State {
            cwd: start,
            ..Default::default()
        }),
    });

    // ---- wiring (each closure clones the Rc<Ctx>) -------------------
    let c = ctx.clone();
    b_back.connect_clicked(move |_| {
        let moved = {
            let mut s = c.st.borrow_mut();
            s.back.pop().map(|p| {
                let cur = std::mem::replace(&mut s.cwd, p);
                s.fwd.push(cur);
            })
        };
        if moved.is_some() {
            refresh(&c);
        }
    });

    let c = ctx.clone();
    b_fwd.connect_clicked(move |_| {
        let moved = {
            let mut s = c.st.borrow_mut();
            s.fwd.pop().map(|p| {
                let cur = std::mem::replace(&mut s.cwd, p);
                s.back.push(cur);
            })
        };
        if moved.is_some() {
            refresh(&c);
        }
    });

    let c = ctx.clone();
    b_up.connect_clicked(move |_| {
        let parent = c.st.borrow().cwd.parent().map(Path::to_path_buf);
        if let Some(p) = parent {
            navigate(&c, p);
        }
    });

    let c = ctx.clone();
    b_home.connect_clicked(move |_| {
        navigate(&c, PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into())));
    });

    let c = ctx.clone();
    b_refresh.connect_clicked(move |_| refresh(&c));

    let c = ctx.clone();
    path_entry.connect_activate(move |e| navigate(&c, PathBuf::from(e.text().to_string())));

    let c = ctx.clone();
    t_hidden.connect_toggled(move |t| {
        c.st.borrow_mut().hidden = t.is_active();
        refresh(&c);
    });

    let c = ctx.clone();
    search.connect_changed(move |e| {
        c.st.borrow_mut().filter = e.text().to_string();
        refresh(&c);
    });

    let c = ctx.clone();
    search.connect_activate(move |e| {
        let q = e.text().to_string();
        if q.is_empty() {
            return;
        }
        let base = c.st.borrow().cwd.clone();
        let hits = recursive_search(&base, &q, 4, 4000);
        clear(&c.files);
        for h in &hits {
            c.files.append(&file_row(h));
        }
        c.status
            .set_text(&format!("{} matches for \"{q}\" under {}", hits.len(), base.display()));
        c.st.borrow_mut().shown = hits.iter().map(|e| e.path.clone()).collect();
    });

    let c = ctx.clone();
    files.connect_row_activated(move |_, row| {
        let p = c.st.borrow().shown.get(row.index() as usize).cloned();
        if let Some(p) = p {
            if p.is_dir() {
                navigate(&c, p);
            } else {
                open_path(&p);
            }
        }
    });

    let c = ctx.clone();
    b_open.connect_clicked(move |_| {
        for p in selected(&c) {
            if p.is_dir() {
                navigate(&c, p);
            } else {
                open_path(&p);
            }
        }
    });

    let c = ctx.clone();
    b_winapp.connect_clicked(move |_| {
        for p in selected(&c) {
            let _ = Command::new("mavind-windows-apps").arg(&p).spawn();
        }
    });

    let c = ctx.clone();
    b_copy.connect_clicked(move |_| {
        let sel = selected(&c);
        if !sel.is_empty() {
            c.st.borrow_mut().clipboard = Some((sel, false));
        }
    });

    let c = ctx.clone();
    b_cut.connect_clicked(move |_| {
        let sel = selected(&c);
        if !sel.is_empty() {
            c.st.borrow_mut().clipboard = Some((sel, true));
        }
    });

    let c = ctx.clone();
    b_paste.connect_clicked(move |_| {
        let (clip, dst) = {
            let s = c.st.borrow();
            (s.clipboard.clone(), s.cwd.clone())
        };
        let Some((paths, cut)) = clip else { return };
        for p in &paths {
            let r = if cut {
                fsops::move_into(p, &dst)
            } else {
                fsops::copy_into(p, &dst)
            };
            if let Err(e) = r {
                error_dialog(&c.win, &format!("{e:#}"));
            }
        }
        if cut {
            c.st.borrow_mut().clipboard = None;
        }
        refresh(&c);
    });

    let c = ctx.clone();
    b_rename.connect_clicked(move |_| {
        let Some(p) = selected(&c).into_iter().next() else { return };
        let cur = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let c2 = c.clone();
        prompt(&c.win, "Rename", &cur, move |name| {
            if let Err(e) = fsops::rename(&p, &name) {
                error_dialog(&c2.win, &format!("{e:#}"));
            }
            refresh(&c2);
        });
    });

    let c = ctx.clone();
    b_newdir.connect_clicked(move |_| {
        let dir = c.st.borrow().cwd.clone();
        let c2 = c.clone();
        prompt(&c.win, "New Folder", "New Folder", move |name| {
            if let Err(e) = fsops::new_folder(&dir, &name) {
                error_dialog(&c2.win, &format!("{e:#}"));
            }
            refresh(&c2);
        });
    });

    let c = ctx.clone();
    b_delete.connect_clicked(move |_| {
        let sel = selected(&c);
        if sel.is_empty() {
            return;
        }
        let alert = gtk4::AlertDialog::builder()
            .message(format!("Move {} item(s) to Trash?", sel.len()))
            .detail("Trashed files go to ~/.local/share/mavind/trash")
            .buttons(["Cancel", "Move to Trash"])
            .cancel_button(0)
            .default_button(1)
            .modal(true)
            .build();
        let c2 = c.clone();
        alert.choose(Some(&c.win), gio::Cancellable::NONE, move |res| {
            if matches!(res, Ok(1)) {
                for p in &sel {
                    if let Err(e) = fsops::trash(p) {
                        error_dialog(&c2.win, &format!("{e:#}"));
                    }
                }
                refresh(&c2);
            }
        });
    });

    let c = ctx.clone();
    b_props.connect_clicked(move |_| {
        if let Some(p) = selected(&c).into_iter().next() {
            properties_dialog(&c.win, &p);
        }
    });

    let c = ctx.clone();
    places_list.connect_row_activated(move |_, row| {
        let p = c.st.borrow().places.get(row.index() as usize).cloned();
        if let Some(p) = p {
            navigate(&c, p);
        }
    });

    // periodic drive-list refresh
    let c = ctx.clone();
    glib::timeout_add_seconds_local(4, move || {
        rebuild_places(&c);
        glib::ControlFlow::Continue
    });

    rebuild_places(&ctx);
    refresh(&ctx);
    win.present();
}

// ---- behaviour ------------------------------------------------------
fn refresh(c: &Ctx) {
    let (cwd, hidden, filter) = {
        let s = c.st.borrow();
        (s.cwd.clone(), s.hidden, s.filter.clone())
    };
    c.path_entry.set_text(&cwd.to_string_lossy());
    c.win.set_title(Some(&format!("Minder — {}", cwd.display())));

    let all = fsops::list_dir(&cwd, hidden).unwrap_or_default();
    let fl = filter.to_lowercase();
    let shown: Vec<Entry> = all
        .iter()
        .filter(|e| fl.is_empty() || e.name.to_lowercase().contains(&fl))
        .take(MAX_ROWS)
        .cloned()
        .collect();

    clear(&c.files);
    for e in &shown {
        c.files.append(&file_row(e));
    }
    let dirs_n = shown.iter().filter(|e| e.is_dir).count();
    let files_n = shown.len() - dirs_n;
    let note = if all.len() > MAX_ROWS {
        format!("  (first {MAX_ROWS} of {})", all.len())
    } else {
        String::new()
    };
    c.status.set_text(&format!("{dirs_n} folders, {files_n} files{note}"));
    c.st.borrow_mut().shown = shown.iter().map(|e| e.path.clone()).collect();
}

fn navigate(c: &Ctx, to: PathBuf) {
    if !to.is_dir() {
        return;
    }
    {
        let mut s = c.st.borrow_mut();
        if s.cwd != to {
            let cur = std::mem::replace(&mut s.cwd, to);
            s.back.push(cur);
            s.fwd.clear();
            s.filter.clear();
        }
    }
    refresh(c);
}

fn selected(c: &Ctx) -> Vec<PathBuf> {
    let s = c.st.borrow();
    c.files
        .selected_rows()
        .iter()
        .filter_map(|r| s.shown.get(r.index() as usize).cloned())
        .collect()
}

fn rebuild_places(c: &Ctx) {
    clear(&c.places_list);
    let mut all = places::user_places();
    for mut d in places::drives() {
        d.label = format!("💾 {}", d.label);
        all.push(d);
    }
    let mut paths = Vec::with_capacity(all.len());
    for pl in &all {
        let row = ListBoxRow::new();
        let l = Label::new(Some(&pl.label));
        l.set_halign(Align::Start);
        if pl.removable {
            l.add_css_class("removable");
        }
        row.set_child(Some(&l));
        c.places_list.append(&row);
        paths.push(pl.path.clone());
    }
    c.st.borrow_mut().places = paths;
}

// ---- helpers ------------------------------------------------------
fn install_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        "window{background:#1c1c22;color:#e6e6ec}\
         .path{font-family:monospace}\
         .places row{padding:6px 8px}\
         .places .removable{color:#63d2d6}\
         list.files row{padding:4px 6px;border-bottom:1px solid #23232b}\
         .dim{color:#9a9aa6;font-size:11px}",
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn strip(spacing: i32) -> GtkBox {
    let b = GtkBox::new(Orientation::Horizontal, spacing);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.set_margin_start(6);
    b.set_margin_end(6);
    b
}

fn clear(list: &ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn file_row(e: &Entry) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hb = GtkBox::new(Orientation::Horizontal, 8);
    let icon = Image::from_icon_name(if e.is_dir {
        "folder-symbolic"
    } else if is_windows_exe(&e.path) {
        "application-x-executable-symbolic"
    } else {
        "text-x-generic-symbolic"
    });
    let name = Label::new(Some(&e.name));
    name.set_halign(Align::Start);
    name.set_hexpand(true);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    let sz = if e.is_dir { "—".to_string() } else { fmt_size(e.size) };
    let size = Label::new(Some(sz.as_str()));
    size.set_width_chars(10);
    size.set_xalign(1.0);
    size.add_css_class("dim");
    hb.append(&icon);
    hb.append(&name);
    hb.append(&size);
    row.set_child(Some(&hb));
    row
}

fn is_windows_exe(p: &Path) -> bool {
    p.extension()
        .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("msi"))
        .unwrap_or(false)
}

fn open_path(p: &Path) {
    if is_windows_exe(p) {
        let _ = Command::new("mavind-windows-apps").arg(p).spawn();
    } else {
        let _ = Command::new("xdg-open").arg(p).spawn();
    }
}

fn recursive_search(base: &Path, q: &str, max_depth: usize, cap: usize) -> Vec<Entry> {
    let ql = q.to_lowercase();
    let mut out = Vec::new();
    let mut stack = vec![(base.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let md = e.metadata().ok();
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            if name.to_lowercase().contains(&ql) {
                out.push(Entry {
                    path: e.path(),
                    name,
                    is_dir,
                    is_symlink: false,
                    size: md.as_ref().map(|m| m.len()).unwrap_or(0),
                    mtime: 0,
                });
                if out.len() >= cap {
                    return out;
                }
            }
            if is_dir && depth < max_depth {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    out
}

fn prompt<F: Fn(String) + 'static>(parent: &impl IsA<gtk4::Window>, title: &str, initial: &str, on_ok: F) {
    let win = gtk4::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(360)
        .build();
    let v = GtkBox::new(Orientation::Vertical, 8);
    v.set_margin_top(12);
    v.set_margin_bottom(12);
    v.set_margin_start(12);
    v.set_margin_end(12);
    let entry = GtkEntry::new();
    entry.set_text(initial);
    let h = GtkBox::new(Orientation::Horizontal, 8);
    h.set_halign(Align::End);
    let cancel = Button::with_label("Cancel");
    let ok = Button::with_label("OK");
    ok.add_css_class("suggested-action");
    h.append(&cancel);
    h.append(&ok);
    v.append(&entry);
    v.append(&h);
    win.set_child(Some(&v));

    let w2 = win.clone();
    cancel.connect_clicked(move |_| w2.close());

    // A plain closure that is `Clone` (all captures are Clone): callable directly.
    let fire = {
        let win = win.clone();
        let entry = entry.clone();
        let on_ok = Rc::new(on_ok);
        move || {
            (on_ok)(entry.text().to_string());
            win.close();
        }
    };
    let f2 = fire.clone();
    ok.connect_clicked(move |_| f2());
    entry.connect_activate(move |_| fire());
    win.present();
}

fn error_dialog(parent: &impl IsA<gtk4::Window>, msg: &str) {
    gtk4::AlertDialog::builder()
        .message("Operation failed")
        .detail(msg)
        .modal(true)
        .build()
        .show(Some(parent));
}

fn properties_dialog(parent: &impl IsA<gtk4::Window>, p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let md = std::fs::symlink_metadata(p).ok();
    let (size, capped) = if p.is_dir() {
        fsops::dir_size(p, 200_000)
    } else {
        (md.as_ref().map(|m| m.len()).unwrap_or(0), false)
    };
    let mode = md.as_ref().map(|m| m.permissions().mode() & 0o777).unwrap_or(0);

    let facts = [
        ("Name", p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
        ("Location", p.parent().map(|d| d.display().to_string()).unwrap_or_default()),
        ("Type", if p.is_dir() { "Folder".into() } else { "File".into() }),
        ("Size", format!("{}{}", fmt_size(size), if capped { " (partial)" } else { "" })),
        ("Permissions", format!("{mode:03o}")),
    ];
    let v = GtkBox::new(Orientation::Vertical, 6);
    v.set_margin_top(14);
    v.set_margin_bottom(14);
    v.set_margin_start(16);
    v.set_margin_end(16);
    for (k, val) in facts {
        let row = GtkBox::new(Orientation::Horizontal, 10);
        let kl = Label::new(Some(k));
        kl.set_width_chars(12);
        kl.set_xalign(0.0);
        kl.add_css_class("dim");
        let vl = Label::new(Some(&val));
        vl.set_xalign(0.0);
        vl.set_wrap(true);
        vl.set_selectable(true);
        row.append(&kl);
        row.append(&vl);
        v.append(&row);
    }
    gtk4::Window::builder()
        .title("Properties")
        .transient_for(parent)
        .modal(true)
        .child(&v)
        .build()
        .present();
}
