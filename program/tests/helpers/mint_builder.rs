use {
    crate::helpers::{
        common::{KeyedAccount, TokenProgram, DEFAULT_MINT_DECIMALS, DEFAULT_MINT_SUPPLY},
        extensions::{calc_mint_len, init_mint_extensions, MintExtension},
    },
    solana_account::Account,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_pod::primitives::{PodBool, PodU64},
    spl_token_2022::{
        extension::{BaseStateWithExtensionsMut, PodStateWithExtensionsMut},
        pod::{PodCOption, PodMint},
        state::Mint,
    },
};

pub struct MintBuilder {
    token_program: TokenProgram,
    mint_authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
    supply: u64,
    decimals: u8,
    mint_key: Option<Pubkey>,
    lamports: Option<u64>,
    extensions: Vec<MintExtension>,
}

impl Default for MintBuilder {
    fn default() -> Self {
        Self {
            token_program: TokenProgram::SplToken,
            mint_authority: None,
            freeze_authority: None,
            supply: DEFAULT_MINT_SUPPLY,
            decimals: DEFAULT_MINT_DECIMALS,
            mint_key: None,
            lamports: None,
            extensions: Vec::new(),
        }
    }
}

impl MintBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn token_program(mut self, program: TokenProgram) -> Self {
        self.token_program = program;
        self
    }

    pub fn mint_authority(mut self, authority: Pubkey) -> Self {
        self.mint_authority = Some(authority);
        self
    }

    pub fn freeze_authority(mut self, authority: Pubkey) -> Self {
        self.freeze_authority = Some(authority);
        self
    }

    pub fn supply(mut self, supply: u64) -> Self {
        self.supply = supply;
        self
    }

    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    pub fn mint_key(mut self, key: Pubkey) -> Self {
        self.mint_key = Some(key);
        self
    }

    pub fn lamports(mut self, lamports: u64) -> Self {
        self.lamports = Some(lamports);
        self
    }

    pub fn with_extension(mut self, extension: MintExtension) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn build(self) -> KeyedAccount {
        let mint_key = self.mint_key.unwrap_or_else(Pubkey::new_unique);
        let mint_size = match self.token_program {
            TokenProgram::SplToken2022 => calc_mint_len(&mint_key, &self.extensions),
            TokenProgram::SplToken => {
                assert!(
                    self.extensions.is_empty(),
                    "SPL Token doesn't support extensions"
                );
                Mint::LEN
            }
        };
        let mut buffer = vec![0; mint_size];
        let mut state =
            PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();

        // Set base mint data
        state.base.decimals = self.decimals;
        state.base.is_initialized = PodBool::from_bool(true);
        state.base.supply = PodU64::from(self.supply);
        let mint_authority = self.mint_authority.unwrap_or_else(Pubkey::new_unique);
        state.base.mint_authority = PodCOption::some(mint_authority);
        state.base.freeze_authority = self
            .freeze_authority
            .map(PodCOption::some)
            .unwrap_or(PodCOption::none());

        state.init_account_type().unwrap();

        // Initialize extensions (only for token 2022)
        if self.token_program == TokenProgram::SplToken2022 {
            init_mint_extensions(&mut state, &self.extensions, &mint_key);
        }

        let lamports = self
            .lamports
            .unwrap_or_else(|| Rent::default().minimum_balance(buffer.len()));

        KeyedAccount {
            key: mint_key,
            account: Account {
                lamports,
                data: buffer,
                owner: self.token_program.id(),
                ..Default::default()
            },
        }
    }
}
