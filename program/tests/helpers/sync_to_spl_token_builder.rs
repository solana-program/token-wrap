use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        mint_builder::MintBuilder,
    },
    borsh::BorshSerialize,
    mollusk_svm::{result::Check, Mollusk},
    mpl_token_metadata::{accounts::Metadata as MetaplexMetadata, types::Key},
    solana_account::Account,
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::sync_metadata_to_spl_token,
    },
};

pub struct SyncToSplTokenResult {
    pub wrapped_mint: KeyedAccount,
    pub wrapped_mint_authority: KeyedAccount,
    pub metaplex_metadata: KeyedAccount,
}

pub struct SyncToSplTokenBuilder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    unwrapped_mint: Option<KeyedAccount>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
    wrapped_mint_authority_lamports: Option<u64>,
    source_metadata: Option<KeyedAccount>,
    metaplex_metadata: Option<KeyedAccount>,
}

impl Default for SyncToSplTokenBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: Vec::new(),
            unwrapped_mint: None,
            wrapped_mint: None,
            wrapped_mint_authority: None,
            wrapped_mint_authority_lamports: None,
            source_metadata: None,
            metaplex_metadata: None,
        }
    }
}

impl<'a> SyncToSplTokenBuilder<'a> {
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

    pub fn wrapped_mint_authority_lamports(mut self, lamports: u64) -> Self {
        self.wrapped_mint_authority_lamports = Some(lamports);
        self
    }

    pub fn source_metadata(mut self, account: KeyedAccount) -> Self {
        self.source_metadata = Some(account);
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

    pub fn execute(mut self) -> SyncToSplTokenResult {
        let unwrapped_mint = self.unwrapped_mint.unwrap_or_else(|| {
            MintBuilder::new()
                .token_program(TokenProgram::SplToken2022)
                .build()
        });

        if unwrapped_mint.account.owner == spl_token::id() && self.source_metadata.is_none() {
            let (source_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
            let source_metadata_obj = MetaplexMetadata {
                key: Key::MetadataV1,
                update_authority: Pubkey::new_unique(),
                mint: unwrapped_mint.key,
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                uri: "uri".to_string(),
                seller_fee_basis_points: 0,
                creators: None,
                primary_sale_happened: false,
                is_mutable: true,
                edition_nonce: None,
                token_standard: None,
                collection: None,
                uses: None,
                collection_details: None,
                programmable_config: None,
            };
            let source_account = Account {
                data: source_metadata_obj.try_to_vec().unwrap(),
                owner: mpl_token_metadata::ID,
                lamports: 1_000_000_000,
                ..Default::default()
            };
            self.source_metadata = Some(KeyedAccount {
                key: source_pda,
                account: source_account,
            });
        }

        let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());

        let wrapped_mint_authority = self
            .wrapped_mint_authority
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint_address));

        let wrapped_mint = self.wrapped_mint.unwrap_or_else(|| {
            MintBuilder::new()
                .token_program(TokenProgram::SplToken)
                .mint_key(wrapped_mint_address)
                .mint_authority(wrapped_mint_authority)
                .build()
        });

        let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint.key);
        let metaplex_metadata = self.metaplex_metadata.unwrap_or_else(|| KeyedAccount {
            key: metaplex_pda,
            account: Account::default(),
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

        let instruction = sync_metadata_to_spl_token(
            &spl_token_wrap::id(),
            &metaplex_metadata.key,
            &wrapped_mint_authority,
            &wrapped_mint.key,
            &unwrapped_mint.key,
            source_metadata_key_opt.as_ref(),
            owner_program_opt.as_ref(),
        );

        let authority_lamports = self
            .wrapped_mint_authority_lamports
            .unwrap_or(10_000_000_000);

        let mut accounts = vec![
            metaplex_metadata.pair(),
            (
                wrapped_mint_authority,
                Account {
                    lamports: authority_lamports,
                    data: vec![],
                    owner: solana_system_interface::program::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            wrapped_mint.pair(),
            unwrapped_mint.pair(),
            (
                mpl_token_metadata::ID,
                mollusk_svm::program::create_program_account_loader_v3(&mpl_token_metadata::ID),
            ),
            mollusk_svm::program::keyed_account_for_system_program(),
            self.mollusk.sysvars.keyed_account_for_rent_sysvar(),
        ];

        if let Some(metadata) = self.source_metadata.as_ref() {
            accounts.push(metadata.pair());
        }

        if let Some(program) = owner_program_opt {
            accounts.push((
                program,
                mollusk_svm::program::create_program_account_loader_v3(&program),
            ));
        }

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, &accounts, &self.checks);

        SyncToSplTokenResult {
            wrapped_mint: KeyedAccount {
                key: wrapped_mint.key,
                account: result.get_account(&wrapped_mint.key).unwrap().clone(),
            },
            wrapped_mint_authority: KeyedAccount {
                key: wrapped_mint_authority,
                account: result.get_account(&wrapped_mint_authority).unwrap().clone(),
            },
            metaplex_metadata: KeyedAccount {
                key: metaplex_metadata.key,
                account: result.get_account(&metaplex_metadata.key).unwrap().clone(),
            },
        }
    }
}
