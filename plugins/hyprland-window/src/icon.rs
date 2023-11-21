use std::{collections::HashMap, env, ffi::OsStr, fs, path::Path, path::PathBuf};

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    pub exec: String,
    pub path: Option<PathBuf>,
    pub name: String,
    pub keywords: Vec<String>,
    pub desc: Option<String>,
    pub icon: String,
    pub term: bool,
    pub offset: i64,
}

const FIELD_CODE_LIST: &[&str] = &[
    "%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m",
];

impl DesktopEntry {
    fn from_dir_entry(entry: &Path) -> Vec<Self> {
        if entry.extension() == Some(OsStr::new("desktop")) {
            let content = match fs::read_to_string(entry) {
                Ok(content) => content,
                Err(_) => return Vec::new(),
            };

            let lines = content.lines().collect::<Vec<_>>();

            let sections = lines
                .split_inclusive(|line| line.starts_with('['))
                .collect::<Vec<_>>();

            let mut line = None;
            let mut new_sections = Vec::new();

            for (i, section) in sections.iter().enumerate() {
                if let Some(line) = line {
                    let mut section = section.to_vec();
                    section.insert(0, line);

                    // Only pop the last redundant entry if it isn't the last item
                    if i < sections.len() - 1 {
                        section.pop();
                    }
                    new_sections.push(section);
                }
                line = Some(section.last().unwrap_or(&""));
            }

            let mut ret = Vec::new();

            let entry = match new_sections.iter().find_map(|section| {
                if section[0].starts_with("[Desktop Entry]") {
                    let mut map = HashMap::new();

                    for line in section.iter().skip(1) {
                        if let Some((key, val)) = line.split_once('=') {
                            map.insert(key, val);
                        }
                    }

                    if map.get("Type")? == &"Application"
                        && match map.get("NoDisplay") {
                            Some(no_display) => !no_display.parse::<bool>().unwrap_or(true),
                            None => true,
                        }
                    {
                        Some(DesktopEntry {
                            exec: {
                                let mut exec = map.get("Exec")?.to_string();

                                for field_code in FIELD_CODE_LIST {
                                    exec = exec.replace(field_code, "");
                                }
                                exec
                            },
                            path: map.get("Path").map(PathBuf::from),
                            name: map.get("Name")?.to_string(),
                            keywords: map
                                .get("Keywords")
                                .map(|keywords| {
                                    keywords
                                        .split(';')
                                        .map(|s| s.to_owned())
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default(),
                            desc: None,
                            icon: map
                                .get("Icon")
                                .unwrap_or(&"application-x-executable")
                                .to_string(),
                            term: map
                                .get("Terminal")
                                .map(|val| val.to_lowercase() == "true")
                                .unwrap_or(false),
                            offset: 0,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                Some(entry) => entry,
                None => return Vec::new(),
            };

            ret.push(entry);
            ret
        } else {
            Vec::new()
        }
    }
}

pub fn get_icon<'a>(desktop_entry_identifier: &str) -> String {
    let icon_fallback: String = "application-x-executable".into();

    // Create iterator over all the files in the XDG_DATA_DIRS
    // XDG compliancy is cool
    let user_path = match env::var("XDG_DATA_HOME") {
        Ok(data_home) => {
            format!("{}/applications/", data_home)
        }
        Err(_) => {
            format!(
                "{}/.local/share/applications/",
                env::var("HOME").expect("Unable to determine home directory!")
            )
        }
    };

    let mut entries: Vec<String> = vec![user_path];

    entries.extend(match env::var("XDG_DATA_DIRS") {
        Ok(data_dirs) => {
            // The vec for all the DirEntry objects
            let mut paths: Vec<String> = Vec::new();
            // Parse the XDG_DATA_DIRS variable and list files of all the paths
            for dir in data_dirs.split(':') {
                paths.push(dir.to_string());
            }

            // Make sure the list of paths isn't empty
            if paths.is_empty() {
                return icon_fallback;
            }

            // Return it
            paths
        }
        Err(_) => vec![String::from("/usr/share/applications")],
    });

    for entry in &entries {
        let str_path = format!("{}/{}.desktop", entry, desktop_entry_identifier.to_lowercase());
        let expected_path = Path::new(str_path.as_str());

        if expected_path.exists() {
            let desktop_entry = DesktopEntry::from_dir_entry(expected_path);

            if desktop_entry.len() > 0 {
                return desktop_entry
                    .first()
                    .map(|entry| entry.icon.clone())
                    .unwrap_or(icon_fallback);
            } else {
                return icon_fallback;
            }
        }
    }

    icon_fallback
}
