use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
    },
    mollusk_svm::{result::Check, Mollusk},
    solana_account::Account,
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::sync_metadata_to_token_2022,
    },
};

pub struct SyncMetadataResult {
    pub unwrapped_mint: KeyedAccount,
    pub wrapped_mint: KeyedAccount,
}

pub struct SyncMetadataBuilder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    unwrapped_mint: Option<KeyedAccount>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
}

impl Default for SyncMetadataBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: Vec::new(),
            unwrapped_mint: None,
            wrapped_mint: None,
            wrapped_mint_authority: None,
        }
    }
}

impl<'a> SyncMetadataBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unwrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.unwrapped_mint = Some(account);
        self
    }

    pub fn wrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.wrapped_mint = Some(account);
        self
    }

    pub fn wrapped_mint_authority(mut self, authority: Pubkey) -> Self {
        self.wrapped_mint_authority = Some(authority);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn execute(mut self) -> SyncMetadataResult {
        let unwrapped_mint = self.unwrapped_mint.unwrap_or_else(|| {
            MintBuilder::new()
                .token_program(TokenProgram::SplToken2022)
                .with_extension(MintExtension::TokenMetadata {
                    name: "Unwrapped".to_string(),
                    symbol: "UP".to_string(),
                    uri: "uri://unwrapped.com".to_string(),
                    additional_metadata: vec![],
                })
                .build()
        });

        let wrapped_mint_address =
            get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());

        let wrapped_mint_authority = self
            .wrapped_mint_authority
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint_address));

        let wrapped_mint = self.wrapped_mint.unwrap_or_else(|| {
            MintBuilder::new()
                .token_program(TokenProgram::SplToken2022)
                .mint_key(wrapped_mint_address)
                .mint_authority(wrapped_mint_authority)
                .lamports(1_000_000_000) // Add sufficient lamports for rent
                .build()
        });

        let instruction = sync_metadata_to_token_2022(
            &spl_token_wrap::id(),
            &wrapped_mint.key,
            &wrapped_mint_authority,
            &unwrapped_mint.key,
        );

        let accounts = &[
            wrapped_mint.pair(),
            (wrapped_mint_authority, Account::default()),
            unwrapped_mint.pair(),
            TokenProgram::SplToken2022.keyed_account(),
        ];

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, accounts, &self.checks);

        SyncMetadataResult {
            unwrapped_mint: KeyedAccount {
                key: unwrapped_mint.key,
                account: result.get_account(&unwrapped_mint.key).unwrap().clone(),
            },
            wrapped_mint: KeyedAccount {
                key: wrapped_mint.key,
                account: result.get_account(&wrapped_mint.key).unwrap().clone(),
            },
        }
    }
}
