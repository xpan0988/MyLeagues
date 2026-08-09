use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::db::Database;
use crate::db::repositories::settings::SettingsRepository;
use crate::dto::analytics::ClientStateDto;
use crate::error::{AppError, AppResult};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct LauncherService<'state> {
    database: &'state Database,
}

impl<'state> LauncherService<'state> {
    pub fn new(database: &'state Database) -> Self {
        Self { database }
    }

    pub fn state(&self) -> AppResult<ClientStateDto> {
        let configured = self.resolve_executable()?;
        let processes = running_processes()?;
        Ok(state_from_processes(&processes, configured.is_some()))
    }

    pub fn launch(&self) -> AppResult<ClientStateDto> {
        let before = self.state()?;
        if before.league_client_running || before.riot_client_running {
            return Ok(before);
        }
        let executable = self.resolve_executable()?.ok_or_else(|| AppError::Configuration(
            "Riot Client executable was not found; configure RiotClientServices.exe in Settings".to_owned(),
        ))?;
        tracing::info!(target: "launcher", executable = %executable.display(), "launching official Riot Client");
        let mut command = Command::new(&executable);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        command.spawn().map_err(|error| {
            AppError::Unavailable(format!("Riot Client launch failed: {error}"))
        })?;
        Ok(ClientStateDto {
            riot_client_running: true,
            league_client_running: false,
            game_running: false,
            configured_executable_found: true,
        })
    }

    fn resolve_executable(&self) -> AppResult<Option<PathBuf>> {
        let connection = self.database.connection()?;
        let settings = SettingsRepository::new(&connection).get()?;
        Ok(find_executable(settings.riot_client_path.as_deref()))
    }
}

fn find_executable(configured: Option<&str>) -> Option<PathBuf> {
    configured
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| common_paths().into_iter().find(|path| path.is_file()))
}

fn common_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(
        r"C:\Riot Games\Riot Client\RiotClientServices.exe",
    )];
    for root in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramData"),
    ]
    .into_iter()
    .flatten()
    {
        paths.push(Path::new(&root).join(r"Riot Games\Riot Client\RiotClientServices.exe"));
    }
    paths
}

fn running_processes() -> AppResult<Vec<String>> {
    let mut command = Command::new("tasklist.exe");
    command.args(["/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output()?;
    if !output.status.success() {
        return Err(AppError::Unavailable(
            "Windows process detection failed".to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            line.strip_prefix('"')?
                .split('"')
                .next()
                .map(|name| name.to_ascii_lowercase())
        })
        .collect())
}

fn state_from_processes(processes: &[String], executable_found: bool) -> ClientStateDto {
    let has = |names: &[&str]| {
        processes
            .iter()
            .any(|process| names.iter().any(|name| process.eq_ignore_ascii_case(name)))
    };
    ClientStateDto {
        riot_client_running: has(&[
            "riotclientservices.exe",
            "riotclientux.exe",
            "riotclientuxrender.exe",
        ]),
        league_client_running: has(&[
            "leagueclient.exe",
            "leagueclientux.exe",
            "leagueclientuxrender.exe",
        ]),
        game_running: has(&["league of legends.exe"]),
        configured_executable_found: executable_found,
    }
}

#[cfg(test)]
mod tests {
    use super::state_from_processes;
    #[test]
    fn detects_riot_league_and_game_processes_case_insensitively() {
        let state = state_from_processes(
            &[
                "riotclientservices.exe".into(),
                "LeagueClientUx.exe".into(),
                "league of legends.exe".into(),
            ],
            true,
        );
        assert!(
            state.riot_client_running
                && state.league_client_running
                && state.game_running
                && state.configured_executable_found
        );
    }
}
