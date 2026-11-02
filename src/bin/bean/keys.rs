//! Keybinds: every control can be bound to one or more keys, mouse buttons or the scroll wheel,
//! set in `bean_settings.txt` as `jump = Space, WheelDown`.

use macroquad::prelude::*;

/// One physical input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
    WheelUp,
    WheelDown,
}

macro_rules! key_names {
    ($($k:ident),* $(,)?) => {
        const KEY_NAMES: &[(&str, KeyCode)] = &[$((stringify!($k), KeyCode::$k)),*];
    };
}

key_names!(
    Space, Apostrophe, Comma, Minus, Period, Slash, Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7,
    Key8, Key9, Semicolon, Equal, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V,
    W, X, Y, Z, LeftBracket, Backslash, RightBracket, GraveAccent, Escape, Enter, Tab, Backspace,
    Insert, Delete, Right, Left, Down, Up, PageUp, PageDown, Home, End, CapsLock, F1, F2, F3, F4,
    F5, F6, F7, F8, F9, F10, F11, F12, Kp0, Kp1, Kp2, Kp3, Kp4, Kp5, Kp6, Kp7, Kp8, Kp9,
    KpDecimal, KpDivide, KpMultiply, KpSubtract, KpAdd, KpEnter, LeftShift, LeftControl, LeftAlt,
    RightShift, RightControl, RightAlt,
);

/// Short aliases people are likely to type.
const ALIASES: &[(&str, &str)] = &[
    ("Shift", "LeftShift"),
    ("Ctrl", "LeftControl"),
    ("Control", "LeftControl"),
    ("Alt", "LeftAlt"),
    ("Esc", "Escape"),
    ("Return", "Enter"),
    ("[", "LeftBracket"),
    ("]", "RightBracket"),
    ("`", "GraveAccent"),
    ("Tilde", "GraveAccent"),
];

impl Bind {
    /// Parses a name like `Space`, `E`, `1`, `Mouse1`, `WheelDown` (case doesn't matter).
    pub fn parse(name: &str) -> Option<Bind> {
        let mut n = name.trim();
        if let Some((_, to)) = ALIASES.iter().find(|(a, _)| a.eq_ignore_ascii_case(n)) {
            n = to;
        }
        let low = n.to_ascii_lowercase();
        match low.as_str() {
            "mouse1" | "lmb" | "mouseleft" => return Some(Bind::Mouse(MouseButton::Left)),
            "mouse2" | "rmb" | "mouseright" => return Some(Bind::Mouse(MouseButton::Right)),
            "mouse3" | "mmb" | "mousemiddle" => return Some(Bind::Mouse(MouseButton::Middle)),
            "wheelup" | "mwheelup" => return Some(Bind::WheelUp),
            "wheeldown" | "mwheeldown" => return Some(Bind::WheelDown),
            _ => {}
        }
        // A bare digit means the number row key.
        if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() {
            return Bind::parse(&format!("Key{n}"));
        }
        KEY_NAMES.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|&(_, k)| Bind::Key(k))
    }

    pub fn name(&self) -> String {
        match self {
            Bind::Key(k) => {
                let n = KEY_NAMES.iter().find(|(_, c)| c == k).map(|(n, _)| *n).unwrap_or("?");
                n.strip_prefix("Key").filter(|d| d.len() == 1).unwrap_or(n).to_string()
            }
            Bind::Mouse(MouseButton::Left) => "Mouse1".into(),
            Bind::Mouse(MouseButton::Right) => "Mouse2".into(),
            Bind::Mouse(MouseButton::Middle) => "Mouse3".into(),
            Bind::Mouse(_) => "Mouse?".into(),
            Bind::WheelUp => "WheelUp".into(),
            Bind::WheelDown => "WheelDown".into(),
        }
    }

    /// Held this frame. A wheel notch counts as held for the frame it happens.
    fn down(&self, wheel: f32) -> bool {
        match *self {
            Bind::Key(k) => is_key_down(k),
            Bind::Mouse(b) => is_mouse_button_down(b),
            Bind::WheelUp => wheel > 0.0,
            Bind::WheelDown => wheel < 0.0,
        }
    }

    /// Went down this frame.
    fn pressed(&self, wheel: f32) -> bool {
        match *self {
            Bind::Key(k) => is_key_pressed(k),
            Bind::Mouse(b) => is_mouse_button_pressed(b),
            Bind::WheelUp => wheel > 0.0,
            Bind::WheelDown => wheel < 0.0,
        }
    }
}

/// Every bindable control, with its settings name, description and default binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Crouch,
    HandCannon,
    RocketPunch,
    SeismicSlam,
    PowerBlock,
    Melee,
    ThirdPerson,
    SlowMo,
    Reset,
    Dummies,
    Record,
    SlamPreview,
    SensDown,
    SensUp,
    ReleaseMouse,
}

pub const ACTS: &[(Act, &str, &str, &str)] = &[
    (Act::Forward, "forward", "move forward", "W"),
    (Act::Back, "back", "move back", "S"),
    (Act::Left, "left", "move left", "A"),
    (Act::Right, "right", "move right", "D"),
    (Act::Jump, "jump", "jump", "Space"),
    (Act::Crouch, "crouch", "crouch", "LeftControl"),
    (Act::HandCannon, "hand_cannon", "Hand Cannon", "Mouse1"),
    (Act::RocketPunch, "rocket_punch", "Rocket Punch (hold)", "Mouse2"),
    (Act::SeismicSlam, "seismic_slam", "Seismic Slam", "LeftShift"),
    (Act::PowerBlock, "power_block", "Power Block", "E"),
    (Act::Melee, "melee", "quick melee", "V"),
    (Act::ThirdPerson, "third_person", "third person", "Tab"),
    (Act::SlowMo, "slow_mo", "slow motion", "T"),
    (Act::Reset, "reset", "reset", "R"),
    (Act::Dummies, "dummies", "respawn dummies", "H"),
    (Act::Record, "record", "record", "F"),
    (Act::SlamPreview, "slam_preview", "show where a slam would land", "G"),
    (Act::SensDown, "sens_down", "sensitivity down", "LeftBracket"),
    (Act::SensUp, "sens_up", "sensitivity up", "RightBracket"),
    (Act::ReleaseMouse, "release_mouse", "free the mouse", "Escape"),
];

/// The binds for every control.
#[derive(Clone, Debug)]
pub struct Keybinds {
    binds: Vec<Vec<Bind>>,
    /// Scroll this frame (set by `frame`).
    wheel: f32,
}

impl Default for Keybinds {
    fn default() -> Self {
        let binds = ACTS.iter().map(|(_, _, _, d)| parse_list(d).0).collect();
        Keybinds { binds, wheel: 0.0 }
    }
}

/// Parses `Space, WheelDown`; returns the binds and any names it didn't understand.
pub fn parse_list(s: &str) -> (Vec<Bind>, Vec<String>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for part in s.split([',', ' ']).map(str::trim).filter(|p| !p.is_empty()) {
        if part.eq_ignore_ascii_case("none") {
            continue;
        }
        match Bind::parse(part) {
            Some(b) => out.push(b),
            None => bad.push(part.to_string()),
        }
    }
    (out, bad)
}

impl Keybinds {
    /// Applies a `name = binds` setting. Returns `None` if `name` isn't a control, otherwise
    /// the names that couldn't be read.
    pub fn set(&mut self, name: &str, value: &str) -> Option<Vec<String>> {
        let i = ACTS.iter().position(|(_, n, _, _)| *n == name)?;
        let (binds, bad) = parse_list(value);
        self.binds[i] = binds;
        Some(bad)
    }

    fn of(&self, a: Act) -> &[Bind] {
        &self.binds[ACTS.iter().position(|(x, ..)| *x == a).unwrap()]
    }

    /// Call once per frame before reading input.
    pub fn frame(&mut self) {
        self.wheel = mouse_wheel().1;
    }

    pub fn down(&self, a: Act) -> bool {
        self.of(a).iter().any(|b| b.down(self.wheel))
    }

    pub fn pressed(&self, a: Act) -> bool {
        self.of(a).iter().any(|b| b.pressed(self.wheel))
    }

    /// `pos` minus `neg` as -1, 0 or 1.
    pub fn axis(&self, pos: Act, neg: Act) -> f32 {
        (self.down(pos) as i32 - self.down(neg) as i32) as f32
    }

    /// Short label for the HUD, e.g. `Space/WheelDown`.
    pub fn label(&self, a: Act) -> String {
        let b = self.of(a);
        if b.is_empty() {
            "unbound".into()
        } else {
            b.iter().map(Bind::name).collect::<Vec<_>>().join("/")
        }
    }

    /// The settings-file lines for every control.
    pub fn to_lines(&self) -> String {
        let mut s = String::from(
            "\n# Keybinds: one or more of a key name (W, Space, LeftShift, LeftControl, Tab, F1, 1, Kp0, ...),\n\
             # Mouse1 / Mouse2 / Mouse3, WheelUp / WheelDown. Separate several with commas; `none` unbinds.\n",
        );
        for (i, (_, name, desc, _)) in ACTS.iter().enumerate() {
            let v = if self.binds[i].is_empty() {
                "none".to_string()
            } else {
                self.binds[i].iter().map(Bind::name).collect::<Vec<_>>().join(", ")
            };
            s += &format!("{name:<14} = {v:<22} # {desc}\n");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for b in ["Space", "W", "1", "Kp5", "LeftShift", "Mouse2", "WheelDown", "F12", "GraveAccent"] {
            let p = Bind::parse(b).unwrap();
            assert_eq!(Bind::parse(&p.name()), Some(p), "{b}");
        }
        assert_eq!(Bind::parse("shift"), Some(Bind::Key(KeyCode::LeftShift)));
        assert_eq!(Bind::parse("lmb"), Some(Bind::Mouse(MouseButton::Left)));
        assert_eq!(Bind::parse("nope"), None);
    }

    #[test]
    fn settings_lines_round_trip() {
        let mut k = Keybinds::default();
        assert_eq!(k.set("jump", "Space, WheelDown"), Some(vec![]));
        assert_eq!(k.set("melee", "none"), Some(vec![]));
        assert_eq!(k.set("power_block", "Q, bogus"), Some(vec!["bogus".to_string()]));
        assert_eq!(k.set("not_a_control", "Q"), None);
        let mut back = Keybinds::default();
        for line in k.to_lines().lines() {
            let line = line.split('#').next().unwrap();
            if let Some((n, v)) = line.split_once('=') {
                back.set(n.trim(), v);
            }
        }
        assert_eq!(back.binds, k.binds);
        assert_eq!(back.label(Act::Jump), "Space/WheelDown");
        assert_eq!(back.label(Act::Melee), "unbound");
    }
}
