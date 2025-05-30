use {
    crate::helpers::{
        common::{init_mollusk, setup_mint},
        create_mint_builder::{KeyedAccount, TokenProgram},
        wrap_builder::TransferAuthority,
    },
    mollusk_svm::{result::Check, Mollusk},
    solana_account::Account,
    solana_instruction::AccountMeta,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            transfer_fee::TransferFeeAmount, BaseStateWithExtensionsMut, ExtensionType,
            PodStateWithExtensionsMut,
        },
        pod::{PodAccount, PodCOption},
    },
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority, instruction::unwrap},
};

pub struct UnwrapBuilder<'a> {
    mollusk: Mollusk,
    unwrap_amount: Option<u64>,
    checks: Vec<Check<'a>>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
    escrow_starting_amount: Option<u64>,
    unwrapped_escrow_owner: Option<Pubkey>,
    wrapped_token_starting_amount: Option<u64>,
    recipient_starting_amount: Option<u64>,
    unwrapped_token_program: Option<TokenProgram>,
    wrapped_token_program: Option<TokenProgram>,
    transfer_authority: Option<TransferAuthority>,
    unwrapped_mint: Option<KeyedAccount>,
    unwrapped_escrow_account: Option<KeyedAccount>,
    extra_accounts: Vec<KeyedAccount>,
    recipient_token_account: Option<KeyedAccount>,
}

impl Default for UnwrapBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            unwrap_amount: None,
            checks: vec![],
            wrapped_mint: None,
            wrapped_mint_authority: None,
            escrow_starting_amount: None,
            unwrapped_escrow_owner: None,
            wrapped_token_starting_amount: None,
            recipient_starting_amount: None,
            unwrapped_token_program: None,
            wrapped_token_program: None,
            transfer_authority: None,
            unwrapped_mint: None,
            unwrapped_escrow_account: None,
            extra_accounts: vec![],
            recipient_token_account: None,
        }
    }
}

impl<'a> UnwrapBuilder<'a> {
    pub fn unwrap_amount(mut self, amount: u64) -> Self {
        self.unwrap_amount = Some(amount);
        self
    }

    pub fn wrapped_token_starting_amount(mut self, amount: u64) -> Self {
        self.wrapped_token_starting_amount = Some(amount);
        self
    }

    pub fn escrow_starting_amount(mut self, amount: u64) -> Self {
        self.escrow_starting_amount = Some(amount);
        self
    }

    pub fn unwrapped_escrow_owner(mut self, key: Pubkey) -> Self {
        self.unwrapped_escrow_owner = Some(key);
        self
    }

    pub fn unwrapped_escrow_account(mut self, account: Account) -> Self {
        self.unwrapped_escrow_account = Some(KeyedAccount {
            key: Pubkey::new_unique(),
            account,
        });
        self
    }

    pub fn unwrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.unwrapped_mint = Some(account);
        self
    }

    pub fn wrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.wrapped_mint = Some(account);
        self
    }

    pub fn wrapped_token_program(mut self, program: TokenProgram) -> Self {
        self.wrapped_token_program = Some(program);
        self
    }

    pub fn unwrapped_token_program(mut self, program: TokenProgram) -> Self {
        self.unwrapped_token_program = Some(program);
        self
    }

    pub fn wrapped_mint_authority(mut self, key: Pubkey) -> Self {
        self.wrapped_mint_authority = Some(key);
        self
    }

    pub fn recipient_starting_amount(mut self, amount: u64) -> Self {
        self.recipient_starting_amount = Some(amount);
        self
    }

    pub fn recipient_token_account(mut self, account: Account) -> Self {
        self.recipient_token_account = Some(KeyedAccount {
            key: Pubkey::new_unique(),
            account,
        });
        self
    }

    pub fn transfer_authority(mut self, auth: TransferAuthority) -> Self {
        self.transfer_authority = Some(auth);
        self
    }

    pub fn add_extra_account(mut self, keyed_account: KeyedAccount) -> Self {
        self.extra_accounts.push(keyed_account);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    fn get_wrapped_mint(
        &self,
        token_program: TokenProgram,
        unwrapped_mint_addr: Pubkey,
    ) -> KeyedAccount {
        let wrapped_mint_addr = get_wrapped_mint_address(&unwrapped_mint_addr, &token_program.id());
        let mint_authority = get_wrapped_mint_authority(&wrapped_mint_addr);

        self.wrapped_mint.clone().unwrap_or(KeyedAccount {
            key: wrapped_mint_addr,
            account: setup_mint(
                token_program.clone(),
                &self.mollusk.sysvars.rent,
                mint_authority,
            ),
        })
    }

    pub fn setup_token_account(
        &self,
        token_program: TokenProgram,
        mint: &KeyedAccount,
        owner: &Pubkey,
        starting_amount: u64,
    ) -> KeyedAccount {
        let extensions = token_program.extensions();

        let account_size =
            ExtensionType::try_calculate_account_len::<PodAccount>(&extensions).unwrap();

        let mut token_account = Account {
            lamports: 100_000_000,
            owner: mint.account.owner,
            data: vec![0; account_size],
            ..Default::default()
        };

        let account_data = PodAccount {
            mint: mint.key,
            owner: *owner,
            amount: starting_amount.into(),
            delegate: PodCOption::none(),
            state: spl_token_2022::state::AccountState::Initialized.into(),
            is_native: PodCOption::none(),
            delegated_amount: 0.into(),
            close_authority: PodCOption::none(),
        };

        let mut state =
            PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut token_account.data)
                .unwrap();
        *state.base = account_data;
        state.init_account_type().unwrap();

        if let TokenProgram::SplToken2022 { extensions } = token_program {
            if extensions.contains(&ExtensionType::TransferFeeAmount) {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
                let fee_extension = state.get_extension_mut::<TransferFeeAmount>().unwrap();
                fee_extension.withheld_amount = 12.into();
            }
        }

        KeyedAccount {
            key: Pubkey::new_unique(),
            account: token_account,
        }
    }

    pub fn execute(mut self) -> UnwrapResult {
        let unwrap_amount = self.unwrap_amount.unwrap_or(500);
        let transfer_authority = self.transfer_authority.clone().unwrap_or_default();

        let unwrapped_token_program = self
            .unwrapped_token_program
            .clone()
            .unwrap_or(TokenProgram::SplToken);

        let unwrapped_mint = self.unwrapped_mint.clone().unwrap_or(KeyedAccount {
            key: Pubkey::new_unique(),
            account: setup_mint(
                unwrapped_token_program.clone(),
                &self.mollusk.sysvars.rent,
                Pubkey::new_unique(),
            ),
        });

        let wrapped_token_program = self
            .wrapped_token_program
            .clone()
            .unwrap_or(TokenProgram::default_2022());

        let wrapped_mint = self.wrapped_mint.clone().unwrap_or_else(|| {
            self.get_wrapped_mint(wrapped_token_program.clone(), unwrapped_mint.key)
        });

        let wrapped_mint_authority = self
            .wrapped_mint_authority
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint.key));

        // Setup wrapped token account to be unwrapped
        let wrapped_token_account = self.setup_token_account(
            wrapped_token_program.clone(),
            &wrapped_mint,
            &transfer_authority.keyed_account.key,
            self.wrapped_token_starting_amount.unwrap_or(unwrap_amount),
        );

        // Setup escrow account
        let escrow = self.unwrapped_escrow_account.clone().unwrap_or(
            self.setup_token_account(
                unwrapped_token_program.clone(),
                &unwrapped_mint,
                &self
                    .unwrapped_escrow_owner
                    .unwrap_or(wrapped_mint_authority),
                self.escrow_starting_amount.unwrap_or(100_000),
            ),
        );

        // Setup recipient account for unwrapped tokens
        let recipient = self.recipient_token_account.clone().unwrap_or_else(|| {
            self.setup_token_account(
                unwrapped_token_program.clone(),
                &unwrapped_mint,
                &Pubkey::new_unique(),
                self.recipient_starting_amount.unwrap_or(0),
            )
        });

        let mut instruction = unwrap(
            &spl_token_wrap::id(),
            &escrow.key,
            &recipient.key,
            &wrapped_mint_authority,
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
            &unwrapped_token_program.id(),
            &wrapped_token_account.key,
            &wrapped_mint.key,
            &transfer_authority.keyed_account.key,
            &transfer_authority.signers.iter().collect::<Vec<_>>(),
            unwrap_amount,
        );

        let mut accounts = vec![
            escrow.pair(),
            recipient.pair(),
            (wrapped_mint_authority, Account::default()),
            unwrapped_mint.pair(),
            wrapped_token_program.keyed_account(),
            unwrapped_token_program.keyed_account(),
            wrapped_token_account.pair(),
            wrapped_mint.pair(),
            transfer_authority.keyed_account.pair(),
        ];

        for signer_key in &transfer_authority.signers {
            accounts.push((*signer_key, Account::default()));
        }

        for extra_account in &self.extra_accounts {
            instruction
                .accounts
                .push(AccountMeta::new(extra_account.key, false));
            accounts.push(extra_account.pair());
        }

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, &accounts, &self.checks);

        UnwrapResult {
            wrapped_token_account: KeyedAccount {
                key: wrapped_token_account.key,
                account: result
                    .get_account(&wrapped_token_account.key)
                    .unwrap()
                    .clone(),
            },
            unwrapped_escrow: KeyedAccount {
                key: escrow.key,
                account: result.get_account(&escrow.key).unwrap().clone(),
            },
            wrapped_mint: KeyedAccount {
                key: wrapped_mint.key,
                account: result.get_account(&wrapped_mint.key).unwrap().clone(),
            },
            recipient_unwrapped_token: KeyedAccount {
                key: recipient.key,
                account: result.get_account(&recipient.key).unwrap().clone(),
            },
            extra_accounts: self
                .extra_accounts
                .iter()
                .map(|keyed_account| KeyedAccount {
                    key: keyed_account.key,
                    account: result.get_account(&keyed_account.key).unwrap().clone(),
                })
                .collect(),
        }
    }
}

pub struct UnwrapResult {
    pub wrapped_token_account: KeyedAccount,
    pub unwrapped_escrow: KeyedAccount,
    pub wrapped_mint: KeyedAccount,
    pub recipient_unwrapped_token: KeyedAccount,
    pub extra_accounts: Vec<KeyedAccount>,
}
