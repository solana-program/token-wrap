use {
    crate::helpers::{
        common::{KeyedAccount, TokenProgram},
        extensions::init_token_account_extensions,
    },
    solana_account::Account,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_token_2022::{
        extension::{BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut},
        pod::{PodAccount, PodCOption},
        state::AccountState,
    },
};

pub struct TokenAccountBuilder {
    token_program: TokenProgram,
    mint: Option<KeyedAccount>,
    owner: Option<Pubkey>,
    amount: u64,
    account_key: Option<Pubkey>,
    lamports: Option<u64>,
    delegate: Option<Pubkey>,
    delegated_amount: u64,
    close_authority: Option<Pubkey>,
    state: AccountState,
    is_native: Option<u64>,
    extensions: Vec<ExtensionType>,
}

impl Default for TokenAccountBuilder {
    fn default() -> Self {
        Self {
            token_program: TokenProgram::SplToken,
            mint: None,
            owner: None,
            amount: 0,
            account_key: None,
            lamports: None,
            delegate: None,
            delegated_amount: 0,
            close_authority: None,
            state: AccountState::Initialized,
            is_native: None,
            extensions: Vec::new(),
        }
    }
}

impl TokenAccountBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn token_program(mut self, program: TokenProgram) -> Self {
        self.token_program = program;
        self
    }

    pub fn mint(mut self, mint: KeyedAccount) -> Self {
        self.mint = Some(mint);
        self
    }

    pub fn owner(mut self, owner: Pubkey) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn amount(mut self, amount: u64) -> Self {
        self.amount = amount;
        self
    }

    pub fn account_key(mut self, key: Pubkey) -> Self {
        self.account_key = Some(key);
        self
    }

    pub fn lamports(mut self, lamports: u64) -> Self {
        self.lamports = Some(lamports);
        self
    }

    pub fn delegate(mut self, delegate: Pubkey) -> Self {
        self.delegate = Some(delegate);
        self
    }

    pub fn delegated_amount(mut self, amount: u64) -> Self {
        self.delegated_amount = amount;
        self
    }

    pub fn close_authority(mut self, authority: Pubkey) -> Self {
        self.close_authority = Some(authority);
        self
    }

    pub fn state(mut self, state: AccountState) -> Self {
        self.state = state;
        self
    }

    pub fn native_balance(mut self, native_balance: u64) -> Self {
        self.is_native = Some(native_balance);
        self
    }

    pub fn with_extension(mut self, extension: ExtensionType) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn build(self) -> KeyedAccount {
        let mint = self.mint.expect("Mint is required for token account");
        let owner = self.owner.unwrap_or_else(Pubkey::new_unique);
        let account_key = self.account_key.unwrap_or_else(Pubkey::new_unique);
        let account_owner = self.token_program.id();

        let extension_types = match self.token_program {
            TokenProgram::SplToken2022 => self.extensions.clone(),
            TokenProgram::SplToken => {
                if self.extensions.is_empty() {
                    vec![]
                } else {
                    panic!("SPL Token doesn't support extensions, but extensions were provided");
                }
            }
        };

        let account_size =
            ExtensionType::try_calculate_account_len::<PodAccount>(&extension_types).unwrap();
        let mut account_data = vec![0; account_size];
        let mut state =
            PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut account_data)
                .unwrap();

        // Set base account data
        state.base.mint = mint.key;
        state.base.owner = owner;
        state.base.amount = self.amount.into();
        state.base.delegate = self
            .delegate
            .map(PodCOption::some)
            .unwrap_or(PodCOption::none());
        state.base.state = self.state.into();
        state.base.is_native = self
            .is_native
            .map(|n| PodCOption::some(n.into()))
            .unwrap_or(PodCOption::none());
        state.base.delegated_amount = self.delegated_amount.into();
        state.base.close_authority = self
            .close_authority
            .map(PodCOption::some)
            .unwrap_or(PodCOption::none());

        state.init_account_type().unwrap();

        // Initialize extensions (only for token 2022)
        if self.token_program == TokenProgram::SplToken2022 {
            init_token_account_extensions(&mut state, &self.extensions);
        }

        let lamports = self.lamports.unwrap_or_else(|| {
            let base_lamports = Rent::default().minimum_balance(account_data.len());
            if let Some(native_balance) = self.is_native {
                base_lamports.checked_add(native_balance).unwrap()
            } else {
                base_lamports
            }
        });

        KeyedAccount {
            key: account_key,
            account: Account {
                lamports,
                data: account_data,
                owner: account_owner,
                ..Default::default()
            },
        }
    }
}
