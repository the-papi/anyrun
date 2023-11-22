use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use fuzzy_matcher::FuzzyMatcher;
use scrubber::DesktopEntry;
use serde_json;
use serde::Deserialize;
use std::{fs, process::Command};

mod scrubber;

struct Context {
    config: Config,
    hyprland_clients: Vec<(HyprlandClient, u64)>,
    desktop_entries: Vec<(DesktopEntry, u64)>,
}

impl Context {
    fn empty() -> Context {
        Context {
            config: Config::default(),
            hyprland_clients: vec![],
            desktop_entries: vec![],
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    max_entries: usize,
    score_threshold: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_entries: 3,
            score_threshold: 50,
        }
    }
}

#[derive(Deserialize)]
struct HyprlandClient {
    address: String,
    class: String,
    title: String,
    pid: i32,
}

fn load_config(config_dir: RString) -> Config {
    match fs::read_to_string(format!("{}/hyprland-window.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_else(|why| {
            eprintln!("Error parsing applications plugin config: {}", why);
            Config::default()
        }),
        Err(why) => {
            eprintln!("Error reading applications plugin config: {}", why);
            Config::default()
        }
    }
}

fn parse_json_data(json_str: &str) -> Vec<HyprlandClient> {
    let parsed_data: Result<Vec<HyprlandClient>, serde_json::Error> = serde_json::from_str(json_str);
    match parsed_data {
        Ok(data) => data,
        Err(_) => vec![],
    }
}

#[init]
fn init(config_dir: RString) -> Context {
    let config = load_config(config_dir);
    let desktop_entries = scrubber::scrubber().unwrap_or_else(|why| {
        eprintln!("Failed to load desktop entries: {}", why);
        Vec::new()
    });
    let output = Command::new("hyprctl").arg("clients").arg("-j").output();
    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8(output.stdout);
                match stdout {
                    Ok(json_str) => Context {
                        config: config,
                        hyprland_clients: parse_json_data(&json_str)
                            .into_iter()
                            .enumerate()
                            .map(|(i, entry)| (entry, i as u64))
                            .collect(),
                        desktop_entries: desktop_entries,
                    },
                    Err(_) => Context::empty(),
                }
            } else {
                Context::empty()
            }
        },
        Err(_) => Context::empty(),
    }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Hyprland window".into(),
        icon: "focus-windows-symbolic".into(),
    }
}

fn find_icon(class: &String, pid: u32, ctx: &Context) -> String {
    // Try to find icon by class == startup_wm_class
    for (entry, _) in &ctx.desktop_entries {
        match &entry.startup_wm_class {
            Some(startup_wm_class) => {
                if startup_wm_class == class {
                    return entry.icon.clone();
                }
            }
            _ => {}
        };
    }

    // Try to find icon by class in name field
    for (entry, _) in &ctx.desktop_entries {
        if entry.name.to_lowercase().contains(&class.to_lowercase()) {
            return entry.icon.clone();
        }
    }

    // Try to find icon by process name
    let process = psutil::process::Process::new(pid).unwrap();
    let process_name = process.name().unwrap_or("".into());
    for (entry, _) in &ctx.desktop_entries {
        if entry.exec.contains(&process_name) {
            return entry.icon.clone();
        }
    }

    class.to_string() // return class as icon name if no icon found -> it may also be a valid icon name
}

#[get_matches]
fn get_matches(input: RString, ctx: &Context) -> RVec<Match> {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();

    let mut entries = ctx
        .hyprland_clients
        .iter()
        .filter_map(|(entry, id)| {
            if entry.title.trim().is_empty() {
                return None;
            }

            let class_score = matcher
                .fuzzy_match(&entry.class, &input)
                .unwrap_or(0);

            let title_score = matcher
                .fuzzy_match(&entry.title, &input)
                .unwrap_or(0);

            let score = (class_score * 10) + title_score; // class score has more weight over title score

            if score > ctx.config.score_threshold {
                Some((entry, *id, score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| b.2.cmp(&a.2));
    entries.truncate(ctx.config.max_entries);

    entries
        .into_iter()
        .map(|(client, id, _)| Match {
            title: client.title.clone().into(),
            description: ROption::RNone,
            use_pango: false,
            icon: ROption::RSome(RString::from(find_icon(&client.class, client.pid as u32, ctx))),
            id: ROption::RSome(id),
        })
        .collect()
}

#[handler]
pub fn handler(selection: Match, ctx: &Context) -> HandleResult {
    let entry = ctx
        .hyprland_clients
        .iter()
        .find_map(|(entry, id)| {
            if *id == selection.id.unwrap() {
                Some(entry)
            } else {
                None
            }
        })
        .unwrap();

    let _ = Command::new("hyprctl")
        .arg("dispatch")
        .arg("focuswindow")
        .arg(format!("address:{}", entry.address))
        .spawn();

    HandleResult::Close
}
