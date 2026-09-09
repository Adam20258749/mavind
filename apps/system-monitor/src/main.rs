//! Mavind System Monitor — shows real RAM / CPU / disk / process usage.
//! Every number is read live from /proc, /sys and statvfs (see proc.rs).
//! Spec §11: "Never fake performance statistics."

mod proc;

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Application, ApplicationWindow, Box as GtkBox, Label, LevelBar, ListBox,
    Orientation, PolicyType, ScrolledWindow, SelectionMode,
};
use proc::*;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "os.mavind.SystemMonitor";
const TICK_SECONDS: u32 = 2;
const MAX_ROWS: usize = 40;

struct Ui {
    header: Label,
    ram_bar: LevelBar,
    ram_lbl: Label,
    cpu_bar: LevelBar,
    cpu_lbl: Label,
    swap_bar: LevelBar,
    swap_lbl: Label,
    disks_box: GtkBox,
    proc_list: ListBox,
}

struct State {
    cpu_prev: CpuSample,
    procs: ProcSampler,
}

fn main() -> glib::ExitCode {
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-v")) {
        println!("mavind-system-monitor {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application) {
    let css = gtk4::CssProvider::new();
    css.load_from_string(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let root = GtkBox::new(Orientation::Vertical, 10);
    root.set_margin_top(14);
    root.set_margin_bottom(14);
    root.set_margin_start(14);
    root.set_margin_end(14);

    let header = Label::new(None);
    header.set_halign(Align::Start);
    header.add_css_class("dim");
    header.set_wrap(true);
    root.append(&header);

    let (ram_bar, ram_lbl, ram_row) = meter("Memory");
    let (cpu_bar, cpu_lbl, cpu_row) = meter("CPU");
    let (swap_bar, swap_lbl, swap_row) = meter("Swap (zram)");
    root.append(&cpu_row);
    root.append(&ram_row);
    root.append(&swap_row);

    let disks_title = Label::new(Some("Disks"));
    disks_title.set_halign(Align::Start);
    disks_title.add_css_class("section");
    root.append(&disks_title);
    let disks_box = GtkBox::new(Orientation::Vertical, 6);
    root.append(&disks_box);

    let proc_title = Label::new(Some("Processes  —  by CPU, then memory"));
    proc_title.set_halign(Align::Start);
    proc_title.add_css_class("section");
    root.append(&proc_title);

    let head = proc_row_widget("PID", "USER", "CPU%", "MEM", "S", "COMMAND", true);
    root.append(&head);

    let proc_list = ListBox::new();
    proc_list.set_selection_mode(SelectionMode::None);
    proc_list.add_css_class("procs");
    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&proc_list)
        .build();
    root.append(&scroller);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("System Monitor")
        .default_width(720)
        .default_height(620)
        .child(&root)
        .build();
    window.present();

    let ui = Rc::new(Ui {
        header,
        ram_bar,
        ram_lbl,
        cpu_bar,
        cpu_lbl,
        swap_bar,
        swap_lbl,
        disks_box,
        proc_list,
    });
    let state = Rc::new(RefCell::new(State {
        cpu_prev: CpuSample::read(),
        procs: ProcSampler::new(),
    }));

    let tick = {
        let ui = ui.clone();
        let state = state.clone();
        move || {
            refresh(&ui, &mut state.borrow_mut());
            glib::ControlFlow::Continue
        }
    };
    // prime once, then every TICK_SECONDS
    refresh(&ui, &mut state.borrow_mut());
    glib::timeout_add_seconds_local(TICK_SECONDS, tick);
}

fn meter(name: &str) -> (LevelBar, Label, GtkBox) {
    let row = GtkBox::new(Orientation::Vertical, 2);
    let top = GtkBox::new(Orientation::Horizontal, 8);
    let n = Label::new(Some(name));
    n.set_halign(Align::Start);
    n.set_width_chars(12);
    n.set_xalign(0.0);
    let v = Label::new(Some("—"));
    v.set_halign(Align::End);
    v.set_hexpand(true);
    v.add_css_class("dim");
    top.append(&n);
    top.append(&v);
    let bar = LevelBar::new();
    bar.set_min_value(0.0);
    bar.set_max_value(1.0);
    bar.add_offset_value("low", 0.75);
    bar.add_offset_value("high", 0.9);
    bar.set_height_request(10);
    row.append(&top);
    row.append(&bar);
    (bar, v, row)
}

fn refresh(ui: &Ui, st: &mut State) {
    // ---- header -----------------------------------------------------
    let (l1, l5, l15) = loadavg();
    ui.header.set_text(&format!(
        "{}   ·   {} cores   ·   load {l1:.2} {l5:.2} {l15:.2}   ·   up {}",
        cpu_model(),
        ncpu(),
        fmt_duration(uptime_secs()),
    ));

    // ---- CPU ------------------------------------------------------
    let cur = CpuSample::read();
    let busy = cur.busy_fraction(&st.cpu_prev);
    let per = cur.per_core_busy(&st.cpu_prev);
    st.cpu_prev = cur;
    ui.cpu_bar.set_value(busy);
    let per_txt: String = per
        .iter()
        .map(|f| format!("{:>3.0}%", f * 100.0))
        .collect::<Vec<_>>()
        .join(" ");
    ui.cpu_lbl.set_text(&format!("{:.0}%   [{per_txt}]", busy * 100.0));

    // ---- memory --------------------------------------------------
    let m = MemInfo::read();
    ui.ram_bar.set_value(m.used_fraction());
    ui.ram_lbl.set_text(&format!(
        "{} / {}   ({:.0}%)   ·   cache {}",
        fmt_kb(m.used_kb()),
        fmt_kb(m.total_kb),
        m.used_fraction() * 100.0,
        fmt_kb(m.cached_kb + m.buffers_kb),
    ));
    if m.swap_total_kb == 0 {
        ui.swap_bar.set_value(0.0);
        ui.swap_lbl.set_text("no swap configured");
    } else {
        let frac = m.swap_used_kb() as f64 / m.swap_total_kb as f64;
        ui.swap_bar.set_value(frac);
        ui.swap_lbl.set_text(&format!(
            "{} / {}   ·   zram cost in RAM {}",
            fmt_kb(m.swap_used_kb()),
            fmt_kb(m.swap_total_kb),
            fmt_kb(m.zram_used_kb),
        ));
    }

    // ---- disks -------------------------------------------------
    while let Some(c) = ui.disks_box.first_child() {
        ui.disks_box.remove(&c);
    }
    for d in disks() {
        let (bar, lbl, row) = meter(&d.mount);
        bar.set_value(d.used_fraction());
        lbl.set_text(&format!(
            "{} / {}   ({:.0}%)   ·   {} on {}",
            fmt_bytes(d.used_bytes()),
            fmt_bytes(d.total_bytes),
            d.used_fraction() * 100.0,
            d.fstype,
            d.source,
        ));
        ui.disks_box.append(&row);
    }

    // ---- processes -------------------------------------------
    let (mut rows, cpu) = st.procs.sample();
    rows.sort_by(|a, b| {
        let ca = cpu.get(&a.pid).copied().unwrap_or(0.0);
        let cb = cpu.get(&b.pid).copied().unwrap_or(0.0);
        cb.partial_cmp(&ca)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.rss_kb.cmp(&a.rss_kb))
    });

    while let Some(c) = ui.proc_list.first_child() {
        ui.proc_list.remove(&c);
    }
    for r in rows.iter().take(MAX_ROWS) {
        let c = cpu.get(&r.pid).copied().unwrap_or(0.0);
        let w = proc_row_widget(
            &r.pid.to_string(),
            &r.user,
            &format!("{c:.1}"),
            &fmt_kb(r.rss_kb),
            &r.state.to_string(),
            &r.comm,
            false,
        );
        ui.proc_list.append(&w);
    }
}

#[allow(clippy::too_many_arguments)]
fn proc_row_widget(
    pid: &str,
    user: &str,
    cpu: &str,
    mem: &str,
    state: &str,
    comm: &str,
    header: bool,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_start(4);
    row.set_margin_end(4);
    let mk = |t: &str, chars: i32, end: bool| {
        let l = Label::new(Some(t));
        l.set_width_chars(chars);
        l.set_xalign(if end { 1.0 } else { 0.0 });
        l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        if header {
            l.add_css_class("th");
        }
        l
    };
    row.append(&mk(pid, 7, true));
    row.append(&mk(user, 10, false));
    row.append(&mk(cpu, 6, true));
    row.append(&mk(mem, 10, true));
    row.append(&mk(state, 2, false));
    let c = mk(comm, 0, false);
    c.set_hexpand(true);
    row.append(&c);
    if header {
        row.add_css_class("th-row");
    }
    row
}
