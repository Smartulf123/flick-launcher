//! Windows: the programs in Start's "All apps" list.

/// What the shell opens to start the app with this `AppsFolder` parsing name.
pub fn shell_target(parsing_name: &str) -> String {
    format!("shell:AppsFolder\\{parsing_name}")
}

/// Starts the app, desktop or Store, without waiting for it.
pub fn launch(parsing_name: &str) -> std::io::Result<()> {
    open::that_detached(shell_target(parsing_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_start_through_the_apps_folder() {
        assert_eq!(
            shell_target("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
            r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
    }
}
