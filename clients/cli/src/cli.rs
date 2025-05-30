use {
    crate::{
        config::Config,
        create_escrow_account::{command_create_escrow_account, CreateEscrowAccountArgs},
        create_mint::{command_create_mint, CreateMintArgs},
        find_pdas::{command_get_pdas, FindPdasArgs},
        output::parse_output_format,
        unwrap::{command_unwrap, UnwrapArgs},
        wrap::{command_wrap, WrapArgs},
        CommandResult,
    },
    clap::{
        builder::{PossibleValuesParser, TypedValueParser},
        ArgMatches, Parser, Subcommand,
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
#[clap(
    author,
    version,
    about = "A command line tool for interacting with the SPL Token Wrap program"
)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,

    /// Configuration file to use
    #[clap(global(true), short = 'C', long = "config", id = "PATH")]
    pub config_file: Option<String>,

    /// Simulate transaction instead of executing
    #[clap(global(true), long, alias = "dryrun")]
    pub dry_run: bool,

    /// URL for Solana JSON `RPC` or moniker (or their first letter):
    /// [`mainnet-beta`, `testnet`, `devnet`, `localhost`].
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

    /// Show additional information
    #[clap(global(true), short, long)]
    pub verbose: bool,

    /// Return information in specified output format
    #[clap(
        global(true),
        long = "output",
        id = "FORMAT",
        conflicts_with = "verbose",
        value_parser = PossibleValuesParser::new([
            "display",
            "json",
            "json-compact",
            "quiet",
            "verbose"
        ]).map(|o| parse_output_format(&o)),
    )]
    pub output_format: Option<OutputFormat>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Create a wrapped mint for a given SPL Token
    CreateMint(CreateMintArgs),
    /// Escrow SPL tokens and mint their wrapped version
    Wrap(WrapArgs),
    /// Find the PDA addresses associated with unwrapped mints
    FindPdas(FindPdasArgs),
    /// Convert wrapped tokens back into their original unwrapped version
    Unwrap(UnwrapArgs),
    /// Create an account used to escrow unwrapped tokens
    CreateEscrowAccount(CreateEscrowAccountArgs),
}

impl Command {
    pub async fn execute(
        self,
        config: &Config,
        matches: &ArgMatches,
        wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
    ) -> CommandResult {
        match self {
            Command::CreateMint(args) => command_create_mint(config, args).await,
            Command::Wrap(args) => command_wrap(config, args, matches, wallet_manager).await,
            Command::FindPdas(args) => command_get_pdas(config, args).await,
            Command::Unwrap(args) => command_unwrap(config, args, matches, wallet_manager).await,
            Command::CreateEscrowAccount(args) => command_create_escrow_account(config, args).await,
        }
    }
}
