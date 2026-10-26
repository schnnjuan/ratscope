// terminal ':' — parse → exec → efeitos declarativos.
//
// Nenhuma mutação de estado global acontece durante o parse/exec: `exec`
// devolve um `Output` com linhas de resposta e `Effect`s declarativos, e o
// event loop aplica cada efeito via `apply()`. Assim o terminal é testável
// sem UI, os efeitos são auditáveis e um futuro undo é trivial.

use ratatui::style::Color;

use crate::{
    get_active_docker_host, get_docker_hosts, hex_of, lang, mudar_tema, parse_hex,
    set_active_docker_host, set_docker_hosts, set_lang, t, tr, tr1, BorderKind, GaugeKind, Lang,
    Theme, THEME_LIST,
};

// ---- catálogo de comandos (usado por help, sugestões e did_you_mean) ----

const SLASH_CMDS: &[(&str, &str)] = &[
    ("/help", "show this help"),
    ("/bg <hex>", "set background color"),
    (
        "/color <name> <hex>",
        "set color (cyan pink green yellow red gray blue peach orange white black)",
    ),
    ("/border <pixel|line|double|thick>", "set border style"),
    ("/gauge <square|circle>", "set gauge style"),
    (
        "/theme <name>",
        "set theme (pixel dracula gruvbox nord catppuccin tokyo solarized synthwave matrix candy ember mono carmesim)",
    ),
    ("/lang <en|pt>", "set language"),
    ("/show", "show current theme values"),
    ("/save", "save config to ~/.config/ratscope/config.json"),
    ("/reset", "reset to pixel theme"),
    ("/hosts", "list docker hosts"),
    (
        "/hosts add <endpoint>",
        "add docker host (tcp://... or ssh://...)",
    ),
    ("/hosts rm <endpoint>", "remove docker host"),
    ("/hosts set <endpoint>", "set active docker host"),
];

// ---- efeitos declarativos ----

/// slot de cor do tema que um comando pode alterar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    Bg,
    Cy,
    Mag,
    Grn,
    Yel,
    Red,
    Gray,
    Blu,
    Lblu,
    Lred,
    Wht,
    Blk,
}

/// mutação de estado pedida por um comando — aplicada fora do parse/exec
pub enum Effect {
    Color(ColorSlot, Color),
    Border(BorderKind),
    Gauge(GaugeKind),
    Theme(fn() -> Theme),
    ResetTheme,
    Lang(Lang),
    /// adiciona o host e já o seleciona como ativo
    AddHost(String),
    /// remove o host; se era o ativo, desativa
    RemoveHost(String),
    ActivateHost(String),
}

/// resposta de um comando: linhas para o buffer do terminal + efeitos a aplicar
pub struct Output {
    pub lines: Vec<Line>,
    pub effects: Vec<Effect>,
}

/// linha de resposta; `Ack` resolve o "✔ done/feito" DEPOIS que os efeitos
/// foram aplicados — assim `lang pt` responde no idioma novo, como sempre foi
pub enum Line {
    Raw(String),
    Ack,
}

impl Output {
    fn msg(lines: Vec<String>) -> Self {
        Self {
            lines: lines.into_iter().map(Line::Raw).collect(),
            effects: vec![],
        }
    }
    fn ok() -> Self {
        Self {
            lines: vec![Line::Ack],
            effects: vec![],
        }
    }
    fn bad(m: &str) -> Self {
        Self::msg(vec![format!("✖ {m}")])
    }
    fn with(mut self, e: Effect) -> Self {
        self.effects.push(e);
        self
    }
    /// linhas prontas pra exibir — chame DEPOIS de aplicar os efeitos
    pub fn lines_resolved(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|l| match l {
                Line::Raw(s) => s.clone(),
                Line::Ack => tr("✔ done", "✔ feito").to_string(),
            })
            .collect()
    }
}

// ---- parse ----

/// tokeniza a entrada: trim, remove o `/` opcional, separa comando dos args
fn parse(input: &str) -> Option<(String, Vec<String>)> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }
    let mut ws = raw.split_whitespace();
    let head = ws.next()?;
    let name = head.strip_prefix('/').unwrap_or(head).to_lowercase();
    let args = ws.map(str::to_string).collect();
    Some((name, args))
}

// ---- exec ----

/// executa um comando do terminal ':' — puro, só lê estado global
pub fn exec(input: &str) -> Output {
    let Some((name, args)) = parse(input) else {
        return Output::msg(vec![]);
    };
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match (name.as_str(), a.as_slice()) {
        ("help" | "ajuda" | "h", _) => Output::msg(help_lines()),
        ("lang", ["en"]) => Output::ok().with(Effect::Lang(Lang::En)),
        ("lang", ["pt"]) => Output::ok().with(Effect::Lang(Lang::Pt)),
        ("lang", _) => Output::bad("usage: /lang en|pt"),
        ("bg" | "fundo", [h]) => match parse_hex(h) {
            Some(c) => Output::ok().with(Effect::Color(ColorSlot::Bg, c)),
            None => Output::bad(&tr(
                "invalid hex (e.g. #10101A)",
                "hex inválido (ex: #10101A)",
            )),
        },
        ("color" | "cor", [nome, h]) => match parse_hex(h) {
            None => Output::bad(&tr(
                "invalid hex (e.g. #FF004D)",
                "hex inválido (ex: #FF004D)",
            )),
            Some(c) => match color_slot(nome) {
                Some(slot) => Output::ok().with(Effect::Color(slot, c)),
                None => Output::bad(&tr(
                    "unknown color — /help lists names",
                    "cor desconhecida — /ajuda lista nomes",
                )),
            },
        },
        ("border" | "borda", [k]) => match border_kind(k) {
            Some(b) => Output::ok().with(Effect::Border(b)),
            None => Output::bad(&tr(
                "use: /border pixel|line|double|thick",
                "use: /borda pixel|linha|dupla|grossa",
            )),
        },
        ("gauge" | "medidor", [k]) => match gauge_kind(k) {
            Some(g) => Output::ok().with(Effect::Gauge(g)),
            None => Output::bad(&tr(
                "use: /gauge square|circle",
                "use: /medidor quadrado|circulo",
            )),
        },
        ("theme" | "tema", [nome]) => match THEME_LIST.iter().find(|(n, _)| n == nome) {
            Some((_, f)) => Output::ok().with(Effect::Theme(*f)),
            None => Output::bad(&tr(
                "use: /theme pixel|dracula|gruvbox|nord|catppuccin|tokyo|solarized|synthwave|matrix|candy|ember|mono|carmesim",
                "use: /tema pixel|dracula|gruvbox|nord|catppuccin|tokyo|solarized|synthwave|matrix|candy|ember|mono|carmesim",
            )),
        },
        ("reset", _) => Output::ok().with(Effect::ResetTheme),
        ("show" | "ver", _) => Output::msg(show_lines()),
        ("save" | "salvar", _) => match salvar_config() {
            Ok(p) => Output::msg(vec![tr1(
                "✔ saved to {}",
                "✔ salvo em {}",
                &p,
            )]),
            Err(e) => Output::bad(&e),
        },
        ("hosts", ["add", endpoint @ ..]) => {
            let host = endpoint.join(" ");
            if host.trim().is_empty() {
                return Output::bad("usage: /hosts add <endpoint>");
            }
            if get_docker_hosts().contains(&host) {
                return Output::bad("host already exists");
            }
            Output::msg(vec![tr1(
                "✔ docker host {} added and selected",
                "✔ docker host {} adicionado e selecionado",
                &host,
            )])
            .with(Effect::AddHost(host))
        }
        ("hosts", ["rm" | "remove" | "del", endpoint @ ..]) => {
            let host = endpoint.join(" ");
            if host.trim().is_empty() {
                return Output::bad("usage: /hosts rm <endpoint>");
            }
            if !get_docker_hosts().contains(&host) {
                return Output::bad(&tr(
                    "host not found — add it first",
                    "host não encontrado — adicione primeiro",
                ));
            }
            Output::msg(vec![tr1(
                "✔ docker host {} removed",
                "✔ docker host {} removido",
                &host,
            )])
            .with(Effect::RemoveHost(host))
        }
        ("hosts", ["set", endpoint @ ..]) => {
            let host = endpoint.join(" ");
            if host.trim().is_empty() {
                return Output::bad("usage: /hosts set <endpoint>");
            }
            if !get_docker_hosts().contains(&host) {
                return Output::bad(&tr(
                    "host not found — add it first",
                    "host não encontrado — adicione primeiro",
                ));
            }
            Output::msg(vec![tr1(
                "✔ docker host set to {}",
                "✔ docker host definido para {}",
                &host,
            )])
            .with(Effect::ActivateHost(host))
        }
        ("hosts", _) => Output::msg(hosts_lines()),
        _ => Output::bad(&tr(
            "unknown command — type /help",
            "comando desconhecido — digite /ajuda",
        )),
    }
}

// ---- apply ----

/// aplica um efeito no estado global — o único lugar que muta
pub fn apply(eff: Effect) {
    match eff {
        Effect::Color(slot, c) => mudar_tema(move |t| set_slot(t, slot, c)),
        Effect::Border(b) => mudar_tema(move |t| t.border = b),
        Effect::Gauge(g) => mudar_tema(move |t| t.gauge = g),
        Effect::Theme(f) => mudar_tema(|t| *t = f()),
        Effect::ResetTheme => mudar_tema(|t| *t = Theme::pixel()),
        Effect::Lang(l) => set_lang(l),
        Effect::AddHost(host) => {
            let mut hosts = get_docker_hosts();
            if !hosts.contains(&host) {
                hosts.push(host.clone());
                set_docker_hosts(hosts);
            }
            set_active_docker_host(Some(host));
        }
        Effect::RemoveHost(host) => {
            let mut hosts = get_docker_hosts();
            hosts.retain(|x| *x != host);
            set_docker_hosts(hosts);
            if get_active_docker_host() == Some(host) {
                set_active_docker_host(None);
            }
        }
        Effect::ActivateHost(host) => set_active_docker_host(Some(host)),
    }
}

// ---- helpers de mapeamento (puros, testáveis) ----

fn color_slot(nome: &str) -> Option<ColorSlot> {
    Some(match nome {
        "cyan" | "ciano" => ColorSlot::Cy,
        "pink" | "rosa" => ColorSlot::Mag,
        "green" | "verde" => ColorSlot::Grn,
        "yellow" | "amarelo" => ColorSlot::Yel,
        "red" | "vermelho" => ColorSlot::Red,
        "gray" | "cinza" => ColorSlot::Gray,
        "blue" | "azul" => ColorSlot::Blu,
        "peach" | "pessego" => ColorSlot::Lblu,
        "orange" | "laranja" => ColorSlot::Lred,
        "white" | "branco" => ColorSlot::Wht,
        "black" | "preto" => ColorSlot::Blk,
        _ => return None,
    })
}

fn set_slot(t: &mut Theme, slot: ColorSlot, c: Color) {
    match slot {
        ColorSlot::Bg => t.bg = c,
        ColorSlot::Cy => t.cy = c,
        ColorSlot::Mag => t.mag = c,
        ColorSlot::Grn => t.grn = c,
        ColorSlot::Yel => t.yel = c,
        ColorSlot::Red => t.red = c,
        ColorSlot::Gray => t.gray = c,
        ColorSlot::Blu => t.blu = c,
        ColorSlot::Lblu => t.lblu = c,
        ColorSlot::Lred => t.lred = c,
        ColorSlot::Wht => t.wht = c,
        ColorSlot::Blk => t.blk = c,
    }
}

fn border_kind(k: &str) -> Option<BorderKind> {
    Some(match k {
        "pixel" => BorderKind::Pixel,
        "line" | "linha" => BorderKind::Linha,
        "double" | "dupla" => BorderKind::Dupla,
        "thick" | "grossa" => BorderKind::Grossa,
        _ => return None,
    })
}

fn gauge_kind(k: &str) -> Option<GaugeKind> {
    Some(match k {
        "square" | "quadrado" => GaugeKind::Quadrado,
        "circle" | "circulo" => GaugeKind::Circulo,
        _ => return None,
    })
}

// ---- linhas de resposta (help, show, hosts) ----

fn help_lines() -> Vec<String> {
    // gerado do catálogo — nunca dessincroniza das sugestões
    let mut v: Vec<String> = SLASH_CMDS
        .iter()
        .map(|(c, d)| format!("  {c} — {d}"))
        .collect();
    v.push(
        "  aliases: tema, fundo, cor, borda, medidor, ver, salvar, ajuda, h".to_string(),
    );
    v
}

fn show_lines() -> Vec<String> {
    let x = t();
    vec![
        format!(
            "{} {} · {} {:?} · {} {:?}",
            tr("bg", "fundo"),
            hex_of(x.bg),
            tr("border", "borda"),
            x.border,
            tr("gauge", "medidor"),
            x.gauge
        ),
        format!(
            "{} {} {} {} {} {}",
            tr("cyan", "ciano"),
            hex_of(x.cy),
            tr("pink", "rosa"),
            hex_of(x.mag),
            tr("green", "verde"),
            hex_of(x.grn)
        ),
        format!(
            "{} {} {} {} {} {}",
            tr("yellow", "amarelo"),
            hex_of(x.yel),
            tr("red", "vermelho"),
            hex_of(x.red),
            tr("gray", "cinza"),
            hex_of(x.gray)
        ),
        format!(
            "{} {} {} {} {} {}",
            tr("blue", "azul"),
            hex_of(x.blu),
            tr("peach", "pessego"),
            hex_of(x.lblu),
            tr("orange", "laranja"),
            hex_of(x.lred)
        ),
        format!(
            "{} {} {} {}",
            tr("white", "branco"),
            hex_of(x.wht),
            tr("black", "preto"),
            hex_of(x.blk)
        ),
    ]
}

fn hosts_lines() -> Vec<String> {
    let hosts = get_docker_hosts();
    let active = get_active_docker_host();
    if hosts.is_empty() {
        return vec![tr(
            "no docker hosts configured — use /hosts add <endpoint>",
            "nenhum docker host configurado — use /hosts add <endpoint>",
        )
        .to_string()];
    }
    hosts
        .into_iter()
        .map(|h| {
            if Some(&h) == active.as_ref() {
                format!("● {h} (active)")
            } else {
                format!("○ {h}")
            }
        })
        .collect()
}

// ---- sugestões / did_you_mean ----

pub fn slash_suggestions(prefix: &str) -> Vec<String> {
    let p = prefix.trim().to_lowercase();
    if p.is_empty() {
        return SLASH_CMDS
            .iter()
            .map(|(c, d)| format!("{c} — {d}"))
            .collect();
    }
    SLASH_CMDS
        .iter()
        .filter(|(c, _)| c.starts_with(&p))
        .map(|(c, d)| format!("{c} — {d}"))
        .collect()
}

/// nomes únicos de comando (ordem do catálogo) cujo nome começa com o
/// primeiro token de `query` — base pra Tab complete/ciclar
pub fn tab_matches(query: &str) -> Vec<String> {
    let head = match query.trim_start().split_whitespace().next() {
        Some(h) => h.trim_start_matches('/').to_lowercase(),
        None => return vec![],
    };
    let mut names: Vec<String> = Vec::new();
    for (c, _) in SLASH_CMDS {
        if let Some(n) = c.trim_start_matches('/').split_whitespace().next() {
            if n.starts_with(&head) && !names.iter().any(|x| x == n) {
                names.push(n.to_string());
            }
        }
    }
    names
}

pub fn did_you_mean(cmd: &str) -> Option<String> {
    let c = cmd.trim().to_lowercase();
    let c = c.strip_prefix('/').unwrap_or(&c);
    // compara só a primeira palavra contra os NOMES (não as usage strings)
    let word = c.split_whitespace().next().unwrap_or("");
    if word.is_empty() {
        return None;
    }
    let mut names: Vec<&str> = Vec::new();
    for (s, _) in SLASH_CMDS {
        if let Some(n) = s.trim_start_matches('/').split_whitespace().next() {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names
        .into_iter()
        .min_by_key(|s| levenshtein(word, s))
        .filter(|s| levenshtein(word, s) <= 3 && *s != word)
        .map(|s| format!("did you mean /{s}?"))
}

#[allow(clippy::needless_range_loop)]
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        dp[i][0] = i;
    }
    for j in 0..=b.len() {
        dp[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

// ---- config.json (migra pra módulo próprio na fase 2) ----

pub fn config_arquivo() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(std::path::Path::new(&home).join(".config/ratscope/config.json"))
}

pub fn salvar_config() -> Result<String, String> {
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
        "lang": match lang() {
            Lang::Pt => "pt",
            Lang::En => "en",
        },
    });
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(&j).unwrap()).map_err(|e| e.to_string())?;
    Ok(p.display().to_string())
}

// tema salvo sobrepõe o padrão no boot; erros são ignorados de propósito
pub fn carregar_config() {
    let Some(p) = config_arquivo() else { return };
    let Ok(txt) = std::fs::read_to_string(p) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else {
        return;
    };
    let cor = |k: &str| v[k].as_str().and_then(parse_hex);
    mudar_tema(|t| {
        if let Some(c) = cor("bg") {
            t.bg = c;
        }
        if let Some(c) = cor("cy") {
            t.cy = c;
        }
        if let Some(c) = cor("mag") {
            t.mag = c;
        }
        if let Some(c) = cor("grn") {
            t.grn = c;
        }
        if let Some(c) = cor("yel") {
            t.yel = c;
        }
        if let Some(c) = cor("red") {
            t.red = c;
        }
        if let Some(c) = cor("gray") {
            t.gray = c;
        }
        if let Some(c) = cor("blu") {
            t.blu = c;
        }
        if let Some(c) = cor("lblu") {
            t.lblu = c;
        }
        if let Some(c) = cor("lred") {
            t.lred = c;
        }
        if let Some(c) = cor("wht") {
            t.wht = c;
        }
        if let Some(c) = cor("blk") {
            t.blk = c;
        }
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
    if v["lang"].as_str() == Some("pt") {
        set_lang(Lang::Pt);
    }
}

// ---- testes: exec é puro, roda sem UI e sem mutar estado global ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_silent() {
        assert!(exec("").lines.is_empty());
        assert!(exec("   ").lines.is_empty());
        assert!(exec("").effects.is_empty());
    }

    #[test]
    fn unknown_command_errors() {
        assert!(exec("nope").lines_resolved()[0].starts_with("✖"));
        assert!(exec("nope").effects.is_empty());
    }

    #[test]
    fn theme_produces_effect() {
        let o = exec("theme gruvbox");
        assert!(o.lines_resolved()[0].starts_with("✔"));
        assert!(matches!(o.effects[0], Effect::Theme(_)));
        // slash prefix é opcional
        let o = exec("/tema nord");
        assert!(matches!(o.effects[0], Effect::Theme(_)));
    }

    #[test]
    fn invalid_theme_is_rejected() {
        let o = exec("theme nope");
        assert!(o.lines_resolved()[0].starts_with("✖"));
        assert!(o.effects.is_empty());
    }

    #[test]
    fn invalid_hex_is_rejected() {
        assert!(exec("bg zz").lines_resolved()[0].starts_with("✖"));
        assert!(exec("color cyan zz").lines_resolved()[0].starts_with("✖"));
        assert!(exec("color nope #10101A").lines_resolved()[0].starts_with("✖"));
    }

    #[test]
    fn lang_effect() {
        assert!(matches!(exec("lang pt").effects[0], Effect::Lang(Lang::Pt)));
        assert!(matches!(exec("lang en").effects[0], Effect::Lang(Lang::En)));
        assert!(exec("lang xx").lines_resolved()[0].starts_with("✖"));
    }

    #[test]
    fn color_and_border_and_gauge_effects() {
        assert!(matches!(
            exec("color cyan #10101A").effects[0],
            Effect::Color(ColorSlot::Cy, _)
        ));
        assert!(matches!(
            exec("border thick").effects[0],
            Effect::Border(BorderKind::Grossa)
        ));
        assert!(matches!(
            exec("gauge circle").effects[0],
            Effect::Gauge(GaugeKind::Circulo)
        ));
        // alias em português
        assert!(matches!(
            exec("borda grossa").effects[0],
            Effect::Border(BorderKind::Grossa)
        ));
    }

    #[test]
    fn reset_produces_effect() {
        assert!(matches!(exec("reset").effects[0], Effect::ResetTheme));
    }

    #[test]
    fn host_validation_is_read_only() {
        // hosts rm sem host → erro, sem efeito
        assert!(exec("hosts rm").lines_resolved()[0].starts_with("✖"));
        assert!(exec("hosts rm").effects.is_empty());
    }

    #[test]
    fn help_and_show_have_no_effects() {
        assert!(exec("help").effects.is_empty());
        assert!(!exec("help").lines.is_empty());
        assert!(exec("show").effects.is_empty());
    }

    #[test]
    fn help_lists_every_command() {
        let h = help_lines().join("\n");
        for (c, _) in SLASH_CMDS {
            assert!(h.contains(c), "help não lista {c}");
        }
        assert!(h.contains("aliases"));
    }

    #[test]
    fn did_you_mean_suggests() {
        // compara contra nomes: "them" → theme (não /help)
        assert_eq!(
            did_you_mean("them").as_deref(),
            Some("did you mean /theme?")
        );
        assert!(did_you_mean("host add x").is_some()); // 1ª palavra → hosts
        assert!(did_you_mean("help").is_none()); // comando exato não sugere
        assert!(did_you_mean("theme nope").is_none()); // erro de uso não sugere
    }

    #[test]
    fn tab_matches_complete_and_cycle_base() {
        assert_eq!(tab_matches("th"), vec!["theme".to_string()]);
        // "s" casa show e save, na ordem do catálogo
        assert_eq!(tab_matches("s"), vec!["show".to_string(), "save".to_string()]);
        assert_eq!(tab_matches("zzz"), Vec::<String>::new());
        assert_eq!(tab_matches("theme gruvbox"), vec!["theme".to_string()]);
    }

    #[test]
    fn parse_strips_slash_and_lowercases() {
        assert_eq!(
            parse("/THEME gruvbox"),
            Some(("theme".into(), vec!["gruvbox".into()]))
        );
        assert_eq!(parse(""), None);
    }
}
