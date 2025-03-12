use {
    crate::{
        config::Config,
        create_mint::{command_create_mint, CreateMintArgs},
        CommandResult,
    },
    clap::{
        builder::{PossibleValuesParser, TypedValueParser},
        ArgMatches, Args, Parser, Subcommand,
    },
    solana_clap_v3_utils::input_parsers::{
        parse_url_or_moniker,
        signer::{SignerSource, SignerSourceParserBuilder},
    },
    solana_cli_output::OutputFormat,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    std::rc::Rc,
};

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(flatten)]
    pub config: CliConfig,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Args, Clone, Debug)]
#[clap(about = "A command line tool for interacting with the SPL Token Wrap program")]
pub struct CliConfig {
    /// URL for Solana JSON RPC or moniker (or their first letter):
    /// [mainnet-beta, testnet, devnet, localhost].
    /// Default from the configuration file.
    #[clap(
        global(true),
        short = 'u',
        long = "url",
        id = "URL_OR_MONIKER",
        value_parser = parse_url_or_moniker,
    )]
    pub json_rpc_url: Option<String>,

    /// Specify the fee-payer account. This may be a keypair file, the ASK
    /// keyword or the pubkey of an offline signer, provided an appropriate
    /// --signer argument is also passed. Defaults to the client keypair.
    #[clap(
        global(true),
        long,
        id = "PAYER_KEYPAIR",
        value_parser = SignerSourceParserBuilder::default().allow_all().build(),
    )]
    pub fee_payer: Option<SignerSource>,

    /// Run in verbose mode
    #[clap(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Simulate transaction instead of executing
    #[clap(long = "dry-run", global = true)]
    pub dry_run: bool,
    /// Path to the configuration file
    #[clap(short = 'C', long = "config", global = true, value_name = "PATH")]
    pub config_file: Option<String>,

    /// Return information in specified output format
    #[clap(
        global(true),
        long = "output",
        id = "FORMAT",
        conflicts_with = "verbose",
        value_parser = PossibleValuesParser::new(["json", "json-compact"]).map(|o| parse_output_format(&o)),
    )]
    pub output_format: Option<OutputFormat>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Create a wrapped mint for a given SPL Token
    CreateMint(CreateMintArgs),
    // TODO: Wrap, Unwrap
}

impl Command {
    pub async fn execute(
        self,
        config: &Config,
        matches: &ArgMatches,
        wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
    ) -> CommandResult {
        match self {
            Command::CreateMint(args) => {
                command_create_mint(config, args, matches, wallet_manager).await
            }
        }
    }
}

pub fn parse_output_format(output_format: &str) -> OutputFormat {
    match output_format {
        "json" => OutputFormat::Json,
        "json-compact" => OutputFormat::JsonCompact,
        _ => unreachable!(),
    }
}
