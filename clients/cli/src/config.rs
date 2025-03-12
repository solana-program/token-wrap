use {
    crate::{cli::Cli, Error},
    clap::ArgMatches,
    solana_clap_v3_utils::keypair::signer_from_source,
    solana_cli_output::OutputFormat,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_sdk::{commitment_config::CommitmentConfig, signature::Signer},
    spl_token_client::client::{ProgramClient, ProgramRpcClient, ProgramRpcClientSendTransaction},
    std::{rc::Rc, sync::Arc},
};

pub struct Config {
    pub rpc_client: Arc<RpcClient>,
    pub program_client: Arc<dyn ProgramClient<ProgramRpcClientSendTransaction>>,
    pub fee_payer: Option<Arc<dyn Signer>>,
    pub output_format: OutputFormat,
    pub dry_run: bool,
}

impl Config {
    pub fn new(
        cli: Cli,
        matches: ArgMatches,
        wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
    ) -> Result<Self, Error> {
        let cli_config = if let Some(config_file) = &cli.config.config_file {
            solana_cli_config::Config::load(config_file).unwrap_or_else(|_| {
                eprintln!("error: Could not load config file `{}`", config_file);
                std::process::exit(1);
            })
        } else if let Some(config_file) = &*solana_cli_config::CONFIG_FILE {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };

        // create rpc client
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            cli.config.json_rpc_url.unwrap_or(cli_config.json_rpc_url),
            CommitmentConfig::confirmed(),
        ));

        // and program client
        let program_client = Arc::new(ProgramRpcClient::new(
            rpc_client.clone(),
            ProgramRpcClientSendTransaction,
        ));

        let fee_payer = cli.config.fee_payer.map(|s| {
            Arc::from(signer_from_source(&matches, &s, "fee_payer", wallet_manager).unwrap())
        });

        let output_format = cli.config.output_format.unwrap_or(OutputFormat::Display);

        Ok(Self {
            rpc_client,
            program_client,
            fee_payer,
            output_format,
            dry_run: cli.config.dry_run,
        })
    }

    // Returns Ok(default signer), or Err if there is no default signer configured
    pub fn fee_payer(&self) -> Result<Arc<dyn Signer>, Error> {
        if let Some(fee_payer) = &self.fee_payer {
            Ok(fee_payer.clone())
        } else {
            Err(
                "fee payer is required, please specify a valid fee payer using the --payer \
                 argument, or by identifying a valid configuration file using the --config \
                 argument, or by creating a valid config at the default location of \
                 ~/.config/solana/cli/config.yml using the solana config command"
                    .to_string()
                    .into(),
            )
        }
    }

    pub fn verbose(&self) -> bool {
        self.output_format == OutputFormat::DisplayVerbose
    }
}
