use std::collections::HashMap;
use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph, Row, Table};
use ratatui::{Frame, Terminal};

const TICK_SECS: u64 = 1;
const SPINNER: [&str; 4] = ["▘", "▝", "▗", "▖"];
// ---- tema em tempo de execução — o terminal ':' (tecla dois-pontos) mexe nisto ----

#[derive(Clone, Copy, PartialEq, Debug)]
enum BorderKind {
    Pixel,
    Linha,
    Dupla,
    Grossa,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum GaugeKind {
    Quadrado,
    Circulo,
}

#[derive(Clone, Copy)]
struct Theme {
    bg: Color,
    cy: Color,
    mag: Color,
    grn: Color,
    yel: Color,
    red: Color,
    gray: Color,
    blu: Color,
    lblu: Color,
    lred: Color,
    wht: Color,
    blk: Color,
    border: BorderKind,
    gauge: GaugeKind,
}

impl Theme {
    fn pixel() -> Self {
        Self {
            bg: Color::Rgb(16, 16, 26),
            cy: Color::Rgb(41, 173, 255),
            mag: Color::Rgb(255, 119, 168),
            grn: Color::Rgb(0, 228, 54),
            yel: Color::Rgb(255, 236, 39),
            red: Color::Rgb(255, 0, 77),
            gray: Color::Rgb(131, 118, 156),
            blu: Color::Rgb(29, 43, 83),
            lblu: Color::Rgb(255, 204, 170),
            lred: Color::Rgb(255, 163, 0),
            wht: Color::Rgb(255, 241, 232),
            blk: Color::Rgb(10, 10, 15),
            border: BorderKind::Linha,
            gauge: GaugeKind::Quadrado,
        }
    }
    fn dracula() -> Self {
        Self {
            bg: Color::Rgb(10, 10, 15),
            cy: Color::Rgb(0, 222, 255),
            mag: Color::Rgb(255, 121, 198),
            grn: Color::Rgb(80, 250, 123),
            yel: Color::Rgb(241, 250, 140),
            red: Color::Rgb(255, 85, 85),
            gray: Color::Rgb(98, 114, 164),
            blu: Color::Rgb(40, 42, 54),
            lblu: Color::Rgb(189, 223, 255),
            lred: Color::Rgb(255, 148, 148),
            wht: Color::Rgb(248, 248, 242),
            blk: Color::Rgb(10, 10, 15),
            border: BorderKind::Linha,
            gauge: GaugeKind::Circulo,
        }
    }
}

static THEME: OnceLock<RwLock<Theme>> = OnceLock::new();

fn t() -> Theme {
    *THEME.get_or_init(|| RwLock::new(Theme::pixel())).read().unwrap()
}

fn c_bg() -> Color { t().bg }
fn c_cy() -> Color { t().cy }
fn c_mag() -> Color { t().mag }
fn c_grn() -> Color { t().grn }
fn c_yel() -> Color { t().yel }
fn c_red() -> Color { t().red }
fn c_gray() -> Color { t().gray }
fn c_blu() -> Color { t().blu }
fn c_lblu() -> Color { t().lblu }
fn c_lred() -> Color { t().lred }
fn c_wht() -> Color { t().wht }
fn c_blk() -> Color { t().blk }

fn pal() -> [Color; 6] {
    [c_cy(), c_mag(), c_grn(), c_yel(), c_lblu(), c_lred()]
}

fn mudar_tema(f: impl FnOnce(&mut Theme)) {
    let lock = THEME.get_or_init(|| RwLock::new(Theme::pixel()));
    let mut novo = *lock.read().unwrap();
    f(&mut novo);
    *lock.write().unwrap() = novo;
}

fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    let full: String = if s.len() == 3 {
        s.chars().flat_map(|c| [c, c]).collect()
    } else {
        s.to_string()
    };
    if full.len() != 6 {
        return None;
    }
    let n = |i: usize| u8::from_str_radix(full.get(i..i + 2)?, 16).ok();
    Some(Color::Rgb(n(0)?, n(2)?, n(4)?))
}

fn hex_of(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        _ => "?".into(),
    }
}

fn config_arquivo() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(std::path::Path::new(&home).join(".config/ratscope/config.json"))
}

fn salvar_config() -> Result<String, String> {
    let p = config_arquivo().ok_or("sem $HOME")?;
    let x = t();
    let j = serde_json::json!({
        "bg": hex_of(x.bg), "cy": hex_of(x.cy), "mag": hex_of(x.mag),
        "grn": hex_of(x.grn), "yel": hex_of(x.yel), "red": hex_of(x.red),
        "gray": hex_of(x.gray), "blu": hex_of(x.blu), "lblu": hex_of(x.lblu),
        "lred": hex_of(x.lred), "wht": hex_of(x.wht), "blk": hex_of(x.blk),
        "borda": match x.border {
            BorderKind::Pixel => "pixel",
            BorderKind::Linha => "linha",
            BorderKind::Dupla => "dupla",
            BorderKind::Grossa => "grossa",
        },
        "medidor": match x.gauge {
            GaugeKind::Quadrado => "quadrado",
            GaugeKind::Circulo => "circulo",
        },
    });
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(&j).unwrap()).map_err(|e| e.to_string())?;
    Ok(p.display().to_string())
}

// tema salvo sobrepõe o padrão no boot; erros são ignorados de propósito
fn carregar_config() {
    let Some(p) = config_arquivo() else { return };
    let Ok(txt) = std::fs::read_to_string(p) else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { return };
    let cor = |k: &str| v[k].as_str().and_then(parse_hex);
    mudar_tema(|t| {
        if let Some(c) = cor("bg") { t.bg = c; }
        if let Some(c) = cor("cy") { t.cy = c; }
        if let Some(c) = cor("mag") { t.mag = c; }
        if let Some(c) = cor("grn") { t.grn = c; }
        if let Some(c) = cor("yel") { t.yel = c; }
        if let Some(c) = cor("red") { t.red = c; }
        if let Some(c) = cor("gray") { t.gray = c; }
        if let Some(c) = cor("blu") { t.blu = c; }
        if let Some(c) = cor("lblu") { t.lblu = c; }
        if let Some(c) = cor("lred") { t.lred = c; }
        if let Some(c) = cor("wht") { t.wht = c; }
        if let Some(c) = cor("blk") { t.blk = c; }
        t.border = match v["borda"].as_str() {
            Some("linha") => BorderKind::Linha,
            Some("dupla") => BorderKind::Dupla,
            Some("grossa") => BorderKind::Grossa,
            Some("pixel") => BorderKind::Pixel,
            _ => t.border,
        };
        t.gauge = match v["medidor"].as_str() {
            Some("circulo") => GaugeKind::Circulo,
            Some("quadrado") => GaugeKind::Quadrado,
            _ => t.gauge,
        };
    });
}

// comandos do terminal embutido; devolve linhas de resposta
fn exec_cmd(cmd: &str) -> Vec<String> {
    let bad = |m: &str| vec![format!("✖ {m}")];
    let ok = || vec!["✔ feito".to_string()];
    let ws: Vec<&str> = cmd.split_whitespace().collect();
    match ws.as_slice() {
        ["ajuda"] => vec![
            "comandos:".into(),
            "  fundo <hex>                ex: fundo #10101A".into(),
            "  cor <nome> <hex>           ciano rosa verde amarelo vermelho".into(),
            "                             cinza azul pessego laranja branco preto".into(),
            "  borda <pixel|linha|dupla|grossa>".into(),
            "  medidor <quadrado|circulo>".into(),
            "  tema <pixel|dracula>       predefinições".into(),
            "  ver                        valores atuais".into(),
            "  salvar                     grava em ~/.config/ratscope/config.json".into(),
            "  reset                      volta ao tema pixel".into(),
        ],
        ["fundo", h] => match parse_hex(h) {
            Some(c) => {
                mudar_tema(move |t| t.bg = c);
                ok()
            }
            None => bad("hex inválido (ex: #10101A)"),
        },
        ["cor", nome, h] => match parse_hex(h) {
            None => bad("hex inválido (ex: #FF004D)"),
            Some(c) => {
                let set: fn(&mut Theme, Color) = match *nome {
                    "ciano" => |t, c| t.cy = c,
                    "rosa" => |t, c| t.mag = c,
                    "verde" => |t, c| t.grn = c,
                    "amarelo" => |t, c| t.yel = c,
                    "vermelho" => |t, c| t.red = c,
                    "cinza" => |t, c| t.gray = c,
                    "azul" => |t, c| t.blu = c,
                    "pessego" => |t, c| t.lblu = c,
                    "laranja" => |t, c| t.lred = c,
                    "branco" => |t, c| t.wht = c,
                    "preto" => |t, c| t.blk = c,
                    _ => {
                        return bad("cor desconhecida — 'ajuda' lista os nomes");
                    }
                };
                mudar_tema(move |t| set(t, c));
                ok()
            }
        },
        ["borda", k] => {
            let k = match *k {
                "pixel" => Some(BorderKind::Pixel),
                "linha" => Some(BorderKind::Linha),
                "dupla" => Some(BorderKind::Dupla),
                "grossa" => Some(BorderKind::Grossa),
                _ => None,
            };
            match k {
                Some(b) => {
                    mudar_tema(move |t| t.border = b);
                    ok()
                }
                None => bad("use: borda pixel|linha|dupla|grossa"),
            }
        }
        ["medidor", k] => {
            let k = match *k {
                "quadrado" => Some(GaugeKind::Quadrado),
                "circulo" => Some(GaugeKind::Circulo),
                _ => None,
            };
            match k {
                Some(g) => {
                    mudar_tema(move |t| t.gauge = g);
                    ok()
                }
                None => bad("use: medidor quadrado|circulo"),
            }
        }
        ["tema", "pixel"] => {
            mudar_tema(|t| *t = Theme::pixel());
            ok()
        }
        ["tema", "dracula"] => {
            mudar_tema(|t| *t = Theme::dracula());
            ok()
        }
        ["tema", _] => bad("use: tema pixel|dracula"),
        ["reset"] => {
            mudar_tema(|t| *t = Theme::pixel());
            ok()
        }
        ["ver"] => {
            let x = t();
            vec![
                format!(
                    "fundo {} · borda {:?} · medidor {:?}",
                    hex_of(x.bg),
                    x.border,
                    x.gauge
                ),
                format!(
                    "ciano {} rosa {} verde {} amarelo {} vermelho {}",
                    hex_of(x.cy),
                    hex_of(x.mag),
                    hex_of(x.grn),
                    hex_of(x.yel),
                    hex_of(x.red)
                ),
                format!(
                    "cinza {} azul {} pessego {} laranja {} branco {}",
                    hex_of(x.gray),
                    hex_of(x.blu),
                    hex_of(x.lblu),
                    hex_of(x.lred),
                    hex_of(x.wht)
                ),
            ]
        }
        ["salvar"] => match salvar_config() {
            Ok(p) => vec![format!("✔ salvo em {p}")],
            Err(e) => bad(&e),
        },
        [] => vec![],
        _ => bad("comando desconhecido — 'ajuda' lista"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Caller {
    Docker,
    Unit,
    Proc,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    Info,
    Logs,
    Status,
    Restart,
    Reload,
    Stop,
    Start,
    Kill,
    Kill9,
    Pause,
    Unpause,
    Prune,
    Enable,
    Disable,
}

impl Act {
    fn label(self) -> &'static str {
        match self {
            Act::Info => "▪ detalhes",
            Act::Logs => "▤ ver logs",
            Act::Status => "▤ status/inspect",
            Act::Restart => "↻ restart",
            Act::Reload => "↻ reload",
            Act::Stop => "■ stop",
            Act::Start => "▶ start",
            Act::Kill => "✚ kill",
            Act::Kill9 => "✚ kill -9",
            Act::Pause => "⏸ pause",
            Act::Unpause => "⏵ unpause",
            Act::Prune => "🧹 prune imagens",
            Act::Enable => "⏻ enable (boot)",
            Act::Disable => "∅ disable (boot)",
        }
    }
    fn cmd(self, tgt: &Target) -> (&'static str, Vec<String>) {
        let n = tgt.name.clone();
        let (p, a): (&'static str, Vec<&str>) = match (tgt.caller, self) {
            (Caller::Docker, Act::Logs) => ("docker", vec!["logs", "--tail", "80", &n]),
            (Caller::Docker, Act::Restart) => ("docker", vec!["restart", &n]),
            (Caller::Docker, Act::Stop) => ("docker", vec!["stop", &n]),
            (Caller::Docker, Act::Start) => ("docker", vec!["start", &n]),
            (Caller::Docker, Act::Kill) => ("docker", vec!["kill", &n]),
            (Caller::Docker, Act::Pause) => ("docker", vec!["pause", &n]),
            (Caller::Docker, Act::Unpause) => ("docker", vec!["unpause", &n]),
            (Caller::Docker, Act::Prune) => ("docker", vec!["image", "prune", "-f"]),
            (Caller::Unit, Act::Logs) => ("journalctl", vec!["-u", &n, "-n", "80", "--no-pager"]),
            (Caller::Unit, Act::Restart) => ("systemctl", vec!["restart", &n]),
            (Caller::Unit, Act::Stop) => ("systemctl", vec!["stop", &n]),
            (Caller::Unit, Act::Start) => ("systemctl", vec!["start", &n]),
            (Caller::Unit, Act::Reload) => ("systemctl", vec!["reload", &n]),
            (Caller::Unit, Act::Enable) => ("systemctl", vec!["enable", &n]),
            (Caller::Unit, Act::Disable) => ("systemctl", vec!["disable", &n]),
            (Caller::Proc, Act::Kill) => ("kill", vec!["-TERM", &n]),
            (Caller::Proc, Act::Kill9) => ("kill", vec!["-KILL", &n]),
            _ => return ("true", vec![]), // Info/Status viram tela, não comando
        };
        (p, a.into_iter().map(String::from).collect())
    }
    fn verb(self) -> &'static str {
        match self {
            Act::Info => "detalhes",
            Act::Logs => "logs",
            Act::Status => "status",
            Act::Restart => "restart",
            Act::Reload => "reload",
            Act::Stop => "stop",
            Act::Start => "start",
            Act::Kill => "kill",
            Act::Kill9 => "kill -9",
            Act::Pause => "pause",
            Act::Unpause => "unpause",
            Act::Prune => "prune imagens",
            Act::Enable => "enable",
            Act::Disable => "disable",
        }
    }
}

fn menu_acts(c: Caller) -> Vec<Act> {
    match c {
        Caller::Docker => vec![
            Act::Info, Act::Logs, Act::Restart, Act::Pause, Act::Unpause,
            Act::Stop, Act::Start, Act::Kill, Act::Prune,
        ],
        Caller::Unit => vec![
            Act::Info, Act::Logs, Act::Status, Act::Restart, Act::Reload,
            Act::Stop, Act::Start, Act::Enable, Act::Disable,
        ],
        Caller::Proc => vec![Act::Info, Act::Kill, Act::Kill9],
    }
}

// ações dentro da tela de detalhes (Info já está aberta)
fn detail_acts(c: Caller) -> Vec<Act> {
    menu_acts(c).into_iter().filter(|a| *a != Act::Info).collect()
}

#[derive(Clone)]
struct Target {
    caller: Caller,
    name: String,
    user: bool, // unit de usuário (systemd --user)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Docker,
    Units,
    Ports,
    Overview,
}

struct Container {
    name: String,
    image: String,
    status: String,
    cpu: String,
    mem: String,
    size_b: u64,
}

struct UnitRow {
    name: String,
    desc: String,
    state: String,
    user: bool,
}

struct ProcRow {
    pid: u32,
    comm: String,
    rss_b: u64,
}

struct Pc {
    cpu_pct: f32,
    mem_total: u64,
    mem_used: u64,
    swap_total: u64,
    swap_used: u64,
    mounts: Vec<(String, u64, u64)>, // target, size, used
}

struct Snapshot {
    containers: Vec<Container>,
    units: Vec<UnitRow>,
    ports: Vec<(String, String)>,
    procs: Vec<ProcRow>,
    pc: Pc,
    docker_ok: bool,
    dk_total: u64,
}

struct DiskData {
    total: u64,
    cats: Vec<(String, u64)>,
    images: Vec<(String, u64)>,
}

enum Mode {
    Browse,
    Cmd { input: String, out: Vec<String> },
    Detail { tgt: Target, hover: usize },
    Menu { tgt: Target, acts: Vec<Act>, hover: usize },
    Confirm { tgt: Target, act: Act },
    Logs { tgt: Target, text: Vec<String>, rx: Receiver<String> },
    Text { title: String, text: Vec<String>, scroll: usize },
    Loading { label: String, at: Instant },
    Disk { data: DiskData },
    Ctx { items: Vec<(String, bool)>, hover: usize },
}

enum Msg {
    Snap(Snapshot),
    Done(String),
    Disk(DiskData),
    Ctx(Vec<(String, bool)>),
    Text { title: String, text: Vec<String> },
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Row(Focus, usize),
    Item(usize),
}

struct State {
    snap: Option<Snapshot>,
    focus: Focus,
    sel_docker: usize,
    sel_units: usize,
    sel_ports: usize,
    sel_over: usize,
    scroll_docker: usize,
    scroll_units: usize,
    scroll_ports: usize,
    hist: HashMap<String, Vec<f32>>,
    hist_global: Vec<f32>,
    mode: Mode,
    last_click: Option<(Instant, Hit)>,
    area: Rect,
    msg: Option<(String, Instant)>,
}

fn run(prog: &str, args: &[&str]) -> Vec<String> {
    run_out({
        let mut c = Command::new(prog);
        c.args(args);
        c
    })
}

fn run_out(mut cmd: Command) -> Vec<String> {
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect())
        .unwrap_or_default()
}

static CTX: OnceLock<RwLock<Option<String>>> = OnceLock::new();

fn set_ctx(c: Option<String>) {
    *CTX.get_or_init(|| RwLock::new(None)).write().unwrap() = c;
}

fn get_ctx() -> Option<String> {
    CTX.get_or_init(|| RwLock::new(None)).read().unwrap().clone()
}

fn docker_cmd(args: &[&str]) -> Command {
    let mut c = Command::new("docker");
    if let Some(x) = get_ctx() {
        c.args(["--context", &x]);
    }
    c.args(args);
    c
}

static ALLS: OnceLock<RwLock<bool>> = OnceLock::new();

fn set_all(v: bool) {
    *ALLS.get_or_init(|| RwLock::new(false)).write().unwrap() = v;
}

fn get_all() -> bool {
    *ALLS.get_or_init(|| RwLock::new(false)).read().unwrap()
}

// layout maior no de baixo quando a visão é "pc" (sync em todo switch de foco)
static OVER_SPLIT: OnceLock<RwLock<bool>> = OnceLock::new();

fn set_over_split(v: bool) {
    *OVER_SPLIT.get_or_init(|| RwLock::new(false)).write().unwrap() = v;
}

fn get_over_split() -> bool {
    *OVER_SPLIT.get_or_init(|| RwLock::new(false)).read().unwrap()
}

fn split(area: Rect) -> [Rect; 3] {
    let (a, b) = if get_over_split() { (36u16, 63u16) } else { (55u16, 41u16) };
    Layout::vertical([
        Constraint::Percentage(a),
        Constraint::Percentage(b),
        Constraint::Length(1),
    ])
    .areas(area)
}

// ---- parsers ----

fn json_lines(lines: &[String]) -> Vec<serde_json::Value> {
    lines.iter().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

// shortcut: os permissões de root ausentes → enable/disable de units de sistema podem falhar; erro vai pro rodapé
fn systemctl_units(user: bool, all: bool) -> Vec<UnitRow> {
    let mut c = Command::new("systemctl");
    if user {
        c.arg("--user");
    }
    c.args(&[
        "list-units",
        "--type=service",
        if all { "--all" } else { "--state=running" },
        "--no-legend",
        "--no-pager",
    ]);
    run_out(c)
        .into_iter()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let name = w.next()?.to_string();
            w.next(); // load
            let state = w.next()?.to_string(); // active
            w.next(); // sub
            let desc = w.collect::<Vec<_>>().join(" ");
            (!name.is_empty() && !desc.is_empty()).then_some(UnitRow { name, desc, state, user })
        })
        .collect()
}

fn units() -> Vec<UnitRow> {
    let all = get_all();
    let mut v = systemctl_units(false, all);
    v.extend(systemctl_units(true, all));
    v
}

fn proc_of(line: &str) -> String {
    let Some(u) = line.find("users:((") else {
        return "—".to_string();
    };
    let rest = &line[u + "users:((".len()..];
    let Some(q) = rest.find('"') else {
        return "—".to_string();
    };
    let s = &rest[q + 1..];
    let end = s.find('"').unwrap_or(s.len());
    s[..end].to_string()
}

fn ports() -> Vec<(String, String)> {
    run("ss", &["-tlnp"])
        .into_iter()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            if w.next()? != "LISTEN" {
                return None;
            }
            w.next();
            w.next();
            let local = w.next()?.to_string();
            Some((local, proc_of(&l)))
        })
        .collect()
}

// cpu% global via delta de /proc/stat entre snapshots
fn cpu_pct() -> f32 {
    static SET: OnceLock<Mutex<Option<(u64, u64)>>> = OnceLock::new();
    let line = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let Some(l) = line.lines().next() else { return 0.0 };
    let nums: Vec<u64> = l
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if nums.is_empty() {
        return 0.0;
    }
    let total: u64 = nums.iter().sum();
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    let m = SET.get_or_init(|| Mutex::new(None));
    let pct = {
        let g = m.lock().unwrap();
        match *g {
            Some((t0, i0)) if total > t0 => 100.0 * (1.0 - (idle - i0) as f32 / (total - t0) as f32),
            _ => 0.0, // warmup: primeiro tick sem delta
        }
    };
    *m.lock().unwrap() = Some((total, idle));
    pct.clamp(0.0, 100.0)
}

// (total, available, swap_total, swap_free)
fn meminfo() -> (u64, u64, u64, u64) {
    let txt = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let grab = |key: &str| -> u64 {
        txt.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
    };
    (
        grab("MemTotal:"),
        grab("MemAvailable:"),
        grab("SwapTotal:"),
        grab("SwapFree:"),
    )
}

// shortcut: alvos de mount com espaço quebram o parse; caso comum (/, /home, /boot) não tem
fn mounts() -> Vec<(String, u64, u64)> {
    run("df", &[
        "-B1",
        "-x",
        "tmpfs",
        "-x",
        "devtmpfs",
        "-x",
        "efivarfs",
        "-x",
        "squashfs",
        "--output=target,used,size",
    ])
    .into_iter()
    .skip(1)
    .filter_map(|l| {
        let mut w = l.split_whitespace();
        let target = w.next()?.to_string();
        let used: u64 = w.next()?.parse().ok()?;
        let size: u64 = w.next()?.parse().ok()?;
        (size > 0).then_some((target, size, used))
    })
    .collect()
}

fn procs_listado(n: usize) -> Vec<ProcRow> {
    run("ps", &["-eo", "pid,comm,rss,pcpu", "--sort=-rss", "--no-headers"])
        .into_iter()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pid: u32 = w.next()?.parse().ok()?;
            let comm = w.next()?.to_string();
            let rss: u64 = w.next()?.parse().ok()?; // kB
            let _cpu: f32 = w.next().unwrap_or("0").parse().unwrap_or(0.0);
            (rss > 0).then_some(ProcRow {
                pid,
                comm,
                rss_b: rss * 1024,
            })
        })
        .skip(1) // seed: pula o próprio ratscope
        .take(n)
        .collect()
}

fn snapshot() -> Snapshot {
    let stats = json_lines(&run_out(docker_cmd(&["stats", "--no-stream", "--format", "{{json .}}"])));
    let ps = json_lines(&run_out(docker_cmd(&["ps", "-s", "--format", "{{json .}}"])));
    let df = json_lines(&run_out(docker_cmd(&["system", "df", "--format", "{{json .}}"])));
    let dk_total = df_all(&df).iter().map(|(_, v)| *v).sum();
    let containers = ps
        .iter()
        .filter_map(|c| {
            let name = c["Names"].as_str()?.to_string();
            let st = stats.iter().find(|s| s["Name"].as_str() == Some(name.as_str()));
            Some(Container {
                name,
                image: c["Image"].as_str().unwrap_or("").to_string(),
                status: c["Status"].as_str().unwrap_or("").to_string(),
                cpu: st.and_then(|s| s["CPUPerc"].as_str()).unwrap_or("-").to_string(),
                mem: st.and_then(|s| s["MemPerc"].as_str()).unwrap_or("-").to_string(),
                size_b: size_bytes(c["Size"].as_str().unwrap_or("")),
            })
        })
        .collect();
    // shortcut: docker_ok = "algum retorno"; docker vivo sem containers mostra como morto
    let docker_ok = !stats.is_empty() || !ps.is_empty();
    let (mt, ma, swt, swf) = meminfo();
    let pc = Pc {
        cpu_pct: cpu_pct(),
        mem_total: mt,
        mem_used: mt.saturating_sub(ma),
        swap_total: swt,
        swap_used: swt.saturating_sub(swf),
        mounts: mounts(),
    };
    let procs = procs_listado(12);
    Snapshot { containers, units: units(), ports: ports(), procs, pc, docker_ok, dk_total }
}

// docker system df --format json vem um obj por linha OU array; aceita ambos
fn df_all(vals: &[serde_json::Value]) -> Vec<(String, u64)> {
    use serde_json::Value;
    let mut out = Vec::new();
    for v in vals {
        let list: Vec<Value> = match v {
            Value::Array(a) => a.clone(),
            _ => vec![v.clone()],
        };
        for o in list {
            let t = match o["Type"].as_str() {
                Some(t) => t.to_string(),
                None => continue,
            };
            let s = o["Size"]
                .as_u64()
                .unwrap_or_else(|| o["Size"].as_str().map(size_bytes).unwrap_or(0));
            out.push((t, s));
        }
    }
    out
}

fn cat_label(t: &str) -> String {
    match t {
        "Images" => "imagens",
        "Containers" => "containers",
        "Local Volumes" | "Volumes" => "volumes",
        "Build Cache" => "build cache",
        other => return other.to_lowercase(),
    }
    .to_string()
}

fn trunc(s: &str, n: usize) -> String {
    s.chars()
        .take(n - 1)
        .collect::<String>()
        + (if s.chars().count() > n - 1 { "…" } else { "" })
}

// medidor de preenchimento conforme o tema
fn pie(frac: f64) -> &'static str {
    let s: [&str; 5] = match t().gauge {
        GaugeKind::Quadrado => ["□", "▤", "▥", "▦", "■"],
        GaugeKind::Circulo => ["○", "◔", "◑", "◕", "●"],
    };
    match frac {
        f if f <= 0.0 => s[0],
        f if f < 0.25 => s[1],
        f if f < 0.5 => s[2],
        f if f < 0.75 => s[3],
        _ => s[4],
    }
}

fn size_bytes(s: &str) -> u64 {
    let clean = s.trim();
    let nums: String = clean
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let suf = clean[nums.len()..].trim().to_lowercase();
    let v: f64 = nums.parse().unwrap_or(0.0);
    let m = if suf.starts_with('t') {
        1e12
    } else if suf.starts_with('g') {
        1e9
    } else if suf.starts_with('m') {
        1e6
    } else if suf.starts_with('k') {
        1e3
    } else {
        1.0
    };
    (v * m) as u64
}

fn fmt_size(b: u64) -> String {
    let (v, u) = if b >= 1_000_000_000_000 {
        (b as f64 / 1e12, "TB")
    } else if b >= 1_000_000_000 {
        (b as f64 / 1e9, "GB")
    } else if b >= 1_000_000 {
        (b as f64 / 1e6, "MB")
    } else if b >= 1000 {
        (b as f64 / 1e3, "kB")
    } else {
        (b as f64, "B")
    };
    if u == "B" {
        format!("{b}{u}")
    } else {
        format!("{v:.1}{u}")
    }
}

fn parse_pct(s: &str) -> f32 {
    s.trim().trim_end_matches('%').trim().parse().unwrap_or(0.0)
}

fn spark(vals: &[f32]) -> String {
    const S: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    vals.iter().rev().take(8).rev().map(|v| S[((*v / 100.0) * 7.0).round().clamp(0.0, 7.0) as usize]).collect()
}

fn spark_full(vals: &[f32]) -> String {
    const S: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    vals.iter().map(|v| S[((*v / 100.0) * 7.0).round().clamp(0.0, 7.0) as usize]).collect()
}

// barra empilhada: (bytes, cor) + resto cinza; cada categoria tem presença mínima de 1 coluna
fn bar_stack(w: usize, total: u64, parts: &[(u64, Color)]) -> Vec<Span<'static>> {
    if total == 0 || w == 0 {
        return vec![Span::styled(" — sem dados".to_string(), Style::new().fg(c_gray()))];
    }
    let mut spans = Vec::new();
    let mut used_w = 0usize;
    for (bytes, col) in parts {
        if used_w >= w {
            break;
        }
        let mut cw = ((*bytes as f64 / total as f64) * w as f64).round() as usize;
        cw = cw.min(w - used_w);
        if cw == 0 {
            cw = 1; // aparece mesmo sem uso (ex: swap zerado)
        }
        spans.push(Span::styled("█".repeat(cw), Style::new().fg(*col)));
        used_w += cw;
    }
    if used_w < w {
        spans.push(Span::styled("▒".repeat(w - used_w), Style::new().fg(c_gray())));
    }
    spans
}

// ---- geometria / hit ----

fn popup_rect(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

// linhas fixas de info por tipo de alvo (docker 6, unit 4, proc 3)
fn info_len(c: Caller) -> usize {
    match c {
        Caller::Docker => 6,
        Caller::Unit => 4,
        Caller::Proc => 3,
    }
}

fn detail_rect(area: Rect, tgt: &Target) -> Rect {
    let h = info_len(tgt.caller) + detail_acts(tgt.caller).len() + 5;
    popup_rect(area, 64, h as u16)
}

fn menu_rect(area: Rect, n: usize) -> Rect {
    popup_rect(area, 30, n as u16 + 3)
}

// linhas de dados: rel 0 = borda, 1 = título, 2 = cabeçalho
fn table_hit(area: Rect, cy: u16, off: usize, len: usize, vis: usize) -> Option<usize> {
    let rel = cy as i32 - area.y as i32;
    if rel < 3 || rel >= area.height as i32 - 1 {
        return None;
    }
    let idx = (rel - 3) as usize + off;
    (idx < len && idx < off + vis).then_some(idx)
}

// qtas linhas de processos cabem na visão pc (2 bordas + título + 14 linhas fixas de conteúdo)
fn over_nprocs(area: Rect) -> usize {
    (area.height.saturating_sub(17) as usize).min(8)
}

fn over_hit(area: Rect, cy: u16, snap: Option<&Snapshot>) -> Option<usize> {
    let n = over_nprocs(area);
    if n == 0 {
        return None;
    }
    // 2 bordas + título + cpu + total + bar + legend + blank + hdr ram = 8
    let rel = cy as i32 - area.y as i32 - 8;
    if rel < 0 {
        return None;
    }
    let idx = rel as usize;
    let len = snap.map_or(0, |s| s.procs.len().min(n));
    (idx < len).then_some(idx)
}

// ---- helpers de estado ----

fn cur_len(st: &State, f: Focus) -> usize {
    match (st.snap.as_ref(), f) {
        (Some(s), Focus::Docker) => s.containers.len(),
        (Some(s), Focus::Units) => s.units.len(),
        (Some(s), Focus::Ports) => s.ports.len(),
        (Some(s), Focus::Overview) => s.procs.len(),
        _ => 0,
    }
}

fn target_of(st: &State, f: Focus, idx: usize) -> Option<Target> {
    let snap = st.snap.as_ref()?;
    match f {
        Focus::Docker => snap.containers.get(idx).map(|c| Target {
            caller: Caller::Docker,
            name: c.name.clone(),
            user: false,
        }),
        Focus::Units => snap.units.get(idx).map(|u| Target {
            caller: Caller::Unit,
            name: u.name.clone(),
            user: u.user,
        }),
        Focus::Overview => snap.procs.get(idx).map(|p| Target {
            caller: Caller::Proc,
            name: p.pid.to_string(), // o nome do alvo é o pid
            user: false,
        }),
        Focus::Ports => None,
    }
}

fn st_sel(st: &State) -> usize {
    match st.focus {
        Focus::Docker => st.sel_docker,
        Focus::Units => st.sel_units,
        Focus::Ports => st.sel_ports,
        Focus::Overview => st.sel_over,
    }
}

fn open_menu(st: &mut State) {
    if let Some(tgt) = target_of(st, st.focus, st_sel(st)) {
        st.mode = Mode::Menu {
            tgt: tgt.clone(),
            acts: menu_acts(tgt.caller),
            hover: 0,
        };
        st.last_click = None;
    }
}

fn open_detail(st: &mut State) {
    if let Some(tgt) = target_of(st, st.focus, st_sel(st)) {
        st.mode = Mode::Detail { tgt, hover: 0 };
        st.last_click = None;
    }
}

// um só lugar que decide o que cada ação faz (menu, detalhes e mouse usam isto)
fn dispatch_act(st: &mut State, tgt: Target, act: Act, tx: &Sender<Msg>) {
    if act == Act::Info {
        st.mode = Mode::Detail { tgt, hover: 0 };
        st.last_click = None;
        return;
    }
    if act == Act::Logs {
        start_logs(st, tgt);
        return;
    }
    if act == Act::Status {
        start_text(st, tgt, act, tx);
        return;
    }
    // destrutivas passam pela confirmação
    st.mode = Mode::Confirm { tgt, act };
}

fn move_sel(st: &mut State, d: i32) {
    let len = cur_len(st, st.focus) as i32;
    let sel = match st.focus {
        Focus::Docker => &mut st.sel_docker,
        Focus::Units => &mut st.sel_units,
        Focus::Ports => &mut st.sel_ports,
        Focus::Overview => &mut st.sel_over,
    };
    *sel = (*sel as i32 + d).clamp(0, len.saturating_sub(1)) as usize;
}

fn move_scroll(st: &mut State, d: i32) {
    let len = cur_len(st, st.focus) as i32;
    let sel = match st.focus {
        Focus::Docker => &mut st.scroll_docker,
        Focus::Units => &mut st.scroll_units,
        Focus::Ports => &mut st.scroll_ports,
        Focus::Overview => return, // visão pc não rola
    };
    *sel = (*sel as i32 + d).clamp(0, len.saturating_sub(1)) as usize;
}

// ---- ações ----

fn spawn_action(tgt: Target, act: Act, tx: Sender<Msg>) {
    let (p, args) = act.cmd(&tgt);
    let label = format!("{} {}", act.verb(), tgt.name);
    std::thread::spawn(move || {
        let mut cmd = Command::new(p);
        // kills e prunes não dependem de contexto docker
        if p == "docker"
            && let Some(x) = get_ctx()
        {
            cmd.args(["--context", &x]);
        }
        match cmd.args(args).output() {
            Ok(o) => {
                let icon = if o.status.success() { "✔" } else { "✖" };
                let _ = tx.send(Msg::Done(format!("{icon} {label}")));
                let _ = tx.send(Msg::Snap(snapshot()));
            }
            Err(e) => {
                let _ = tx.send(Msg::Done(format!("✖ {e}")));
            }
        }
    });
}

// logs = stream ao vivo; fecha popup → rx morre → docker/journalctl caem por EPIPE
fn start_logs(st: &mut State, tgt: Target) {
    st.last_click = None;
    let (ltx, lrx) = mpsc::channel();
    st.mode = Mode::Logs {
        tgt: tgt.clone(),
        text: Vec::new(),
        rx: lrx,
    };
    let cmdline = match tgt.caller {
        Caller::Docker => {
            let c = get_ctx().map_or(String::new(), |x| format!("--context {x} "));
            format!("docker {c}logs -f --tail 80 {} 2>&1", tgt.name)
        }
        Caller::Unit => {
            let u = if tgt.user { "--user " } else { "" };
            format!("journalctl {u}-f -u {} -n 80 --no-pager 2>&1", tgt.name)
        }
        Caller::Proc => return, // proc não tem logs
    };
    std::thread::spawn(move || {
        let Ok(mut p) = Command::new("sh")
            .arg("-c")
            .arg(cmdline)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
        else {
            return;
        };
        if let Some(sa) = p.stdout.take() {
            for l in BufReader::new(sa).lines().map_while(Result::ok) {
                if ltx.send(l).is_err() {
                    break;
                }
            }
        }
        let _ = p.wait();
    });
}

fn start_act(st: &mut State, tgt: Target, act: Act, tx: &Sender<Msg>) {
    if act == Act::Logs {
        start_logs(st, tgt);
        return;
    }
    if act == Act::Status {
        start_text(st, tgt, act, tx);
        return;
    }
    st.mode = Mode::Loading {
        label: format!("{} {}", act.verb(), tgt.name),
        at: Instant::now(),
    };
    spawn_action(tgt, act, tx.clone());
}

// texto estático (docker inspect / systemctl status) num popup rolável
fn start_text(st: &mut State, tgt: Target, act: Act, tx: &Sender<Msg>) {
    st.last_click = None;
    let cmdline = match tgt.caller {
        Caller::Docker => {
            let c = get_ctx().map_or(String::new(), |x| format!("--context {x} "));
            format!("docker {c}inspect {} 2>&1", tgt.name)
        }
        Caller::Unit => {
            let u = if tgt.user { "--user " } else { "" };
            format!("systemctl {u}status {} --no-pager -l 2>&1", tgt.name)
        }
        Caller::Proc => return,
    };
    st.mode = Mode::Loading {
        label: format!("{} {}", act.verb(), tgt.name),
        at: Instant::now(),
    };
    let title = format!(" {} — {}", act.verb(), tgt.name);
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        let mut text = run_out({
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmdline);
            c
        });
        text.truncate(500);
        let _ = tx2.send(Msg::Text { title, text });
    });
}

fn apply_ctx(st: &mut State, name: String, tx: &Sender<Msg>) {
    set_ctx(Some(name.clone()));
    st.msg = Some((format!("docker context → {name}"), Instant::now()));
    st.mode = Mode::Browse;
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        let _ = tx2.send(Msg::Snap(snapshot()));
    });
}

fn spawn_disk(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let df = json_lines(&run_out(docker_cmd(&["system", "df", "--format", "{{json .}}"])));
        let cats = df_all(&df);
        let total = cats.iter().map(|(_, v)| *v).sum();
        let imgs_raw = json_lines(&run_out(docker_cmd(&["images", "--format", "{{json .}}"])));
        let mut images: Vec<(String, u64)> = imgs_raw
            .iter()
            .filter_map(|v| {
                let repo = v["Repository"].as_str()?;
                let tag = v["Tag"].as_str().unwrap_or("<none>");
                if repo == "<none>" {
                    return None;
                }
                Some((
                    format!("{repo}:{tag}"),
                    size_bytes(v["Size"].as_str().unwrap_or("")),
                ))
            })
            .collect();
        images.sort_by_key(|i| i.1);
        images.reverse();
        let _ = tx.send(Msg::Disk(DiskData { total, cats, images }));
    });
}

// ---- input ----

fn handle_key(st: &mut State, k: KeyEvent, tx: &Sender<Msg>) -> bool {
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        return false;
    }
    match &mut st.mode {
        Mode::Browse => match k.code {
            KeyCode::Char('q') => return false,
            KeyCode::Char('r') => {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Snap(snapshot()));
                });
            }
            KeyCode::Tab => {
                st.focus = match st.focus {
                    Focus::Docker => Focus::Units,
                    Focus::Units => Focus::Ports,
                    Focus::Ports => Focus::Overview,
                    Focus::Overview => Focus::Docker,
                };
                set_over_split(st.focus == Focus::Overview);
            }
            KeyCode::Char('a') => {
                set_all(!get_all());
                let msg = if get_all() {
                    "systemd: mostrando todos os units"
                } else {
                    "systemd: só rodando"
                };
                st.msg = Some((msg.to_string(), Instant::now()));
                let tx2 = tx.clone();
                std::thread::spawn(move || {
                    let _ = tx2.send(Msg::Snap(snapshot()));
                });
            }
            KeyCode::Char('d') => {
                st.mode = Mode::Loading {
                    label: "medindo uso de disco".into(),
                    at: Instant::now(),
                };
                spawn_disk(tx.clone());
            }
            KeyCode::Char('x') => {
                st.mode = Mode::Loading {
                    label: "listando contexts".into(),
                    at: Instant::now(),
                };
                let tx2 = tx.clone();
                std::thread::spawn(move || {
                    let raw = run_out(docker_cmd(&["context", "ls", "--format", "{{json .}}"]));
                    let items: Vec<(String, bool)> = json_lines(&raw)
                        .iter()
                        .filter_map(|v| {
                            let n = v["Name"].as_str()?.to_string();
                            let cur = v["Current"].as_bool().unwrap_or_else(|| {
                                v["Current"]
                                    .as_str()
                                    .map_or(false, |s| s.contains('t') || s.contains('*'))
                            });
                            Some((n, cur))
                        })
                        .collect();
                    let _ = tx2.send(Msg::Ctx(items));
                });
            }
            KeyCode::Down => move_sel(st, 1),
            KeyCode::Up => move_sel(st, -1),
            KeyCode::Right => move_scroll(st, 1),
            KeyCode::Left => move_scroll(st, -1),
            KeyCode::Enter => open_menu(st),
            KeyCode::Char('v') => open_detail(st),
            KeyCode::Char(':') => {
                st.mode = Mode::Cmd {
                    input: String::new(),
                    out: vec!["terminal do tema — 'ajuda' lista comandos · Esc fecha".into()],
                };
            }
            _ => {}
        },
        Mode::Cmd { input, out } => match k.code {
            KeyCode::Char(c) => input.push(c),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(input);
                if !cmd.trim().is_empty() {
                    out.push(format!("> {cmd}"));
                    out.extend(exec_cmd(&cmd));
                    let excess = out.len().saturating_sub(200);
                    out.drain(..excess);
                }
            }
            KeyCode::Esc => st.mode = Mode::Browse,
            _ => {}
        },
        Mode::Menu { tgt, acts, hover } => match k.code {
            KeyCode::Down => *hover = (*hover).saturating_add(1).min(acts.len().saturating_sub(1)),
            KeyCode::Up => *hover = (*hover).saturating_sub(1),
            KeyCode::Enter => {
                let act = acts[*hover];
                let tgt = tgt.clone();
                dispatch_act(st, tgt, act, tx);
            }
            KeyCode::Char('q') | KeyCode::Esc => st.mode = Mode::Browse,
            _ => {}
        },
        Mode::Detail { tgt, hover } => match k.code {
            KeyCode::Down => {
                let n = detail_acts(tgt.caller).len().saturating_sub(1);
                *hover = (*hover).saturating_add(1).min(n);
            }
            KeyCode::Up => *hover = (*hover).saturating_sub(1),
            KeyCode::Enter => {
                let act = detail_acts(tgt.caller)[*hover];
                let tgt = tgt.clone();
                dispatch_act(st, tgt, act, tx);
            }
            KeyCode::Esc | KeyCode::Char('q') => st.mode = Mode::Browse,
            _ => {}
        },
        Mode::Text { scroll, .. } => match k.code {
            KeyCode::Down => *scroll = scroll.saturating_add(1),
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Esc | KeyCode::Char('q') => st.mode = Mode::Browse,
            _ => st.mode = Mode::Browse,
        },
        Mode::Ctx { items, hover } => match k.code {
            KeyCode::Down => *hover = (*hover).saturating_add(1).min(items.len().saturating_sub(1)),
            KeyCode::Up => *hover = (*hover).saturating_sub(1),
            KeyCode::Enter => {
                if let Some((n, _)) = items.get(*hover) {
                    let n = n.clone();
                    apply_ctx(st, n, tx);
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => st.mode = Mode::Browse,
            _ => {}
        },
        Mode::Confirm { tgt, act } => match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let (t, a) = (tgt.clone(), *act);
                start_act(st, t, a, tx);
            }
            _ => st.mode = Mode::Browse,
        },
        Mode::Logs { .. } | Mode::Loading { .. } | Mode::Disk { .. } => st.mode = Mode::Browse,
    }
    true
}

fn handle_mouse(st: &mut State, m: MouseEvent, tx: &Sender<Msg>) {
    let (mx, my) = (m.column, m.row);
    match &mut st.mode {
        Mode::Browse => match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let areas = split(st.area);
                let vd = areas[0].height.saturating_sub(4) as usize;
                let vu = areas[1].height.saturating_sub(4) as usize;
                let hit = table_hit(areas[0], my, st.scroll_docker, cur_len(st, Focus::Docker), vd)
                    .map(|i| (Focus::Docker, i))
                    .or_else(|| match st.focus {
                        Focus::Ports => {
                            table_hit(areas[1], my, st.scroll_ports, cur_len(st, Focus::Ports), vu)
                                .map(|i| (Focus::Ports, i))
                        }
                        Focus::Overview => over_hit(areas[1], my, st.snap.as_ref())
                            .map(|i| (Focus::Overview, i)),
                        _ => table_hit(areas[1], my, st.scroll_units, cur_len(st, Focus::Units), vu)
                            .map(|i| (Focus::Units, i)),
                    });
                let (f, i) = match hit {
                    Some(x) => x,
                    None => {
                        st.last_click = None;
                        return;
                    }
                };
                st.focus = f;
                set_over_split(f == Focus::Overview);
                match f {
                    Focus::Docker => st.sel_docker = i,
                    Focus::Units => st.sel_units = i,
                    Focus::Ports => st.sel_ports = i,
                    Focus::Overview => st.sel_over = i,
                }
                let now = Instant::now();
                let prev = st.last_click.replace((now, Hit::Row(f, i)));
                if prev.map_or(false, |(t, h)| h == Hit::Row(f, i) && t.elapsed() < Duration::from_millis(350)) {
                    open_menu(st);
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let areas = split(st.area);
                let inside = |r: Rect| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height;
                let d = if m.kind == MouseEventKind::ScrollDown { 1 } else { -1 };
                if inside(areas[0]) {
                    let len = cur_len(st, Focus::Docker) as i32;
                    st.scroll_docker = (st.scroll_docker as i32 + d).clamp(0, len.saturating_sub(1)) as usize;
                } else if inside(areas[1]) && st.focus != Focus::Overview {
                    let (off, len) = if st.focus == Focus::Ports {
                        (st.scroll_ports, cur_len(st, Focus::Ports))
                    } else {
                        (st.scroll_units, cur_len(st, Focus::Units))
                    };
                    let off = (off as i32 + d).clamp(0, len as i32 - 1) as usize;
                    if st.focus == Focus::Ports {
                        st.scroll_ports = off;
                    } else {
                        st.scroll_units = off;
                    }
                }
            }
            _ => {}
        },
        Mode::Menu { tgt, acts, hover } => {
            if m.kind != MouseEventKind::Down(MouseButton::Left) {
                return;
            }
            let r = menu_rect(st.area, acts.len());
            let inside = mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height;
            if !inside {
                st.mode = Mode::Browse;
                return;
            }
            let row = (my - r.y - 2) as usize;
            if row >= acts.len() {
                return;
            }
            let now = Instant::now();
            let prev = st.last_click.replace((now, Hit::Item(row)));
            *hover = row;
            if prev.map_or(false, |(t, h)| h == Hit::Item(row) && t.elapsed() < Duration::from_millis(350)) {
                st.last_click = None;
                let act = acts[row];
                let tgt = tgt.clone();
                dispatch_act(st, tgt, act, tx);
            }
        }
        Mode::Detail { tgt, hover } => {
            if m.kind != MouseEventKind::Down(MouseButton::Left) {
                return;
            }
            let r = detail_rect(st.area, tgt);
            let inside = mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height;
            if !inside {
                st.mode = Mode::Browse;
                return;
            }
            // 2 bordas + título + info + blank + hint antes das ações
            let rel = my as i32 - r.y as i32 - info_len(tgt.caller) as i32 - 4;
            if rel < 0 {
                return;
            }
            let row = rel as usize;
            let acts = detail_acts(tgt.caller);
            if row >= acts.len() {
                return;
            }
            let now = Instant::now();
            let prev = st.last_click.replace((now, Hit::Item(row)));
            *hover = row;
            if prev.map_or(false, |(t, h)| h == Hit::Item(row) && t.elapsed() < Duration::from_millis(350)) {
                st.last_click = None;
                let act = acts[row];
                let tgt = tgt.clone();
                dispatch_act(st, tgt, act, tx);
            }
        }
        Mode::Ctx { items, hover } => {
            if m.kind != MouseEventKind::Down(MouseButton::Left) {
                return;
            }
            let r = popup_rect(st.area, 32, items.len() as u16 + 3);
            let inside = mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height;
            if !inside {
                st.mode = Mode::Browse;
                return;
            }
            let row = (my - r.y - 2) as usize;
            if row >= items.len() {
                return;
            }
            *hover = row;
            let now = Instant::now();
            let prev = st.last_click.replace((now, Hit::Item(row)));
            if prev.map_or(false, |(t, h)| h == Hit::Item(row) && t.elapsed() < Duration::from_millis(350)) {
                st.last_click = None;
                let n = items[row].0.clone();
                apply_ctx(st, n, tx);
            }
        }
        _ => {}
    }
}

// ---- render ----

// interpola duas cores (para o shimmer)
fn lerp(a: Color, b: Color, f: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => Color::Rgb(
            (r1 as f32 + (r2 - r1) as f32 * f) as u8,
            (g1 as f32 + (g2 - g1) as f32 * f) as u8,
            (b1 as f32 + (b2 - b1) as f32 * f) as u8,
        ),
        _ => a,
    }
}

// brilho que varre o texto, estilo shimmer — chars viram spans com brilho decrescente
fn shimmer(text: &str, at: Instant, base: Color) -> Vec<Span<'static>> {
    let hi = Color::Rgb(255, 255, 255);
    let n = text.chars().count();
    let pos = (at.elapsed().as_millis() as usize / 70) % (n + 8); // varre e pausa
    let mut spans = Vec::new();
    for (i, ch) in text.chars().enumerate() {
        let d = (i as i32 - pos as i32).abs();
        let col = match d {
            0 => hi,
            1 => lerp(base, hi, 0.6),
            2 => lerp(base, hi, 0.3),
            _ => base,
        };
        spans.push(Span::styled(ch.to_string(), Style::new().fg(col)));
    }
    spans
}

static BOOT: OnceLock<Instant> = OnceLock::new();

// ---- logo "ratscope" — glifos pixel-art desenhados à mão, renderizados com ▀ ----
// grade lógica: 14 linhas × 47 colunas; cada célula do terminal = 2 pixels verticais
// (offset = linhas vazias acima; baseline dos glifos x-height = linha 11)
const GLYPHS: &[(&str, usize, &[&str])] = &[
    (
        "r",
        4,
        &["#.##.", "##..#", "#....", "#....", "#....", "#....", "#....", "#...."],
    ),
    (
        "a",
        4,
        &[".###.", "....#", "....#", ".###.", "#...#", "#...#", "#...#", ".####"],
    ),
    (
        "t",
        2,
        &[
            "..#..", "..#..", "..#..", "####.", "..#..", "..#..", "..#..", "..#..", "..##.", "..##.",
        ],
    ),
    (
        "s",
        4,
        &[".####", "#....", "#....", ".###.", "....#", "....#", "....#", "####."],
    ),
    (
        "c",
        4,
        &[".####", "#....", "#....", "#....", "#....", "#....", "#....", ".####"],
    ),
    (
        "o",
        4,
        &[".###.", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    ),
    (
        "p",
        4,
        &["####.", "#...#", "#...#", "#...#", "####.", "#....", "#....", "#....", "#....", "#...."],
    ),
    (
        "e",
        4,
        &[".###.", "#...#", "#...#", "#####", "#....", "#....", "#...#", ".###."],
    ),
];

fn logo_grid() -> Vec<Vec<bool>> {
    let mut grid = vec![vec![false; 47]; 14];
    let mut x = 0usize;
    for (_, off, rows) in GLYPHS {
        for (ry, row) in rows.iter().enumerate() {
            for (cx, ch) in row.chars().enumerate() {
                if ch == '#' {
                    grid[off + ry][x + cx] = true;
                }
            }
        }
        x += 6; // 5 de glifo + 1 de respiro
    }
    grid
}

// desenha a logo centrada; devolve a área ocupada (None se não cabe)
fn draw_logo(f: &mut Frame, area: Rect) -> Option<Rect> {
    const W: usize = 47; // 8 letras × 5 + 7 espaços
    const ROWS: usize = 14;
    // pixel largo: 2 colunas por pixel compensa a célula de terminal ser mais alta que larga
    let wide = (area.width as usize) >= W * 2;
    let cols = if wide { W * 2 } else { W };
    if (area.width as usize) < W || (area.height as usize) < ROWS / 2 + 2 {
        return None;
    }
    let grid = logo_grid();
    // gradiente mint → teal → verde escuro por linha lógica
    let a = Color::Rgb(198, 230, 224);
    let b = Color::Rgb(83, 143, 135);
    let c = Color::Rgb(39, 68, 66);
    let grad: Vec<Color> = (0..ROWS)
        .map(|y| {
            if y < 7 {
                lerp(a, b, y as f32 / 7.0)
            } else {
                lerp(b, c, (y - 7) as f32 / 7.0)
            }
        })
        .collect();
    let bg = c_bg();
    let mut lines: Vec<Line> = Vec::with_capacity(ROWS / 2);
    for pair in 0..ROWS / 2 {
        let mut spans = Vec::with_capacity(cols);
        for cx in 0..W {
            let top = grid[pair * 2][cx];
            let bot = grid[pair * 2 + 1][cx];
            let span = Span::styled(
                "▀",
                Style::new()
                    .fg(if top { grad[pair * 2] } else { bg })
                    .bg(if bot { grad[pair * 2 + 1] } else { bg }),
            );
            spans.push(span.clone());
            if wide {
                spans.push(span);
            }
        }
        lines.push(Line::from(spans));
    }
    let r = Rect {
        x: area.x + (area.width.saturating_sub(cols as u16)) / 2,
        y: area.y + (area.height.saturating_sub(ROWS as u16 / 2 + 2)) / 2,
        width: cols as u16,
        height: ROWS as u16 / 2,
    };
    f.render_widget(Paragraph::new(lines), r);
    Some(r)
}

fn spin(at: Instant) -> &'static str {
    SPINNER[(at.elapsed().as_millis() / 80) as usize % SPINNER.len()]
}

// moldura conforme o tema, fundo sólido da paleta
fn pblock(col: Color) -> Block<'static> {
    let set = match t().border {
        BorderKind::Pixel => border::QUADRANT_OUTSIDE,
        BorderKind::Linha => border::PLAIN,
        BorderKind::Dupla => border::DOUBLE,
        BorderKind::Grossa => border::THICK,
    };
    Block::bordered()
        .border_set(set)
        .border_style(Style::new().fg(col))
        .style(Style::new().bg(c_bg()))
}

// moldura + título na primeira linha DENTRO da caixa; devolve a área do conteúdo
fn box_title(f: &mut Frame, area: Rect, title: impl Into<Line<'static>>, col: Color) -> Rect {
    let b = pblock(col);
    f.render_widget(&b, area);
    let inner = b.inner(area);
    let chunks: [Rect; 2] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    let line: Line<'static> = title.into();
    f.render_widget(
        Paragraph::new(line).style(Style::new().fg(col).add_modifier(Modifier::BOLD)),
        chunks[0],
    );
    chunks[1]
}

fn draw(f: &mut Frame, st: &mut State) {
    st.area = f.area();
    f.render_widget(Block::default().style(Style::new().bg(c_bg())), st.area);
    let areas = split(st.area);

    let Some(snap) = st.snap.as_ref() else {
        let b = pblock(c_cy());
        f.render_widget(&b, st.area);
        let inner = b.inner(st.area);
        let msg = || {
            Line::from(shimmer(
                "esperando primeira leitura…",
                *BOOT.get_or_init(Instant::now),
                c_cy(),
            ))
        };
        match draw_logo(f, inner) {
            Some(lr) => {
                f.render_widget(
                    Paragraph::new(msg()).alignment(Alignment::Center),
                    Rect {
                        x: inner.x,
                        y: lr.y + lr.height + 1,
                        width: inner.width,
                        height: 1,
                    },
                );
            }
            None => {
                f.render_widget(Paragraph::new(msg()), inner);
            }
        }
        return;
    };

    // ---- tabela docker ----
    let title = if snap.docker_ok {
        format!(" ■ docker — {} containers", snap.containers.len())
    } else {
        " ■ docker — indisponível".to_string()
    };
    let head = Style::new().fg(c_cy()).add_modifier(Modifier::BOLD);
    let sel_style = Style::new().fg(c_wht()).bg(c_gray());

    let rows: Vec<Row> = snap
        .containers
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let col = if c.status.contains("healthy") {
                c_grn()
            } else if c.status.contains("Exited") || c.status.contains("Dead") {
                c_red()
            } else if c.status.contains("Restarting") {
                c_yel()
            } else {
                c_cy()
            };
            let mut style = Style::new().fg(col);
            if st.focus == Focus::Docker && i == st.sel_docker {
                style = sel_style;
            }
            let up = if snap.dk_total > 0 && c.size_b > 0 {
                pie(c.size_b as f64 / snap.dk_total as f64)
            } else {
                "○"
            };
            let graf = st.hist.get(&c.name).map_or_else(String::new, |h| spark(h));
            Row::new([
                c.name.clone(),
                c.image.clone(),
                c.status.clone(),
                c.cpu.clone(),
                c.mem.clone(),
                format!("{up} {}", fmt_size(c.size_b)),
                graf,
            ])
            .style(style)
        })
        .collect();
    let body = box_title(f, areas[0], title, c_cy());
    let vis = body.height.saturating_sub(1) as usize;
    let t_docker = Table::new(
        rows.into_iter().skip(st.scroll_docker).take(vis),
        [
            Constraint::Percentage(18),
            Constraint::Percentage(24),
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(9),
        ],
    )
    .header(Row::new(["NOME", "IMAGEM", "STATUS", "CPU", "MEM", "SSD", "GRAF"]).style(head));
    f.render_widget(t_docker, body);

    // ---- bloco de baixo: systemd · portas · pc ----
    let bottom = areas[1];
    match st.focus {
        Focus::Overview => draw_overview(f, snap, &st.hist_global, st.sel_over, bottom),
        _ => {
            let is_ports = st.focus == Focus::Ports;
            let urows: Vec<Row> = if is_ports {
                snap.ports
                    .iter()
                    .enumerate()
                    .map(|(i, (addr, pr))| {
                        let mut style = Style::new();
                        if st.focus == Focus::Ports && i == st.sel_ports {
                            style = sel_style;
                        }
                        Row::new([addr.clone(), pr.clone()]).style(style)
                    })
                    .collect()
            } else {
                snap.units
                    .iter()
                    .enumerate()
                    .map(|(i, u)| {
                        let mut style = Style::new().fg(if st.focus == Focus::Units && i == st.sel_units {
                            c_wht()
                        } else {
                            match u.state.as_str() {
                                "active" => c_grn(),
                                "failed" => c_red(),
                                "activating" | "reloading" => c_yel(),
                                _ => c_gray(),
                            }
                        });
                        if st.focus == Focus::Units && i == st.sel_units {
                            style = sel_style;
                        }
                        let mark = match (u.user, u.state == "active") {
                            (true, true) => "◉",
                            (true, false) => "◎",
                            (false, true) => "●",
                            (false, false) => "○",
                        };
                        Row::new([
                            mark.to_string(),
                            u.name.clone(),
                            u.state.clone(),
                            u.desc.clone(),
                        ])
                        .style(style)
                    })
                    .collect()
            };
            let (tr_title, cols, hdr) = if is_ports {
                (
                    format!(" ■ portas escutando — {} servers", snap.ports.len()),
                    vec![Constraint::Percentage(34), Constraint::Percentage(66)],
                    Row::new(["SOCKET", "PROCESSO"]),
                )
            } else {
                let modo = if get_all() { "todos os units" } else { "só rodando" };
                (
                    format!(
                        " ■ systemd — {} services — {} · 'a' alterna",
                        snap.units.len(),
                        modo
                    ),
                    vec![
                        Constraint::Length(2),
                        Constraint::Percentage(28),
                        Constraint::Length(10),
                        Constraint::Percentage(60),
                    ],
                    Row::new(["", "SERVIÇO", "ESTADO", "DESCRIÇÃO"]),
                )
            };
            let body = box_title(f, bottom, tr_title, if is_ports { c_grn() } else { c_mag() });
            let uvis = body.height.saturating_sub(1) as usize;
            let t_bottom = Table::new(
                urows
                    .into_iter()
                    .skip(if is_ports { st.scroll_ports } else { st.scroll_units })
                    .take(uvis),
                cols,
            )
            .header(hdr.style(Style::new().fg(c_mag()).add_modifier(Modifier::BOLD)));
            f.render_widget(t_bottom, body);
        }
    }

    // ---- rodapé ----
    let foot: Line<'static> = match &st.mode {
        _ if st.msg.as_ref().map_or(false, |(_, at)| at.elapsed() < Duration::from_secs(5)) => {
            Line::from(st.msg.as_ref().unwrap().0.clone())
        }
        Mode::Loading { label, at } => {
            let mut v = vec![Span::styled(
                format!("{} ", spin(*at)),
                Style::new().fg(c_yel()),
            )];
            v.extend(shimmer(label, *at, c_yel()));
            Line::from(v)
        }
        _ => Line::from("↓↑ seleciona · Tab: docker/systemd/portas/pc · Enter menu · v detalhes · : tema · d disco · x contexto · q sair"),
    };
    f.render_widget(
        Paragraph::new(foot).style(Style::new().fg(c_wht()).bg(c_blu())),
        areas[2],
    );

    // ---- popups ----
    match &st.mode {
        Mode::Menu { tgt, acts, hover } => {
            let r = menu_rect(st.area, acts.len());
            f.render_widget(Clear, r);
            let who = match tgt.caller {
                Caller::Docker => "docker",
                Caller::Unit => "systemd",
                Caller::Proc => "processo",
            };
            let body = box_title(f, r, format!("{who}:{}", tgt.name), c_yel());
            let items: Vec<ListItem> = acts
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let style = if i == *hover {
                        Style::new().fg(c_blk()).bg(c_cy())
                    } else {
                        Style::new()
                    };
                    ListItem::new(format!(" {}", a.label())).style(style)
                })
                .collect();
            f.render_widget(List::new(items), body);
        }
        Mode::Confirm { tgt, act } => {
            let r = popup_rect(st.area, 46, 6);
            f.render_widget(Clear, r);
            let body = box_title(f, r, " confirmar ", c_yel());
            f.render_widget(
                Paragraph::new(format!(
                    " {} '{}'?\n\n y = sim · n/Esc = cancelar",
                    act.verb(),
                    tgt.name
                ))
                .style(Style::new().fg(c_yel())),
                body,
            );
        }
        Mode::Logs { tgt, text, .. } => {
            let r = popup_rect(
                st.area,
                st.area.width.saturating_sub(6).min(100),
                st.area.height.saturating_sub(4).min(32),
            );
            f.render_widget(Clear, r);
            let body = box_title(
                f,
                r,
                format!(" logs ao vivo — {} (Esc fecha)", tgt.name),
                c_yel(),
            );
            let keep = body.height as usize;
            let tail = &text[text.len().saturating_sub(keep)..];
            f.render_widget(
                Paragraph::new(
                    tail.iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                body,
            );
        }
        Mode::Disk { data } => {
            let imgs: Vec<&(String, u64)> = data.images.iter().take(14).collect();
            let nrows = 1 + 1 + data.cats.len() + 1 + imgs.len() + 1;
            let h = (nrows as u16 + 5).min(st.area.height.saturating_sub(2)).max(6);
            let w = 56u16.min(st.area.width.saturating_sub(4)).max(34);
            let r = popup_rect(st.area, w, h);
            f.render_widget(Clear, r);
            let body = box_title(f, r, " ■ uso de SSD (docker)", c_yel());
            let mut lines_s = String::new();
            if data.total == 0 {
                lines_s.push_str(" docker indisponível — sem medidas de disco");
            } else {
                lines_s.push_str(&format!(" docker total: {}\n\n", fmt_size(data.total)));
                for (t, v) in &data.cats {
                    let pl = pie(*v as f64 / data.total as f64);
                    lines_s.push_str(&format!(" {} {:<13} {:>9}\n", pl, cat_label(t), fmt_size(*v)));
                }
                lines_s.push_str("\n maiores imagens:\n");
                if imgs.is_empty() {
                    lines_s.push_str(" (nenhuma imagem puxada)");
                } else {
                    for (repo, b) in imgs {
                        let pl = pie(*b as f64 / data.total as f64);
                        lines_s.push_str(&format!(
                            " {} {:<42} {:>9}\n",
                            pl,
                            trunc(repo, 43),
                            fmt_size(*b)
                        ));
                    }
                }
            }
            f.render_widget(Paragraph::new(lines_s), body);
        }
        Mode::Ctx { items, hover } => {
            let r = popup_rect(st.area, 32, items.len() as u16 + 3);
            f.render_widget(Clear, r);
            let body = box_title(f, r, " docker contexts ", c_cy());
            let ls: Vec<ListItem> = items
                .iter()
                .enumerate()
                .map(|(i, (n, cur))| {
                    let mark = if *cur { "●" } else { "○" };
                    let style = if i == *hover {
                        Style::new().fg(c_blk()).bg(c_cy())
                    } else {
                        Style::new()
                    };
                    ListItem::new(format!(" {mark} {n}")).style(style)
                })
                .collect();
            f.render_widget(List::new(ls), body);
        }
        Mode::Detail { tgt, hover } => {
            let acts = detail_acts(tgt.caller);
            let (who, col) = match tgt.caller {
                Caller::Docker => ("docker", c_cy()),
                Caller::Unit => ("systemd", c_mag()),
                Caller::Proc => ("processo", c_lred()),
            };
            // info ao vivo do snapshot atual
            let mut info: Vec<(String, String)> = Vec::new();
            let mut sumiu = false;
            if let Some(s) = st.snap.as_ref() {
                match tgt.caller {
                    Caller::Docker => match s.containers.iter().find(|c| c.name == tgt.name) {
                        Some(c) => {
                            info.push(("nome".into(), c.name.clone()));
                            info.push(("imagem".into(), c.image.clone()));
                            info.push(("status".into(), c.status.clone()));
                            info.push(("cpu".into(), c.cpu.clone()));
                            info.push(("mem".into(), c.mem.clone()));
                            info.push(("ssd".into(), fmt_size(c.size_b)));
                        }
                        None => sumiu = true,
                    },
                    Caller::Unit => match s.units.iter().find(|u| u.name == tgt.name) {
                        Some(u) => {
                            info.push(("nome".into(), u.name.clone()));
                            info.push(("estado".into(), u.state.clone()));
                            info.push(("escopo".into(), if u.user { "usuário" } else { "sistema" }.into()));
                            info.push(("descrição".into(), u.desc.clone()));
                        }
                        None => sumiu = true,
                    },
                    Caller::Proc => match s.procs.iter().find(|p| p.pid.to_string() == tgt.name) {
                        Some(p) => {
                            info.push(("pid".into(), p.pid.to_string()));
                            info.push(("nome".into(), p.comm.clone()));
                            info.push(("ram".into(), fmt_size(p.rss_b)));
                        }
                        None => sumiu = true,
                    },
                }
            }
            let mut lines: Vec<Line> = Vec::new();
            if sumiu {
                lines.push(Line::from(Span::styled(
                    " alvo sumiu do snapshot — Esc volta",
                    Style::new().fg(c_red()),
                )));
            }
            for (k, v) in &info {
                lines.push(Line::from(vec![
                    Span::styled(format!(" {k:<10} "), Style::new().fg(c_gray())),
                    Span::styled(v.clone(), Style::new().fg(c_wht())),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ações — ↑↓ move · Enter executa · clique fora fecha",
                Style::new().fg(c_gray()),
            )));
            for (i, a) in acts.iter().enumerate() {
                let style = if i == *hover {
                    Style::new().fg(c_blk()).bg(c_cy())
                } else {
                    Style::new().fg(c_wht())
                };
                lines.push(Line::from(Span::styled(format!(" {}", a.label()), style)));
            }
            let r = detail_rect(st.area, tgt);
            f.render_widget(Clear, r);
            let body = box_title(f, r, format!(" ■ {who}: {}", tgt.name), col);
            f.render_widget(Paragraph::new(lines), body);
        }
        Mode::Cmd { input, out } => {
            let h = 12u16.min(st.area.height.saturating_sub(2));
            let r = Rect {
                x: st.area.x,
                y: st.area.y + st.area.height - h,
                width: st.area.width,
                height: h,
            };
            f.render_widget(Clear, r);
            let body = box_title(
                f,
                r,
                " terminal — 'ajuda' lista comandos · Esc fecha ",
                c_cy(),
            );
            let keep = h.saturating_sub(4) as usize;
            let tail = &out[out.len().saturating_sub(keep)..];
            let mut lines: Vec<Line> = tail
                .iter()
                .map(|l| {
                    let col = if l.starts_with('✖') {
                        c_red()
                    } else if l.starts_with('✔') {
                        c_grn()
                    } else {
                        c_gray()
                    };
                    Line::from(Span::styled(l.clone(), Style::new().fg(col)))
                })
                .collect();
            lines.push(Line::from(vec![
                Span::styled("> ", Style::new().fg(c_cy()).add_modifier(Modifier::BOLD)),
                Span::styled(input.clone(), Style::new().fg(c_wht())),
            ]));
            f.render_widget(Paragraph::new(lines), body);
        }
        Mode::Text { title, text, scroll } => {
            let r = popup_rect(
                st.area,
                st.area.width.saturating_sub(6).min(100),
                st.area.height.saturating_sub(4).min(32),
            );
            f.render_widget(Clear, r);
            let body = box_title(f, r, title.clone(), c_yel());
            let keep = body.height as usize;
            let scroll = (*scroll).min(text.len().saturating_sub(keep));
            let win = &text[scroll..(scroll + keep).min(text.len())];
            f.render_widget(
                Paragraph::new(win.iter().map(String::as_str).collect::<Vec<_>>().join("\n")),
                body,
            );
        }
        Mode::Loading { label, at } => {
            let r = popup_rect(st.area, 34, 3);
            f.render_widget(Clear, r);
            let b = pblock(c_yel());
            f.render_widget(&b, r);
            let mut v = vec![Span::styled(
                format!(" {} ", spin(*at)),
                Style::new().fg(c_yel()),
            )];
            v.extend(shimmer(label, *at, c_yel()));
            f.render_widget(Paragraph::new(Line::from(v)), b.inner(r));
        }
        Mode::Browse => {}
    }
}

// visão "pc": cpu global + pizza geral + ram por processo (clicável, kill) + ssd por montagem
fn draw_overview(f: &mut Frame, snap: &Snapshot, hist_global: &[f32], sel_over: usize, area: Rect) {
    let pc = &snap.pc;
    let inner_w = area.width.saturating_sub(4) as usize;
    let n_p = over_nprocs(area);
    let mut lines: Vec<Line> = Vec::new();

    // cpu global
    let cpu_col = if pc.cpu_pct < 40.0 {
        c_grn()
    } else if pc.cpu_pct < 75.0 {
        c_yel()
    } else {
        c_red()
    };
    let spark = spark_full(hist_global);
    lines.push(Line::from(vec![
        Span::styled(" cpu ", Style::new().fg(c_cy()).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:>4.0}%", pc.cpu_pct),
            Style::new().fg(cpu_col).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {spark}"), Style::new().fg(cpu_col)),
        Span::styled("  1s por ponto", Style::new().fg(c_gray())),
    ]));

    // pizza geral — 1 cor por coisa; cada partição de disco com cor própria
    let disk_size: u64 = pc.mounts.iter().map(|m| m.1).sum();
    let pool = pc.mem_total + pc.swap_total + disk_size;
    let mut parts: Vec<(u64, Color)> = vec![(pc.mem_used, c_cy()), (pc.swap_used, c_mag())];
    let mut legend = vec![
        Span::styled("■ ram", Style::new().fg(c_cy())),
        Span::raw(" "),
        Span::styled("■ swap", Style::new().fg(c_mag())),
        Span::raw(" "),
    ];
    let mcols = [c_grn(), c_lred(), c_lblu()];
    for (i, (target, _, used)) in pc.mounts.iter().enumerate() {
        let col = mcols[i % mcols.len()];
        parts.push((*used, col));
        legend.push(Span::styled(
            format!("■ {}", trunc(target, 12)),
            Style::new().fg(col),
        ));
        legend.push(Span::raw(" "));
    }
    parts.push((snap.dk_total, c_yel()));
    legend.push(Span::styled("■ docker", Style::new().fg(c_yel())));
    legend.push(Span::raw(" "));
    legend.push(Span::styled("▒ livre", Style::new().fg(c_gray())));
    lines.push(Line::from(vec![
        Span::styled(" total pc ", Style::new().fg(c_cy()).add_modifier(Modifier::BOLD)),
        Span::from(" "),
    ]));
    let bar = bar_stack(inner_w.max(10), pool, &parts);
    lines.push(Line::from(bar.clone()));
    lines.push(Line::from(legend));
    lines.push(Line::from(""));

    // ram por coisa (processos) — clicável, pizza por quota de memória
    lines.push(Line::from(Span::styled(
        " ram por coisa — duplo-clique/Enter pra kill",
        Style::new().fg(c_mag()).add_modifier(Modifier::BOLD),
    )));
    for (i, p) in snap.procs.iter().take(n_p).enumerate() {
        let frac = if pc.mem_total > 0 {
            p.rss_b as f64 / pc.mem_total as f64
        } else {
            0.0
        };
        let mut style = Style::new().fg(pal()[i % pal().len()]);
        if i == sel_over {
            style = Style::new().fg(c_wht()).bg(c_gray());
        }
        lines.push(Line::from(Span::styled(
            format!(" {} {:<16} {:>9}", pie(frac), trunc(&p.comm, 16), fmt_size(p.rss_b)),
            style,
        )));
    }

    // ssd por coisa
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ssd por coisa",
        Style::new().fg(c_mag()).add_modifier(Modifier::BOLD),
    )));
    for (i, (target, size, used)) in pc.mounts.iter().enumerate().take(4) {
        let frac = if *size > 0 { *used as f64 / *size as f64 } else { 0.0 };
        lines.push(Line::from(Span::styled(
            format!(
                " {} {:<16} {:>9} / {:>9}",
                pie(frac),
                trunc(target, 16),
                fmt_size(*used),
                fmt_size(*size)
            ),
            Style::new().fg(mcols[i % mcols.len()]),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            " docker total: {} — 'd' abre o popup de disco",
            fmt_size(snap.dk_total)
        ),
        Style::new().fg(c_yel()),
    )));

    let body = box_title(f, area, " ■ pc — tudo que roda e ocupa ", c_cy());
    f.render_widget(Paragraph::new(lines), body);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_hex_ok() {
        assert_eq!(parse_hex("#FF004D"), Some(Color::Rgb(255, 0, 77)));
        assert_eq!(parse_hex("f0a"), Some(Color::Rgb(255, 0, 170)));
        assert_eq!(parse_hex("zzz"), None);
        assert_eq!(parse_hex("#12345"), None);
    }
    #[test]
    fn logo_legivel() {
        let g = logo_grid();
        for row in &g {
            println!("{}", row.iter().map(|p| if *p { '█' } else { ' ' }).collect::<String>());
        }
        assert_eq!(g.len(), 14);
        assert!(g.iter().any(|r| r.iter().any(|p| *p)));
    }
}

// ---- boot ----

fn main() {
    if std::env::args().any(|s| s == "--version") {
        println!("ratscope 0.9 — shimmer · tema pixel · terminal de tema (:) · detalhes por alvo · pause/reload/kill -9/status");
        return;
    }
    run_and_exit();
}

fn run_and_exit() {
    std::process::exit(match boot() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    });
}

fn boot() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let res = run_app();
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    res
}

fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    carregar_config();
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (tx, rx) = mpsc::channel();
    let px = tx.clone();
    std::thread::spawn(move || {
        loop {
            if px.send(Msg::Snap(snapshot())).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_secs(TICK_SECS));
        }
    });
    let mut st = State {
        snap: None,
        focus: Focus::Docker,
        sel_docker: 0,
        sel_units: 0,
        sel_ports: 0,
        sel_over: 0,
        scroll_docker: 0,
        scroll_units: 0,
        scroll_ports: 0,
        hist: HashMap::new(),
        hist_global: Vec::new(),
        mode: Mode::Browse,
        last_click: None,
        area: Rect::default(),
        msg: None,
    };
    loop {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Snap(s) => {
                    for c in &s.containers {
                        let e = st.hist.entry(c.name.clone()).or_default();
                        e.push(parse_pct(&c.cpu));
                        if e.len() > 48 {
                            e.remove(0);
                        }
                    }
                    st.hist_global.push(s.pc.cpu_pct);
                    if st.hist_global.len() > 48 {
                        st.hist_global.remove(0);
                    }
                    st.snap = Some(s);
                }
                Msg::Done(m) => st.msg = Some((m, Instant::now())),
                Msg::Disk(d) => st.mode = Mode::Disk { data: d },
                Msg::Ctx(i) => st.mode = Mode::Ctx { items: i, hover: 0 },
                Msg::Text { title, text } => st.mode = Mode::Text { title, text, scroll: 0 },
            }
        }
        if let Mode::Logs { text, rx: lrx, .. } = &mut st.mode {
            while let Ok(l) = lrx.try_recv() {
                text.push(l);
                if text.len() > 500 {
                    text.remove(0);
                }
            }
        }
        term.draw(|f| draw(f, &mut st))?;
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if !handle_key(&mut st, k, &tx) {
                        return Ok(());
                    }
                }
                Event::Mouse(m) => handle_mouse(&mut st, m, &tx),
                _ => {}
            }
        }
    }
}
