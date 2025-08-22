use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
    },
    mollusk_svm::{program::create_program_account_loader_v3, result::Check, Mollusk},
    solana_account::Account,
    solana_instruction::AccountMeta,
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority, id,
        instruction::sync_metadata_to_token_2022,
    },
};

pub struct SyncMetadataResult {
    pub wrapped_mint: KeyedAccount,
    pub wrapped_mint_authority: KeyedAccount,
}

pub struct SyncToToken2022Builder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    unwrapped_mint: Option<KeyedAccount>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
    source_metadata: Option<KeyedAccount>,
}

impl Default for SyncToToken2022Builder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: Vec::new(),
            unwrapped_mint: None,
            wrapped_mint: None,
            wrapped_mint_authority: None,
            source_metadata: None,
        }
    }
}

impl<'a> SyncToToken2022Builder<'a> {
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

    pub fn source_metadata(mut self, account: KeyedAccount) -> Self {
        self.source_metadata = Some(account);
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
                    name: "Alphabet".to_string(),
                    symbol: "ABC".to_string(),
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
                .with_extension(MintExtension::MetadataPointer {
                    metadata_address: Some(wrapped_mint_address),
                })
                .lamports(1_000_000_000)
                .build()
        });

        let source_metadata_key_opt = self.source_metadata.as_ref().map(|k| k.key);
        let owner_program_opt = self.source_metadata.as_ref().and_then(|k| {
            let owner = k.account.owner;
            let is_metaplex = owner == mpl_token_metadata::ID;
            let is_token2022 = owner == spl_token_2022::id();
            if !is_metaplex && !is_token2022 {
                Some(owner)
            } else {
                None
            }
        });

        let mut instruction = sync_metadata_to_token_2022(
            &id(),
            &wrapped_mint.key,
            &wrapped_mint_authority,
            &unwrapped_mint.key,
            source_metadata_key_opt.as_ref(),
            owner_program_opt.as_ref(),
        );

        let mut accounts = vec![
            wrapped_mint.pair(),
            (wrapped_mint_authority, Account::default()),
            unwrapped_mint.pair(),
            TokenProgram::SplToken2022.keyed_account(),
        ];

        if let Some(metadata) = self.source_metadata.as_ref() {
            accounts.push(metadata.pair());
        }

        if let Some(program) = owner_program_opt {
            instruction
                .accounts
                .push(AccountMeta::new_readonly(program, false));
            accounts.push((
                program,
                create_program_account_loader_v3(&Pubkey::new_unique()),
            ));
        }

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, &accounts, &self.checks);

        SyncMetadataResult {
            wrapped_mint: KeyedAccount {
                key: wrapped_mint.key,
                account: result.get_account(&wrapped_mint.key).unwrap().clone(),
            },
            wrapped_mint_authority: KeyedAccount {
                key: wrapped_mint_authority,
                account: result.get_account(&wrapped_mint_authority).unwrap().clone(),
            },
        }
    }
}
