use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use fuzzy_matcher::FuzzyMatcher;
use icon::get_icon;
use serde_json;
use serde::Deserialize;
use std::{fs, process::Command};

mod icon;

struct Context {
    config: Config,
    hyprland_clients: Vec<(HyprlandClient, u64)>
}

impl Context {
    fn empty() -> Context {
        Context {
            config: Config::default(),
            hyprland_clients: vec![]
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
            max_entries: 10,
            score_threshold: 50,
        }
    }
}

#[derive(Deserialize)]
struct HyprlandClient {
    address: String,
    class: String,
    title: String,
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

#[get_matches]
fn get_matches(input: RString, ctx: &mut Context) -> RVec<Match> {
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

    for (entry, _, _) in &mut entries {
        let _ = get_icon(entry.class.as_str());
    }

    entries
        .into_iter()
        .map(|(client, id, _)| Match {
            title: client.title.clone().into(),
            description: ROption::RNone,
            use_pango: false,
            icon: ROption::RSome(RString::from(get_icon(client.class.as_str()))),
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
