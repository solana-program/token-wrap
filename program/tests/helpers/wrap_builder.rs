use {
    crate::helpers::mint_builder::{CreateMintResult, KeyedAccount},
    mollusk_svm::{result::Check, Mollusk},
    solana_account::Account,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_token_wrap::{get_escrow_address, get_wrapped_mint_authority, instruction::wrap},
};

pub struct WrapBuilder<'a> {
    mollusk: &'a mut Mollusk,
    mint_result: CreateMintResult,
    wrap_amount: Option<u64>,
    recipient: Option<KeyedAccount>,
    checks: Vec<Check<'a>>,
    wrapped_mint: Option<KeyedAccount>,
    unwrapped_escrow_addr: Option<Pubkey>,
    wrapped_mint_authority: Option<Pubkey>,
    unwrapped_escrow_owner: Option<Pubkey>,
    recipient_starting_amount: Option<u64>,
    unwrapped_token_starting_amount: Option<u64>,
    unwrapped_escrow_account: Option<Account>,
}

impl<'a> WrapBuilder<'a> {
    pub fn new(mollusk: &'a mut Mollusk, mint_result: CreateMintResult) -> Self {
        Self {
            mollusk,
            mint_result,
            wrap_amount: None,
            recipient: None,
            checks: vec![],
            wrapped_mint: None,
            unwrapped_escrow_addr: None,
            wrapped_mint_authority: None,
            unwrapped_escrow_owner: None,
            recipient_starting_amount: None,
            unwrapped_token_starting_amount: None,
            unwrapped_escrow_account: None,
        }
    }

    pub fn wrap_amount(mut self, amount: u64) -> Self {
        self.wrap_amount = Some(amount);
        self
    }

    pub fn unwrapped_token_starting_amount(mut self, amount: u64) -> Self {
        self.unwrapped_token_starting_amount = Some(amount);
        self
    }

    pub fn wrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.wrapped_mint = Some(account);
        self
    }

    pub fn unwrapped_escrow_addr(mut self, key: Pubkey) -> Self {
        self.unwrapped_escrow_addr = Some(key);
        self
    }

    pub fn unwrapped_escrow_account(mut self, account: Account) -> Self {
        self.unwrapped_escrow_account = Some(account);
        self
    }

    pub fn unwrapped_escrow_owner(mut self, key: Pubkey) -> Self {
        self.unwrapped_escrow_owner = Some(key);
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

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn setup_token_account(&self) -> KeyedAccount {
        let recipient_addr = Pubkey::new_unique();
        let mut recipient_token_account = Account {
            lamports: 100_000_000,
            owner: self.mint_result.wrapped_mint.account.owner,
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };
        let recipient_account_data = spl_token::state::Account {
            mint: self.mint_result.wrapped_mint.key,
            owner: recipient_addr,
            amount: self.recipient_starting_amount.unwrap_or(0),
            delegate: None.into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token::state::Account::pack(recipient_account_data, &mut recipient_token_account.data)
            .unwrap();
        KeyedAccount {
            key: recipient_addr,
            account: recipient_token_account,
        }
    }

    pub fn execute(mut self) -> WrapResult {
        let unwrapped_token_account_address = Pubkey::new_unique();
        let unwrapped_token_account_authority = Pubkey::new_unique();

        let wrapped_mint_account = self
            .wrapped_mint
            .clone()
            .unwrap_or(self.mint_result.wrapped_mint.clone());

        let mut unwrapped_token_account = Account {
            lamports: 100_000_000,
            owner: self.mint_result.unwrapped_mint.account.owner,
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };

        let wrap_amount = self.wrap_amount.unwrap_or(500);

        let token = spl_token::state::Account {
            mint: self.mint_result.unwrapped_mint.key,
            owner: unwrapped_token_account_authority,
            amount: self.unwrapped_token_starting_amount.unwrap_or(wrap_amount),
            delegate: None.into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token::state::Account::pack(token, &mut unwrapped_token_account.data).unwrap();

        let unwrapped_escrow_address = self.unwrapped_escrow_addr.unwrap_or_else(|| {
            get_escrow_address(
                &unwrapped_token_account_authority,
                &self.mint_result.unwrapped_mint.key,
            )
        });
        let wrapped_mint_authority = self
            .wrapped_mint_authority
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint_account.key));

        let mut unwrapped_escrow_account = Account {
            lamports: 100_000_000,
            owner: self.mint_result.unwrapped_mint.account.owner,
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };
        let escrow_token = spl_token::state::Account {
            mint: self.mint_result.unwrapped_mint.key,
            owner: self
                .unwrapped_escrow_owner
                .unwrap_or(wrapped_mint_authority),
            amount: 0,
            delegate: None.into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token::state::Account::pack(escrow_token, &mut unwrapped_escrow_account.data).unwrap();

        let recipient = self
            .recipient
            .clone()
            .unwrap_or_else(|| self.setup_token_account());

        let unwrapped_token_program =
            if self.mint_result.unwrapped_mint.account.owner == spl_token_2022::id() {
                mollusk_svm_programs_token::token2022::keyed_account()
            } else {
                mollusk_svm_programs_token::token::keyed_account()
            };

        let wrapped_token_program =
            if self.mint_result.wrapped_mint.account.owner == spl_token_2022::id() {
                mollusk_svm_programs_token::token2022::keyed_account()
            } else {
                mollusk_svm_programs_token::token::keyed_account()
            };

        let instruction = wrap(
            &spl_token_wrap::id(),
            &unwrapped_token_account_authority,
            &unwrapped_escrow_address,
            &unwrapped_token_account_address,
            &recipient.key,
            &wrapped_mint_account.key,
            &unwrapped_token_program.0,
            &wrapped_token_program.0,
            &self.mint_result.unwrapped_mint.key,
            &wrapped_mint_authority,
            &[],
            wrap_amount,
        );

        let accounts = &[
            (unwrapped_token_account_authority, Account::default()),
            (
                unwrapped_escrow_address,
                self.unwrapped_escrow_account
                    .unwrap_or(unwrapped_escrow_account),
            ),
            (unwrapped_token_account_address, unwrapped_token_account),
            recipient.pair(),
            wrapped_mint_account.pair(),
            unwrapped_token_program,
            wrapped_token_program,
            self.mint_result.unwrapped_mint.pair(),
            (wrapped_mint_authority, Account::default()),
        ];

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, accounts, &self.checks);

        WrapResult {
            unwrapped_token: KeyedAccount {
                key: unwrapped_token_account_address,
                account: result
                    .get_account(&unwrapped_token_account_address)
                    .unwrap()
                    .clone(),
            },
            unwrapped_escrow: KeyedAccount {
                key: unwrapped_escrow_address,
                account: result
                    .get_account(&unwrapped_escrow_address)
                    .unwrap()
                    .clone(),
            },
            wrapped_mint: KeyedAccount {
                key: wrapped_mint_account.key,
                account: result
                    .get_account(&wrapped_mint_account.key)
                    .unwrap()
                    .clone(),
            },
            recipient_wrapped_token: KeyedAccount {
                key: recipient.key,
                account: result.get_account(&recipient.key).unwrap().clone(),
            },
        }
    }
}

pub struct WrapResult {
    pub unwrapped_token: KeyedAccount,
    pub unwrapped_escrow: KeyedAccount,
    pub wrapped_mint: KeyedAccount,
    pub recipient_wrapped_token: KeyedAccount,
}
