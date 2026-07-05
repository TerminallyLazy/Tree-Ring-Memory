#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Rings,
    Search(String),
    Remember(String),
    Forget,
    Redact,
    Promote,
    Scar,
    Seed,
    Supersede(String),
    Consolidate,
    Export,
    Sync,
    Stream,
    Watch,
    Unknown(String),
}

pub fn parse_slash_command(input: &str) -> SlashCommand {
    let command = input.trim().trim_start_matches('/');
    let mut parts = command.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default().to_ascii_lowercase();
    let argument = parts.next().unwrap_or_default().trim().to_string();

    match name.as_str() {
        "rings" => SlashCommand::Rings,
        "search" => SlashCommand::Search(argument),
        "remember" => SlashCommand::Remember(argument),
        "forget" => SlashCommand::Forget,
        "redact" => SlashCommand::Redact,
        "promote" => SlashCommand::Promote,
        "scar" => SlashCommand::Scar,
        "seed" => SlashCommand::Seed,
        "supersede" => SlashCommand::Supersede(argument),
        "consolidate" => SlashCommand::Consolidate,
        "export" => SlashCommand::Export,
        "sync" => SlashCommand::Sync,
        "stream" => SlashCommand::Stream,
        "watch" => SlashCommand::Watch,
        "" => SlashCommand::Unknown(String::new()),
        _ => SlashCommand::Unknown(name),
    }
}

pub fn command_help() -> &'static str {
    "/rings /search <q> /remember <summary> /forget /redact /promote /scar /seed /supersede <old_id> /consolidate /export /sync"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_and_remember_arguments() {
        assert_eq!(
            parse_slash_command("/search stale cache"),
            SlashCommand::Search("stale cache".to_string())
        );
        assert_eq!(
            parse_slash_command("/remember User prefers concise reports"),
            SlashCommand::Remember("User prefers concise reports".to_string())
        );
    }

    #[test]
    fn parses_dangerous_commands_without_executing_them() {
        assert_eq!(parse_slash_command("/forget"), SlashCommand::Forget);
        assert_eq!(
            parse_slash_command("/supersede mem_old"),
            SlashCommand::Supersede("mem_old".to_string())
        );
    }
}
