use {
    crate::config::Config,
    serde::Serialize,
    solana_cli_output::{OutputFormat, QuietDisplay, VerboseDisplay},
    std::fmt::Display,
};

pub fn parse_output_format(output_format: &str) -> OutputFormat {
    match output_format {
        "display" => OutputFormat::Display,
        "json" => OutputFormat::Json,
        "json-compact" => OutputFormat::JsonCompact,
        "quiet" => OutputFormat::DisplayQuiet,
        "verbose" => OutputFormat::DisplayVerbose,
        _ => unreachable!(),
    }
}

pub fn println_display(config: &Config, message: String) {
    match config.output_format {
        OutputFormat::Display | OutputFormat::DisplayVerbose => {
            println!("{}", message);
        }
        _ => {}
    }
}

pub fn format_output<T>(config: &Config, command_output: T) -> String
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    config.output_format.formatted_string(&command_output)
}
