//! Power menu popover for the panel.

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Orientation, Popover, Widget, Window};
use std::process::Command;

#[derive(Clone, Copy)]
enum Action {
    Lock,
    Logout,
    Suspend,
    Reboot,
    Poweroff,
}

impl Action {
    fn run(self) {
        let spawn = |cmd: &str, args: &[&str]| {
            let _ = Command::new(cmd).args(args).spawn();
        };
        match self {
            Action::Lock => spawn("mavind-shell", &["--lock"]),
            // Ending the compositor ends the session (greetd/getty respawns login).
            Action::Logout => spawn("labwc", &["--exit"]),
            Action::Suspend => spawn("systemctl", &["suspend"]),
            Action::Reboot => spawn("systemctl", &["reboot"]),
            Action::Poweroff => spawn("systemctl", &["poweroff"]),
        }
    }
}

pub fn show_menu<W: IsA<Widget>>(_win: &impl IsA<Window>, anchor: &W) {
    let menu = GtkBox::new(Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let pop = Popover::new();
    pop.set_child(Some(&menu));
    pop.set_parent(anchor);
    pop.set_autohide(true);

    for (label, action) in [
        ("Lock", Action::Lock),
        ("Log Out", Action::Logout),
        ("Suspend", Action::Suspend),
        ("Restart", Action::Reboot),
        ("Shut Down", Action::Poweroff),
    ] {
        let b = Button::with_label(label);
        b.set_halign(Align::Fill);
        b.set_has_frame(false);
        let pop = pop.clone();
        b.connect_clicked(move |_| {
            pop.popdown();
            action.run();
        });
        menu.append(&b);
    }

    pop.popup();
}
