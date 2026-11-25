use openwoot_protocol::{
    apps::{App, Filter, Link, Matching},
    profile::Slot,
};

use super::{Foreground, SteamApps};
use crate::store::Linked;

const EXECUTABLE_SUFFIXES: [&str; 2] = [".exe", ".app"];

pub(crate) fn linked_target(
    linked: &[Linked],
    foreground: &Foreground,
    steam: &SteamApps,
) -> Option<Slot> {
    (0_u8..)
        .zip(linked)
        .find(|(_, profile)| {
            profile
                .links
                .iter()
                .any(|link| matches(link, foreground, steam))
        })
        .map(|(index, _)| Slot::linked(index))
}

pub(crate) fn uses_title(link: &Link) -> bool {
    match &link.app {
        App::Custom(custom) => custom
            .filters
            .iter()
            .any(|filter| matches!(filter, Filter::WindowTitle(_))),
        App::Steam(_) => false,
    }
}

pub(crate) fn steam_ids(linked: &[Linked]) -> impl Iterator<Item = u32> + '_ {
    linked
        .iter()
        .flat_map(|profile| &profile.links)
        .filter_map(|link| match link.app {
            App::Steam(id) => Some(id),
            App::Custom(_) => None,
        })
}

pub(crate) fn identity(link: &Link) -> String {
    match &link.app {
        App::Steam(id) => format!("steam:{id}"),
        App::Custom(custom) => {
            let mut filters: Vec<String> = custom
                .filters
                .iter()
                .map(|filter| match filter {
                    Filter::ProcessName(name) => format!("name({})", name.trim().to_lowercase()),
                    Filter::ProcessDirectory(directory) => {
                        format!("directory({})", normalized(directory))
                    }
                    Filter::WindowTitle(title) => format!("title({})", title.trim()),
                    Filter::ProcessPath(path) => format!("path({})", normalized(path)),
                })
                .collect();
            filters.sort();
            let matching = match custom.matching {
                Matching::Any => "any",
                Matching::All => "all",
            };
            format!("custom:{matching}:{}", filters.join("|"))
        }
    }
}

fn matches(link: &Link, foreground: &Foreground, steam: &SteamApps) -> bool {
    match &link.app {
        App::Custom(custom) if custom.filters.is_empty() => false,
        App::Custom(custom) => {
            let mut results = custom
                .filters
                .iter()
                .map(|filter| filter_matches(filter, foreground));
            match custom.matching {
                Matching::Any => results.any(|matched| matched),
                Matching::All => results.all(|matched| matched),
            }
        }
        App::Steam(id) => steam.directory(*id).is_some_and(|directory| {
            foreground
                .paths
                .iter()
                .any(|path| within(&path.to_string_lossy(), &directory.to_string_lossy()))
        }),
    }
}

fn filter_matches(filter: &Filter, foreground: &Foreground) -> bool {
    let paths = || foreground.paths.iter().map(|path| path.to_string_lossy());
    match filter {
        Filter::ProcessName(name) => paths().any(|path| same_name(file_name(&path), name)),
        Filter::ProcessDirectory(directory) => paths().any(|path| within(&path, directory)),
        Filter::ProcessPath(wanted) => {
            let wanted = normalized(wanted);
            !wanted.is_empty() && paths().any(|path| normalized(&path) == wanted)
        }
        Filter::WindowTitle(text) => {
            let text = text.trim().to_lowercase();
            !text.is_empty() && foreground.title.to_lowercase().contains(&text)
        }
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn same_name(file: &str, wanted: &str) -> bool {
    let file = file.to_lowercase();
    let wanted = wanted.trim().to_lowercase();
    !wanted.is_empty() && (file == wanted || bare(&file) == bare(&wanted))
}

fn bare(name: &str) -> &str {
    EXECUTABLE_SUFFIXES
        .into_iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(name)
}

fn within(path: &str, directory: &str) -> bool {
    let directory = normalized(directory);
    !directory.is_empty()
        && normalized(path)
            .strip_prefix(&directory)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

fn normalized(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use openwoot_protocol::{apps, heapless, lighting::Rgb, profile};

    use super::*;

    fn text(value: &str) -> apps::Text {
        apps::Text::try_from(value).unwrap_or_default()
    }

    fn custom(matching: Matching, filters: &[Filter]) -> Link {
        Link {
            name: text("Game"),
            app: App::Custom(apps::Custom {
                matching,
                filters: filters.iter().cloned().collect(),
            }),
        }
    }

    fn linked(name: &str, links: Vec<Link>) -> Linked {
        Linked {
            info: profile::Info {
                name: heapless::String::try_from(name).unwrap_or_default(),
                color: Rgb {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
                icon: 0,
            },
            links,
        }
    }

    fn foreground(paths: &[&str], title: &str) -> Foreground {
        Foreground {
            process: 7,
            name: "Game".to_owned(),
            paths: paths.iter().map(PathBuf::from).collect(),
            title: title.to_owned(),
        }
    }

    fn valorant() -> Foreground {
        foreground(&[r"C:\Riot Games\VALORANT\live\VALORANT.exe"], "VALORANT  ")
    }

    #[test]
    fn a_process_name_matches_regardless_of_case_and_executable_suffix() {
        let steam = SteamApps::default();
        for name in ["VALORANT.exe", "valorant.exe", "Valorant"] {
            let link = custom(Matching::Any, &[Filter::ProcessName(text(name))]);
            assert!(matches(&link, &valorant(), &steam), "{name}");
        }
        let other = custom(Matching::Any, &[Filter::ProcessName(text("valorant2.exe"))]);
        assert!(!matches(&other, &valorant(), &steam));
    }

    #[test]
    fn a_directory_holds_every_process_below_it_but_not_its_namesakes() {
        let steam = SteamApps::default();
        let inside = custom(
            Matching::Any,
            &[Filter::ProcessDirectory(text("c:/riot games/"))],
        );
        let sibling = custom(Matching::Any, &[Filter::ProcessDirectory(text(r"C:\Riot"))]);
        assert!(matches(&inside, &valorant(), &steam));
        assert!(!matches(&sibling, &valorant(), &steam));
    }

    #[test]
    fn a_full_path_and_a_window_title_match_like_stock() {
        let steam = SteamApps::default();
        let path = custom(
            Matching::Any,
            &[Filter::ProcessPath(text(
                "c:/riot games/valorant/live/valorant.exe",
            ))],
        );
        let title = custom(Matching::Any, &[Filter::WindowTitle(text("valor"))]);
        let empty_title = custom(Matching::Any, &[Filter::WindowTitle(text("  "))]);
        assert!(matches(&path, &valorant(), &steam));
        assert!(matches(&title, &valorant(), &steam));
        assert!(!matches(&empty_title, &valorant(), &steam));
    }

    #[test]
    fn all_needs_every_filter_and_any_needs_one() {
        let steam = SteamApps::default();
        let filters = [
            Filter::ProcessName(text("VALORANT.exe")),
            Filter::WindowTitle(text("Minecraft")),
        ];
        assert!(matches(
            &custom(Matching::Any, &filters),
            &valorant(),
            &steam
        ));
        assert!(!matches(
            &custom(Matching::All, &filters),
            &valorant(),
            &steam
        ));
        assert!(!matches(&custom(Matching::All, &[]), &valorant(), &steam));
    }

    #[test]
    fn a_mac_app_matches_by_its_executable_or_its_bundle() {
        let steam = SteamApps::default();
        let safari = foreground(
            &[
                "/Applications/Safari.app/Contents/MacOS/Safari",
                "/Applications/Safari.app",
            ],
            "",
        );
        for name in ["Safari", "Safari.app"] {
            let link = custom(Matching::Any, &[Filter::ProcessName(text(name))]);
            assert!(matches(&link, &safari, &steam), "{name}");
        }
    }

    #[test]
    fn a_steam_game_matches_inside_its_install_directory() {
        let steam = SteamApps::from_directories([(
            730,
            PathBuf::from("/games/steamapps/common/Counter-Strike Global Offensive"),
        )]);
        let game = Link {
            name: text("Counter-Strike 2"),
            app: App::Steam(730),
        };
        let unknown = Link {
            name: text("Unknown"),
            app: App::Steam(440),
        };
        let inside = foreground(
            &["/games/steamapps/common/Counter-Strike Global Offensive/game/bin/cs2"],
            "",
        );
        assert!(matches(&game, &inside, &steam));
        assert!(!matches(&unknown, &inside, &steam));
        assert!(!matches(&game, &valorant(), &steam));
    }

    #[test]
    fn the_first_linked_profile_with_a_matching_app_is_the_target() {
        let steam = SteamApps::default();
        let profiles = [
            linked(
                "Browsing",
                vec![custom(
                    Matching::Any,
                    &[Filter::ProcessName(text("Safari"))],
                )],
            ),
            linked(
                "Shooter",
                vec![custom(
                    Matching::Any,
                    &[Filter::ProcessName(text("VALORANT.exe"))],
                )],
            ),
            linked(
                "Riot",
                vec![custom(
                    Matching::Any,
                    &[Filter::ProcessDirectory(text("C:/Riot Games"))],
                )],
            ),
        ];
        assert_eq!(
            linked_target(&profiles, &valorant(), &steam),
            Some(Slot::linked(1))
        );
        assert_eq!(
            linked_target(&profiles, &foreground(&["/usr/bin/vim"], "vim"), &steam),
            None
        );
    }
}
