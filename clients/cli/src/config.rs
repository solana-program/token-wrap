use {
    crate::{cli::Cli, Error},
    anyhow::anyhow,
    clap::ArgMatches,
    solana_clap_v3_utils::keypair::{signer_from_path, signer_from_source},
    solana_cli_output::OutputFormat,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_commitment_config::CommitmentConfig,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signer::Signer,
    std::{rc::Rc, sync::Arc},
};

pub struct Config {
    pub rpc_client: Arc<RpcClient>,
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
        let cli_config = if let Some(config_file) = &cli.config_file {
            solana_cli_config::Config::load(config_file)
                .map_err(|e| anyhow!("Could not load config file: {}", e))?
        } else if let Some(config_file) = &*solana_cli_config::CONFIG_FILE {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };

        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            cli.json_rpc_url.unwrap_or(cli_config.json_rpc_url),
            CommitmentConfig::confirmed(),
        ));

        let fee_payer = match &cli.fee_payer {
            Some(fee_payer_source) => {
                signer_from_source(&matches, fee_payer_source, "fee_payer", wallet_manager)
            }
            None => signer_from_path(
                &matches,
                &cli_config.keypair_path,
                "default",
                wallet_manager,
            ),
        }
        .ok()
        .map(Arc::from);

        let output_format = match (cli.output_format, cli.verbose) {
            (Some(format), _) => format,
            (None, true) => OutputFormat::DisplayVerbose,
            (None, false) => OutputFormat::Display,
        };

        Ok(Self {
            rpc_client,
            fee_payer,
            output_format,
            dry_run: cli.dry_run,
        })
    }

    /// Returns `Ok(default signer)`, or Err if there is no default signer
    /// configured
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
