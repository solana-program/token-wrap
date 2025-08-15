use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
    },
    borsh::BorshSerialize,
    mollusk_svm::{result::Check, Mollusk},
    mpl_token_metadata::{accounts::Metadata as MetaplexMetadata, types::Key},
    solana_account::Account,
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

pub struct SyncMetadataBuilder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    unwrapped_mint: Option<KeyedAccount>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
    metaplex_metadata: Option<KeyedAccount>,
}

impl Default for SyncMetadataBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: Vec::new(),
            unwrapped_mint: None,
            wrapped_mint: None,
            wrapped_mint_authority: None,
            metaplex_metadata: None,
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

    pub fn metaplex_metadata(mut self, account: KeyedAccount) -> Self {
        self.metaplex_metadata = Some(account);
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

        let metaplex_metadata: Option<KeyedAccount> = self.metaplex_metadata.or_else(|| {
            if unwrapped_mint.account.owner == spl_token::id() {
                let metadata = MetaplexMetadata {
                    key: Key::MetadataV1,
                    update_authority: Default::default(),
                    mint: unwrapped_mint.key,
                    name: "x".to_string(),
                    symbol: "y".to_string(),
                    uri: "z".to_string(),
                    seller_fee_basis_points: 0,
                    creators: None,
                    primary_sale_happened: false,
                    is_mutable: false,
                    edition_nonce: None,
                    token_standard: None,
                    collection: None,
                    uses: None,
                    collection_details: None,
                    programmable_config: None,
                };
                Some(KeyedAccount {
                    key: MetaplexMetadata::find_pda(&unwrapped_mint.key).0,
                    account: Account {
                        lamports: 1_000_000_000,
                        data: metadata.try_to_vec().unwrap(),
                        owner: mpl_token_metadata::ID,
                        ..Default::default()
                    },
                })
            } else {
                None
            }
        });

        let instruction = sync_metadata_to_token_2022(
            &id(),
            &wrapped_mint.key,
            &wrapped_mint_authority,
            &unwrapped_mint.key,
            metaplex_metadata.as_ref().map(|ka| &ka.key),
        );

        let mut accounts = vec![
            wrapped_mint.pair(),
            (wrapped_mint_authority, Account::default()),
            unwrapped_mint.pair(),
            TokenProgram::SplToken2022.keyed_account(),
        ];

        if let Some(metadata) = metaplex_metadata {
            accounts.push(metadata.pair());
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
