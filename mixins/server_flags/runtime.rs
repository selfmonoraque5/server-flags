// Server Flags for Horizon: a small flag beside each saved server in the
// Multiplayer list for the country the server is hosted in, and the country's
// name when you hover it. Inspired by Khajiitos' Server Country Flags,
// written from scratch for Horizon (none of its code or images: the flags are
// drawn here as 12 x 8 pixel art, and a country without one shows its code).
// This file is both the mod's Cargo library (its tests) and the module the
// bundle adds to the game.
//
// Where a server is: its address is resolved to an IP here, and the IP sent to
// https://api.country.is (open source, no key, nothing else sent), once per
// server per session (a lookup that couldn't get through is retried a minute
// later, three times at most), only while the setting is on. Local network
// addresses are never sent. A server with no country to show gets a "?" and a
// line in the game log saying why.
use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "server_flags";
pub const NAME: &str = "Server Flags";
pub const VERSION: &str = "1.0.1";
pub const DESCRIPTION: &str = "A flag beside each server in the Multiplayer list for the country it is hosted in. Looks servers up at api.country.is.";

/// The service a server's IP is sent to.
pub const SERVICE: &str = "https://api.country.is/";

/// A flag's size in GUI pixels, and the space after the server name.
pub const FLAG_W: f32 = 12.0;
pub const FLAG_H: f32 = 8.0;
pub const GAP: f32 = 4.0;

// ---------------------------------------------------------------- settings

pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub choices: &'static [(u8, &'static str)],
    pub default: u8,
}

const ON_OFF: &[(u8, &str)] = &[(0, "OFF"), (1, "ON")];
const PLACE: &[(u8, &str)] = &[(0, "After the name"), (1, "On the icon")];

pub const OPTS: [Opt; 5] = [
    Opt { key: "lookup", label: "Look up server locations", choices: ON_OFF, default: 1 },
    Opt { key: "place", label: "Flag position", choices: PLACE, default: 0 },
    Opt { key: "tooltip", label: "Country name on hover", choices: ON_OFF, default: 1 },
    Opt { key: "codes", label: "Code when no flag", choices: ON_OFF, default: 1 },
    Opt { key: "unknown", label: "? when the location is unknown", choices: ON_OFF, default: 1 },
];

pub mod o {
    pub const LOOKUP: usize = 0;
    pub const PLACE: usize = 1;
    pub const TOOLTIP: usize = 2;
    pub const CODES: usize = 3;
    pub const UNKNOWN: usize = 4;
}

pub const RESET_LABEL: &str = "Reset to defaults";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    values: [u8; OPTS.len()],
}

impl Default for Settings {
    fn default() -> Self {
        Self { values: OPTS.map(|opt| opt.default) }
    }
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut s = Self::default();
        for (i, opt) in OPTS.iter().enumerate() {
            let value = get(opt.key).and_then(|v| v.trim().parse::<u8>().ok());
            if let Some(v) = value
                && opt.choices.iter().any(|c| c.0 == v)
            {
                s.values[i] = v;
            }
        }
        s
    }

    pub fn get(&self, i: usize) -> u8 {
        self.values[i]
    }

    pub fn on(&self, i: usize) -> bool {
        self.values[i] != 0
    }

    pub fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = OPTS
            .iter()
            .zip(self.values)
            .map(|(opt, v)| {
                let name = opt.choices.iter().find(|c| c.0 == v).map_or("?", |c| c.1);
                format!("{}: {name}", opt.label)
            })
            .collect();
        labels.push(RESET_LABEL.into());
        labels
    }

    pub fn click(&mut self, i: usize) -> Vec<(&'static str, String)> {
        if i == OPTS.len() {
            *self = Self::default();
            return OPTS.iter().zip(self.values).map(|(opt, v)| (opt.key, v.to_string())).collect();
        }
        let Some(opt) = OPTS.get(i) else {
            return Vec::new();
        };
        let at = opt.choices.iter().position(|c| c.0 == self.values[i]).unwrap_or(0);
        self.values[i] = opt.choices[(at + 1) % opt.choices.len()].0;
        vec![(opt.key, self.values[i].to_string())]
    }
}

static LOADED: AtomicBool = AtomicBool::new(false);
static VALUES: [AtomicU8; OPTS.len()] = [const { AtomicU8::new(0) }; OPTS.len()];
/// Whether the mod is on in Options > Mods (the title screen can't ask the game).
static ON: AtomicBool = AtomicBool::new(true);

pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    if !LOADED.load(Ordering::Acquire) {
        set_settings(Settings::read(get));
    }
    current()
}

pub fn set_settings(settings: Settings) {
    for (slot, v) in VALUES.iter().zip(settings.values) {
        slot.store(v, Ordering::Relaxed);
    }
    LOADED.store(true, Ordering::Release);
}

pub fn current() -> Settings {
    if !LOADED.load(Ordering::Acquire) {
        return Settings::default();
    }
    let mut s = Settings::default();
    for (v, slot) in s.values.iter_mut().zip(&VALUES) {
        *v = slot.load(Ordering::Relaxed);
    }
    s
}

pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

// --------------------------------------------------------------- addresses

/// The host and port of a server list address: `host`, `host:port`,
/// `1.2.3.4:25566`, `[::1]:25565` or a bare IPv6 address.
pub fn host_port(address: &str) -> Option<(String, u16)> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = after.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(25565);
        return Some((host.to_owned(), port));
    }
    match address.matches(':').count() {
        0 => Some((address.to_owned(), 25565)),
        1 => {
            let (host, port) = address.split_once(':')?;
            Some((host.to_owned(), port.parse().unwrap_or(25565)))
        }
        // A bare IPv6 address.
        _ => Some((address.to_owned(), 25565)),
    }
}

/// An address that never leaves the computer's network: loopback, private,
/// link-local and the like are not looked up.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64.0.0/10, carrier-grade NAT and Tailscale.
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xFE00) == 0xFC00
                || (v6.segments()[0] & 0xFFC0) == 0xFE80
        }
    }
}

/// The two-letter code country.is answers with (`{"ip":"…","country":"DE"}`).
pub fn parse_answer(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = json.get("country")?.as_str()?.trim().to_ascii_uppercase();
    (code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase())).then_some(code)
}

// ----------------------------------------------------------------- lookups

/// Where a server's lookup stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Pending,
    /// On the local network: nothing was sent.
    Local,
    Country(String),
    /// The service knows no single country for the IP: anycast, as DDoS
    /// protection (Cloudflare, TCPShield) gives big servers. Final.
    NoCountry,
    /// The service couldn't be reached: an ad blocker or DNS filter blocking
    /// it (AdGuard, Pi-hole), a firewall, or no internet. Retried.
    Blocked,
    /// The server's own address didn't resolve. Retried.
    Unresolved,
}

impl Place {
    /// Whether looking again may change the answer.
    pub fn retried(&self) -> bool {
        matches!(self, Place::Blocked | Place::Unresolved)
    }
}

/// How many times a failed lookup is retried, and how long apart.
pub const RETRIES: u8 = 3;
pub const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

/// What the service answered: an HTTP status and body, or no answer at all.
pub enum Answer {
    Status(u16, String),
    Unreachable,
}

/// A server's place from the service's answer.
pub fn place_of(answer: Answer) -> Place {
    match answer {
        Answer::Status(200, body) => parse_answer(&body).map_or(Place::NoCountry, Place::Country),
        // country.is answers 404 for an IP it has no country for.
        Answer::Status(404, _) => Place::NoCountry,
        // Rate limited or the service is down: try again later.
        Answer::Status(_, _) | Answer::Unreachable => Place::Blocked,
    }
}

/// The game log's line for a lookup that found no country, once per server.
pub fn log_line(address: &str, place: &Place) -> Option<String> {
    let why = match place {
        Place::NoCountry => "has no single country (anycast or DDoS protection): no flag".to_owned(),
        Place::Blocked => "can't reach api.country.is: allow api.country.is in your ad blocker (AdGuard, Pi-hole...) or firewall, or check your connection; retrying in a minute".to_owned(),
        Place::Unresolved => "doesn't resolve (wrong address or a DNS problem); retrying in a minute".to_owned(),
        _ => return None,
    };
    Some(format!("{NAME}: {address} {why}"))
}

struct Entry {
    place: Place,
    /// Lookups made so far, and when the next may be made.
    tries: u8,
    next: std::time::Instant,
    logged: bool,
}

fn places() -> &'static Mutex<HashMap<String, Entry>> {
    static PLACES: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    PLACES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Where server `address` is, starting its lookup the first time it is asked
/// (in the background) while lookups are on, and again a minute after a
/// lookup that may succeed later (at most `RETRIES` times).
pub fn place(address: &str) -> Option<Place> {
    let mut known = places().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = std::time::Instant::now();
    if let Some(e) = known.get(address)
        && !(e.place.retried() && e.tries <= RETRIES && now >= e.next)
    {
        return Some(e.place.clone());
    }
    if !current().on(o::LOOKUP) {
        return known.get(address).map(|e| e.place.clone());
    }
    let tries = known.get(address).map_or(0, |e| e.tries) + 1;
    let logged = known.get(address).is_some_and(|e| e.logged);
    known.insert(
        address.to_owned(),
        Entry { place: Place::Pending, tries, next: now + RETRY_AFTER, logged },
    );
    let key = address.to_owned();
    let spawned = std::thread::Builder::new()
        .name("server-flags".into())
        .spawn(move || {
            let found = look_up(&key);
            let mut known = places().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(e) = known.get_mut(&key) {
                if !e.logged
                    && let Some(line) = log_line(&key, &found)
                {
                    eprintln!("{line}");
                    e.logged = true;
                }
                e.place = found;
                e.next = std::time::Instant::now() + RETRY_AFTER;
            }
        });
    if spawned.is_err()
        && let Some(e) = known.get_mut(address)
    {
        e.place = Place::Blocked;
    }
    Some(Place::Pending)
}

/// Resolves the address and asks the service where its IP is.
fn look_up(address: &str) -> Place {
    let Some((host, port)) = host_port(address) else {
        return Place::Unresolved;
    };
    let Ok(mut addrs) = (host.as_str(), port).to_socket_addrs() else {
        return Place::Unresolved;
    };
    let Some(ip) = addrs.next().map(|a| a.ip()) else {
        return Place::Unresolved;
    };
    if is_local(ip) {
        return Place::Local;
    }
    let answer = match ureq::get(&format!("{SERVICE}{ip}"))
        .timeout(std::time::Duration::from_secs(6))
        .call()
    {
        Ok(r) => Answer::Status(r.status(), r.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(code, r)) => Answer::Status(code, r.into_string().unwrap_or_default()),
        Err(ureq::Error::Transport(_)) => Answer::Unreachable,
    };
    place_of(answer)
}

// ------------------------------------------------------------------- flags

/// One rectangle of a flag on its 12 x 8 grid, and its colour.
pub type Cell = (u8, u8, u8, u8, [u8; 3]);

const RED: [u8; 3] = [206, 17, 38];
const WHITE: [u8; 3] = [245, 245, 245];
const NAVY: [u8; 3] = [1, 33, 105];
const YELLOW: [u8; 3] = [252, 209, 22];
const BLACK: [u8; 3] = [20, 20, 20];
const ORANGE: [u8; 3] = [255, 136, 62];
const SKY: [u8; 3] = [116, 172, 223];

fn horizontal(colors: &[[u8; 3]]) -> Vec<Cell> {
    let h = 8 / colors.len() as u8;
    colors.iter().enumerate().map(|(i, c)| (0, i as u8 * h, 12, h, *c)).collect()
}

fn vertical(colors: &[[u8; 3]]) -> Vec<Cell> {
    let w = 12 / colors.len() as u8;
    colors.iter().enumerate().map(|(i, c)| (i as u8 * w, 0, w, 8, *c)).collect()
}

/// A Nordic cross: the field, the cross, and an inner cross (Norway, Iceland).
fn nordic(field: [u8; 3], cross: [u8; 3], inner: Option<[u8; 3]>) -> Vec<Cell> {
    match inner {
        None => vec![(0, 0, 12, 8, field), (3, 0, 2, 8, cross), (0, 3, 12, 2, cross)],
        // A wider cross with the inner one down its middle.
        Some(inner) => vec![
            (0, 0, 12, 8, field),
            (2, 0, 4, 8, cross),
            (0, 2, 12, 4, cross),
            (3, 0, 2, 8, inner),
            (0, 3, 12, 2, inner),
        ],
    }
}

/// The flag of country `code` as rectangles, if it is drawn here.
pub fn flag(code: &str) -> Option<Vec<Cell>> {
    Some(match code {
        // Horizontal bands.
        "DE" => horizontal(&[BLACK, RED, [255, 206, 0]]),
        "NL" => horizontal(&[[174, 28, 40], WHITE, [33, 70, 139]]),
        "RU" => horizontal(&[WHITE, [0, 57, 166], [213, 43, 30]]),
        "AT" => horizontal(&[[237, 41, 57], WHITE, [237, 41, 57]]),
        "HU" => horizontal(&[[205, 42, 62], WHITE, [67, 111, 77]]),
        "BG" => horizontal(&[WHITE, [0, 150, 110], [214, 38, 18]]),
        "EE" => horizontal(&[[0, 114, 206], BLACK, WHITE]),
        "LT" => horizontal(&[[253, 185, 19], [0, 106, 68], [193, 39, 45]]),
        "LU" => horizontal(&[[237, 41, 57], WHITE, [0, 161, 222]]),
        "LV" => vec![(0, 0, 12, 8, [158, 48, 57]), (0, 3, 12, 2, WHITE)],
        "UA" => horizontal(&[[0, 87, 183], [255, 215, 0]]),
        "PL" => horizontal(&[WHITE, [220, 20, 60]]),
        "ID" | "MC" => horizontal(&[RED, WHITE]),
        "RS" => horizontal(&[[198, 54, 60], [12, 64, 118], WHITE]),
        "AM" => horizontal(&[[217, 0, 18], [0, 51, 160], [242, 168, 0]]),
        "GA" => horizontal(&[[0, 158, 96], [252, 209, 22], [58, 117, 196]]),
        "EG" | "YE" | "IQ" | "SY" => horizontal(&[RED, WHITE, BLACK]),
        "CO" => vec![(0, 0, 12, 4, YELLOW), (0, 4, 12, 2, NAVY), (0, 6, 12, 2, RED)],
        "ES" => vec![(0, 0, 12, 8, [198, 11, 30]), (0, 2, 12, 4, [255, 196, 0])],
        "TH" => vec![
            (0, 0, 12, 8, [165, 25, 49]),
            (0, 1, 12, 6, WHITE),
            (0, 3, 12, 2, [45, 42, 74]),
        ],
        "AR" => vec![
            (0, 0, 12, 8, SKY),
            (0, 3, 12, 2, WHITE),
            (5, 3, 2, 2, [246, 180, 14]),
        ],
        "IN" => vec![
            (0, 0, 12, 3, [255, 153, 51]),
            (0, 3, 12, 2, WHITE),
            (0, 5, 12, 3, [19, 136, 8]),
            (5, 3, 2, 2, [0, 0, 128]),
        ],
        // Vertical bands.
        "FR" => vertical(&[[0, 85, 164], WHITE, [239, 65, 53]]),
        "IT" => vertical(&[[0, 146, 70], WHITE, [206, 43, 55]]),
        "IE" => vertical(&[[22, 155, 98], WHITE, ORANGE]),
        "BE" => vertical(&[BLACK, [253, 218, 36], [239, 51, 64]]),
        "RO" => vertical(&[[0, 43, 127], [252, 209, 22], [206, 17, 38]]),
        "MX" => vec![
            (0, 0, 4, 8, [0, 104, 71]),
            (4, 0, 4, 8, WHITE),
            (8, 0, 4, 8, [206, 17, 38]),
            (5, 3, 2, 2, [140, 90, 40]),
        ],
        "NG" => vertical(&[[0, 135, 81], WHITE, [0, 135, 81]]),
        "PE" => vertical(&[[217, 16, 35], WHITE, [217, 16, 35]]),
        "CA" => vec![
            (0, 0, 3, 8, [216, 6, 33]),
            (3, 0, 6, 8, WHITE),
            (9, 0, 3, 8, [216, 6, 33]),
            (5, 2, 2, 4, [216, 6, 33]),
            (4, 3, 4, 2, [216, 6, 33]),
        ],
        // Nordic crosses.
        "SE" => nordic([0, 106, 167], [254, 204, 0], None),
        "FI" => nordic(WHITE, [0, 47, 108], None),
        "DK" => nordic([200, 16, 46], WHITE, None),
        "NO" => nordic([186, 12, 47], WHITE, Some([0, 32, 91])),
        "IS" => nordic([2, 82, 156], WHITE, Some([220, 30, 53])),
        // A disc on a field.
        "JP" => vec![(0, 0, 12, 8, WHITE), (5, 2, 2, 4, [188, 0, 45]), (4, 3, 4, 2, [188, 0, 45])],
        "BD" => vec![(0, 0, 12, 8, [0, 106, 78]), (4, 2, 2, 4, [244, 42, 65]), (3, 3, 4, 2, [244, 42, 65])],
        "KR" => vec![
            (0, 0, 12, 8, WHITE),
            (5, 2, 2, 2, [205, 46, 58]),
            (5, 4, 2, 2, [0, 71, 160]),
            (1, 1, 2, 1, BLACK),
            (9, 1, 2, 1, BLACK),
            (1, 6, 2, 1, BLACK),
            (9, 6, 2, 1, BLACK),
        ],
        // Crosses, stars and crescents.
        "CH" => vec![(0, 0, 12, 8, [218, 41, 28]), (5, 1, 2, 6, WHITE), (3, 3, 6, 2, WHITE)],
        "GB" => vec![
            (0, 0, 12, 8, NAVY),
            (0, 0, 2, 1, WHITE),
            (2, 1, 2, 1, WHITE),
            (8, 6, 2, 1, WHITE),
            (10, 7, 2, 1, WHITE),
            (10, 0, 2, 1, WHITE),
            (8, 1, 2, 1, WHITE),
            (2, 6, 2, 1, WHITE),
            (0, 7, 2, 1, WHITE),
            (4, 0, 4, 8, WHITE),
            (0, 3, 12, 2, WHITE),
            (5, 0, 2, 8, [200, 16, 46]),
            (0, 3, 12, 2, [200, 16, 46]),
        ],
        "US" => {
            let mut cells: Vec<Cell> = (0..8)
                .map(|row| (0, row, 12, 1, if row % 2 == 0 { [178, 34, 52] } else { WHITE }))
                .collect();
            cells.push((0, 0, 5, 4, [60, 59, 110]));
            cells.extend([(1, 1, 1, 1, WHITE), (3, 1, 1, 1, WHITE), (2, 2, 1, 1, WHITE)]);
            cells
        }
        "AU" | "NZ" => {
            let star = if code == "NZ" { [204, 20, 43] } else { WHITE };
            vec![
                (0, 0, 12, 8, NAVY),
                (0, 0, 6, 4, NAVY),
                (2, 0, 2, 4, WHITE),
                (0, 1, 6, 2, WHITE),
                (2, 0, 1, 4, [200, 16, 46]),
                (0, 2, 6, 1, [200, 16, 46]),
                (9, 1, 1, 1, star),
                (8, 3, 1, 1, star),
                (10, 4, 1, 1, star),
                (9, 6, 1, 1, star),
                (2, 6, 1, 1, WHITE),
            ]
        }
        "CN" => vec![(0, 0, 12, 8, [222, 41, 16]), (1, 1, 2, 2, [255, 222, 0]), (4, 1, 1, 1, [255, 222, 0]), (4, 3, 1, 1, [255, 222, 0])],
        "VN" => vec![(0, 0, 12, 8, [218, 37, 29]), (5, 2, 2, 4, [255, 255, 0]), (4, 3, 4, 1, [255, 255, 0])],
        "TR" => vec![
            (0, 0, 12, 8, [227, 10, 23]),
            (3, 2, 3, 4, WHITE),
            (4, 3, 2, 2, [227, 10, 23]),
            (7, 3, 1, 2, WHITE),
        ],
        "SG" => vec![(0, 0, 12, 4, [239, 51, 64]), (0, 4, 12, 4, WHITE), (1, 1, 2, 2, WHITE), (2, 1, 1, 2, [239, 51, 64])],
        "HK" => vec![(0, 0, 12, 8, [222, 41, 16]), (5, 2, 2, 4, WHITE), (4, 3, 4, 2, WHITE)],
        "TW" => vec![(0, 0, 12, 8, [254, 0, 0]), (0, 0, 6, 4, [0, 0, 149]), (2, 1, 2, 2, WHITE)],
        "IL" => vec![
            (0, 0, 12, 8, WHITE),
            (0, 1, 12, 1, [0, 56, 184]),
            (0, 6, 12, 1, [0, 56, 184]),
            (5, 3, 2, 2, [0, 56, 184]),
        ],
        "BR" => vec![
            (0, 0, 12, 8, [0, 156, 59]),
            (2, 3, 8, 2, [254, 223, 0]),
            (4, 2, 4, 4, [254, 223, 0]),
            (5, 1, 2, 6, [254, 223, 0]),
            (5, 3, 2, 2, [0, 39, 118]),
        ],
        "PT" => vec![(0, 0, 5, 8, [0, 102, 0]), (5, 0, 7, 8, [255, 0, 0]), (4, 3, 2, 2, [255, 204, 0])],
        "CZ" => vec![
            (0, 0, 12, 4, WHITE),
            (0, 4, 12, 4, [215, 20, 26]),
            (0, 0, 2, 8, [17, 69, 126]),
            (2, 1, 2, 6, [17, 69, 126]),
            (4, 3, 2, 2, [17, 69, 126]),
        ],
        "SK" => vec![
            (0, 0, 12, 3, WHITE),
            (0, 3, 12, 2, [11, 78, 162]),
            (0, 5, 12, 3, [238, 28, 37]),
            (2, 2, 3, 4, [238, 28, 37]),
        ],
        "GR" => {
            let mut cells: Vec<Cell> = (0..8)
                .map(|row| (0, row, 12, 1, if row % 2 == 0 { [13, 94, 175] } else { WHITE }))
                .collect();
            cells.extend([(0, 0, 5, 5, [13, 94, 175]), (2, 0, 1, 5, WHITE), (0, 2, 5, 1, WHITE)]);
            cells
        }
        "CL" => vec![(0, 0, 12, 4, WHITE), (0, 4, 12, 4, [213, 43, 30]), (0, 0, 4, 4, [0, 57, 166]), (1, 1, 2, 2, WHITE)],
        "PH" => vec![
            (0, 0, 12, 4, [0, 56, 168]),
            (0, 4, 12, 4, [206, 17, 38]),
            (0, 0, 2, 8, WHITE),
            (2, 1, 2, 6, WHITE),
            (4, 3, 2, 2, WHITE),
            (1, 3, 1, 2, [252, 209, 22]),
        ],
        "ZA" => vec![
            (0, 0, 12, 4, [222, 56, 49]),
            (0, 4, 12, 4, [0, 35, 149]),
            (0, 3, 12, 2, WHITE),
            (0, 0, 2, 8, WHITE),
            (2, 2, 2, 4, WHITE),
            (0, 1, 2, 6, [0, 122, 77]),
            (0, 3, 12, 2, [0, 122, 77]),
            (0, 2, 1, 4, BLACK),
            (1, 3, 1, 2, [255, 182, 18]),
        ],
        "AE" => vec![
            (0, 0, 12, 3, [0, 115, 47]),
            (0, 3, 12, 2, WHITE),
            (0, 5, 12, 3, BLACK),
            (0, 0, 3, 8, [255, 0, 0]),
        ],
        "SA" => vec![(0, 0, 12, 8, [0, 108, 53]), (3, 3, 6, 1, WHITE), (3, 5, 6, 1, WHITE)],
        "KZ" => vec![(0, 0, 12, 8, [0, 175, 202]), (5, 2, 2, 3, [254, 197, 0]), (4, 3, 4, 1, [254, 197, 0])],
        "MY" => {
            let mut cells: Vec<Cell> = (0..8)
                .map(|row| (0, row, 12, 1, if row % 2 == 0 { [204, 0, 1] } else { WHITE }))
                .collect();
            cells.extend([(0, 0, 6, 4, [1, 0, 102]), (2, 1, 2, 2, [255, 204, 0])]);
            cells
        }
        _ => return None,
    })
}

/// Country names for the tooltip.
pub fn country_name(code: &str) -> Option<&'static str> {
    COUNTRIES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

const COUNTRIES: &[(&str, &str)] = &[
    ("AD", "Andorra"), ("AE", "United Arab Emirates"), ("AF", "Afghanistan"), ("AG", "Antigua and Barbuda"),
    ("AL", "Albania"), ("AM", "Armenia"), ("AO", "Angola"), ("AR", "Argentina"), ("AT", "Austria"),
    ("AU", "Australia"), ("AZ", "Azerbaijan"), ("BA", "Bosnia and Herzegovina"), ("BB", "Barbados"),
    ("BD", "Bangladesh"), ("BE", "Belgium"), ("BF", "Burkina Faso"), ("BG", "Bulgaria"), ("BH", "Bahrain"),
    ("BI", "Burundi"), ("BJ", "Benin"), ("BN", "Brunei"), ("BO", "Bolivia"), ("BR", "Brazil"),
    ("BS", "Bahamas"), ("BT", "Bhutan"), ("BW", "Botswana"), ("BY", "Belarus"), ("BZ", "Belize"),
    ("CA", "Canada"), ("CD", "DR Congo"), ("CF", "Central African Republic"), ("CG", "Congo"),
    ("CH", "Switzerland"), ("CI", "Côte d'Ivoire"), ("CL", "Chile"), ("CM", "Cameroon"), ("CN", "China"),
    ("CO", "Colombia"), ("CR", "Costa Rica"), ("CU", "Cuba"), ("CV", "Cape Verde"), ("CY", "Cyprus"),
    ("CZ", "Czechia"), ("DE", "Germany"), ("DJ", "Djibouti"), ("DK", "Denmark"), ("DM", "Dominica"),
    ("DO", "Dominican Republic"), ("DZ", "Algeria"), ("EC", "Ecuador"), ("EE", "Estonia"), ("EG", "Egypt"),
    ("ER", "Eritrea"), ("ES", "Spain"), ("ET", "Ethiopia"), ("FI", "Finland"), ("FJ", "Fiji"),
    ("FR", "France"), ("GA", "Gabon"), ("GB", "United Kingdom"), ("GD", "Grenada"), ("GE", "Georgia"),
    ("GH", "Ghana"), ("GM", "Gambia"), ("GN", "Guinea"), ("GQ", "Equatorial Guinea"), ("GR", "Greece"),
    ("GT", "Guatemala"), ("GW", "Guinea-Bissau"), ("GY", "Guyana"), ("HK", "Hong Kong"), ("HN", "Honduras"),
    ("HR", "Croatia"), ("HT", "Haiti"), ("HU", "Hungary"), ("ID", "Indonesia"), ("IE", "Ireland"),
    ("IL", "Israel"), ("IN", "India"), ("IQ", "Iraq"), ("IR", "Iran"), ("IS", "Iceland"), ("IT", "Italy"),
    ("JM", "Jamaica"), ("JO", "Jordan"), ("JP", "Japan"), ("KE", "Kenya"), ("KG", "Kyrgyzstan"),
    ("KH", "Cambodia"), ("KM", "Comoros"), ("KN", "Saint Kitts and Nevis"), ("KP", "North Korea"),
    ("KR", "South Korea"), ("KW", "Kuwait"), ("KZ", "Kazakhstan"), ("LA", "Laos"), ("LB", "Lebanon"),
    ("LC", "Saint Lucia"), ("LI", "Liechtenstein"), ("LK", "Sri Lanka"), ("LR", "Liberia"), ("LS", "Lesotho"),
    ("LT", "Lithuania"), ("LU", "Luxembourg"), ("LV", "Latvia"), ("LY", "Libya"), ("MA", "Morocco"),
    ("MC", "Monaco"), ("MD", "Moldova"), ("ME", "Montenegro"), ("MG", "Madagascar"), ("MK", "North Macedonia"),
    ("ML", "Mali"), ("MM", "Myanmar"), ("MN", "Mongolia"), ("MO", "Macao"), ("MR", "Mauritania"),
    ("MT", "Malta"), ("MU", "Mauritius"), ("MV", "Maldives"), ("MW", "Malawi"), ("MX", "Mexico"),
    ("MY", "Malaysia"), ("MZ", "Mozambique"), ("NA", "Namibia"), ("NE", "Niger"), ("NG", "Nigeria"),
    ("NI", "Nicaragua"), ("NL", "Netherlands"), ("NO", "Norway"), ("NP", "Nepal"), ("NZ", "New Zealand"),
    ("OM", "Oman"), ("PA", "Panama"), ("PE", "Peru"), ("PG", "Papua New Guinea"), ("PH", "Philippines"),
    ("PK", "Pakistan"), ("PL", "Poland"), ("PR", "Puerto Rico"), ("PS", "Palestine"), ("PT", "Portugal"),
    ("PY", "Paraguay"), ("QA", "Qatar"), ("RO", "Romania"), ("RS", "Serbia"), ("RU", "Russia"),
    ("RW", "Rwanda"), ("SA", "Saudi Arabia"), ("SC", "Seychelles"), ("SD", "Sudan"), ("SE", "Sweden"),
    ("SG", "Singapore"), ("SI", "Slovenia"), ("SK", "Slovakia"), ("SL", "Sierra Leone"), ("SM", "San Marino"),
    ("SN", "Senegal"), ("SO", "Somalia"), ("SR", "Suriname"), ("SV", "El Salvador"), ("SY", "Syria"),
    ("TD", "Chad"), ("TG", "Togo"), ("TH", "Thailand"), ("TJ", "Tajikistan"), ("TM", "Turkmenistan"),
    ("TN", "Tunisia"), ("TR", "Türkiye"), ("TT", "Trinidad and Tobago"), ("TW", "Taiwan"), ("TZ", "Tanzania"),
    ("UA", "Ukraine"), ("UG", "Uganda"), ("US", "United States"), ("UY", "Uruguay"), ("UZ", "Uzbekistan"),
    ("VE", "Venezuela"), ("VN", "Vietnam"), ("YE", "Yemen"), ("ZA", "South Africa"), ("ZM", "Zambia"),
    ("ZW", "Zimbabwe"),
];

/// Where a row's flag goes: after the name (when the row has room for it),
/// else on the icon's bottom right. `cx, cy` is the row's content corner,
/// `name_w` the name's width and `room` how far right the flag may reach.
pub fn flag_at(place: u8, cx: f32, cy: f32, name_w: f32, room: f32) -> [f32; 2] {
    let after = [cx + 35.0 + name_w + GAP, cy];
    if place == 0 && after[0] + FLAG_W <= room {
        after
    } else {
        [cx + 32.0 - FLAG_W - 1.0, cy + 32.0 - FLAG_H - 1.0]
    }
}

/// The tooltip line for a server's place.
pub fn tooltip(place: &Place) -> Option<String> {
    match place {
        Place::Country(code) => Some(match country_name(code) {
            Some(name) => format!("{name} ({code})"),
            None => code.clone(),
        }),
        Place::Local => Some("Local network".into()),
        Place::NoCountry => Some("Location unknown: behind DDoS protection (anycast)".into()),
        Place::Blocked => Some("Location lookup blocked: allow api.country.is in your ad blocker".into()),
        Place::Unresolved => Some("Location unknown: the address doesn't resolve".into()),
        Place::Pending => None,
    }
}

/// Whether a place shows the "?" marker.
pub fn unknown(place: &Place) -> bool {
    matches!(place, Place::NoCountry | Place::Blocked | Place::Unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn addresses_split_into_host_and_port() {
        assert_eq!(host_port("mc.example.com"), Some(("mc.example.com".into(), 25565)));
        assert_eq!(host_port(" 1.2.3.4:25566 "), Some(("1.2.3.4".into(), 25566)));
        assert_eq!(host_port("[::1]:25570"), Some(("::1".into(), 25570)));
        assert_eq!(host_port("[2001:db8::1]"), Some(("2001:db8::1".into(), 25565)));
        assert_eq!(host_port("2001:db8::1"), Some(("2001:db8::1".into(), 25565)));
        assert_eq!(host_port("host:notaport"), Some(("host".into(), 25565)));
        assert_eq!(host_port(""), None);
    }

    #[test]
    fn local_addresses_are_never_looked_up() {
        for ip in ["127.0.0.1", "192.168.1.20", "10.0.0.5", "172.16.4.4", "100.100.1.1", "169.254.0.1"] {
            assert!(is_local(ip.parse::<Ipv4Addr>().unwrap().into()), "{ip}");
        }
        assert!(!is_local(Ipv4Addr::new(51, 75, 1, 1).into()));
        assert!(is_local(Ipv6Addr::LOCALHOST.into()));
        assert!(is_local("fd00::1".parse::<Ipv6Addr>().unwrap().into()));
        assert!(!is_local("2a01:4f8::1".parse::<Ipv6Addr>().unwrap().into()));
    }

    #[test]
    fn answers_parse_to_codes() {
        assert_eq!(parse_answer(r#"{"ip":"1.1.1.1","country":"us"}"#), Some("US".into()));
        assert_eq!(parse_answer(r#"{"error":"not found"}"#), None);
        assert_eq!(parse_answer(r#"{"country":"USA"}"#), None);
        assert_eq!(parse_answer("not json"), None);
    }

    #[test]
    fn flags_stay_inside_their_grid() {
        for (code, _) in COUNTRIES {
            if let Some(cells) = flag(code) {
                assert!(!cells.is_empty(), "{code}");
                for (x, y, w, h, _) in cells {
                    assert!(w > 0 && h > 0 && x + w <= 12 && y + h <= 8, "{code}: {x} {y} {w} {h}");
                }
            }
        }
        let drawn = COUNTRIES.iter().filter(|(c, _)| flag(c).is_some()).count();
        assert!(drawn >= 60, "{drawn}");
        assert!(flag("XX").is_none());
    }

    #[test]
    fn every_drawn_flag_has_a_name() {
        for code in ["DE", "US", "GB", "FR", "NL", "CA", "FI", "SG", "AU", "JP", "BR", "PL"] {
            assert!(flag(code).is_some() && country_name(code).is_some(), "{code}");
        }
        assert_eq!(tooltip(&Place::Country("DE".into())), Some("Germany (DE)".into()));
        assert_eq!(tooltip(&Place::Country("ZZ".into())), Some("ZZ".into()));
        assert_eq!(tooltip(&Place::Local), Some("Local network".into()));
        assert_eq!(tooltip(&Place::Pending), None);
        for p in [Place::NoCountry, Place::Blocked, Place::Unresolved] {
            assert!(tooltip(&p).is_some() && unknown(&p), "{p:?}");
        }
        assert!(!unknown(&Place::Local) && !unknown(&Place::Pending));
    }

    #[test]
    fn answers_become_places() {
        let ok = |b: &str| Answer::Status(200, b.into());
        assert_eq!(place_of(ok(r#"{"ip":"8.8.8.8","country":"US"}"#)), Place::Country("US".into()));
        // Anycast: country.is has no country and says 404.
        assert_eq!(place_of(Answer::Status(404, String::new())), Place::NoCountry);
        assert_eq!(place_of(ok("{}")), Place::NoCountry);
        // Blocked (AdGuard answers 0.0.0.0: the connection is refused), down or rate limited: retried.
        assert_eq!(place_of(Answer::Unreachable), Place::Blocked);
        assert_eq!(place_of(Answer::Status(429, String::new())), Place::Blocked);
        assert_eq!(place_of(Answer::Status(503, String::new())), Place::Blocked);
        assert!(Place::Blocked.retried() && Place::Unresolved.retried());
        assert!(!Place::NoCountry.retried() && !Place::Country("DE".into()).retried());
    }

    #[test]
    fn failures_get_a_log_line() {
        let blocked = log_line("play.example.net", &Place::Blocked).unwrap();
        assert!(blocked.starts_with("Server Flags: play.example.net can't reach api.country.is"));
        assert!(blocked.contains("AdGuard"));
        assert!(log_line("mc.hypixel.net", &Place::NoCountry).unwrap().contains("anycast"));
        assert!(log_line("x", &Place::Unresolved).unwrap().contains("doesn't resolve"));
        assert_eq!(log_line("x", &Place::Country("US".into())), None);
        assert_eq!(log_line("x", &Place::Local), None);
    }

    #[test]
    fn flag_goes_after_the_name_or_on_the_icon() {
        assert_eq!(flag_at(0, 10.0, 50.0, 40.0, 300.0), [10.0 + 35.0 + 40.0 + GAP, 50.0]);
        // No room after a long name: on the icon.
        assert_eq!(flag_at(0, 10.0, 50.0, 250.0, 300.0), [10.0 + 19.0, 50.0 + 23.0]);
        assert_eq!(flag_at(1, 10.0, 50.0, 10.0, 300.0), [29.0, 73.0]);
    }

    #[test]
    fn settings_cycle_and_reset() {
        let mut s = Settings::read(|k| (k == "lookup").then(|| "0".to_owned()));
        assert!(!s.on(o::LOOKUP));
        assert!(s.on(o::UNKNOWN), "the ? shows by default");
        assert_eq!(s.click(o::PLACE), [("place", "1".to_owned())]);
        assert_eq!(s.labels()[o::PLACE], "Flag position: On the icon");
        assert_eq!(s.click(OPTS.len()).len(), OPTS.len());
        assert_eq!(s, Settings::default());
    }
}
