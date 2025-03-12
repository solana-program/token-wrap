#[derive(Clone, Debug, Copy, Eq, Hash, PartialEq, PartialOrd, Ord, clap::ValueEnum)]
pub enum OutputFormat {
    Display,
    DisplayVerbose,
    Json,
    JsonCompact,
}

impl From<OutputFormat> for solana_cli_output::OutputFormat {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Display => solana_cli_output::OutputFormat::Display,
            OutputFormat::DisplayVerbose => solana_cli_output::OutputFormat::DisplayVerbose,
            OutputFormat::Json => solana_cli_output::OutputFormat::Json,
            OutputFormat::JsonCompact => solana_cli_output::OutputFormat::JsonCompact,
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "json-compact" => Ok(OutputFormat::JsonCompact),
            "display" => Ok(OutputFormat::Display),
            "verbose" => Ok(OutputFormat::DisplayVerbose),
            _ => Err(format!("Invalid output format: {}", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Display => write!(f, "display"),
            OutputFormat::DisplayVerbose => write!(f, "display-verbose"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonCompact => write!(f, "json-compact"),
        }
    }
}
