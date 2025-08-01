use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        mint_builder::MintBuilder,
    },
    mollusk_svm::{program::keyed_account_for_system_program, result::Check, Mollusk},
    solana_account::Account,
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority, get_wrapped_mint_backpointer_address,
        instruction::create_mint,
    },
};

pub struct CreateMintResult {
    pub unwrapped_mint: KeyedAccount,
    pub wrapped_mint: KeyedAccount,
    pub wrapped_backpointer: KeyedAccount,
}

pub struct CreateMintBuilder<'a> {
    mollusk: Mollusk,
    wrapped_token_program: TokenProgram,
    wrapped_token_program_addr: Option<Pubkey>,
    unwrapped_mint_addr: Option<Pubkey>,
    unwrapped_mint_account: Option<Account>,
    unwrapped_token_program: TokenProgram,
    wrapped_mint_addr: Option<Pubkey>,
    wrapped_mint_account: Option<Account>,
    wrapped_mint_authority_addr: Option<Pubkey>,
    backpointer_addr: Option<Pubkey>,
    backpointer_account: Option<Account>,
    freeze_authority: Option<Pubkey>,
    idempotent: bool,
    checks: Vec<Check<'a>>,
}

impl Default for CreateMintBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            wrapped_token_program: TokenProgram::SplToken2022,
            wrapped_token_program_addr: None,
            unwrapped_mint_addr: None,
            unwrapped_mint_account: None,
            unwrapped_token_program: TokenProgram::SplToken,
            wrapped_mint_addr: None,
            wrapped_mint_account: None,
            wrapped_mint_authority_addr: None,
            backpointer_addr: None,
            backpointer_account: None,
            freeze_authority: None,
            idempotent: false,
            checks: vec![],
        }
    }
}

impl<'a> CreateMintBuilder<'a> {
    pub fn wrapped_token_program(mut self, program: TokenProgram) -> Self {
        self.wrapped_token_program = program;
        self
    }

    pub fn unwrapped_token_program(mut self, program: TokenProgram) -> Self {
        self.unwrapped_token_program = program;
        self
    }

    pub fn token_program_addr(mut self, key: Pubkey) -> Self {
        self.wrapped_token_program_addr = Some(key);
        self
    }

    pub fn unwrapped_mint_addr(mut self, key: Pubkey) -> Self {
        self.unwrapped_mint_addr = Some(key);
        self
    }

    pub fn wrapped_mint_addr(mut self, key: Pubkey) -> Self {
        self.wrapped_mint_addr = Some(key);
        self
    }

    pub fn wrapped_mint_account(mut self, account: Account) -> Self {
        self.wrapped_mint_account = Some(account);
        self
    }

    pub fn wrapped_mint_authority_addr(mut self, key: Pubkey) -> Self {
        self.wrapped_mint_authority_addr = Some(key);
        self
    }

    pub fn backpointer_addr(mut self, key: Pubkey) -> Self {
        self.backpointer_addr = Some(key);
        self
    }
    pub fn backpointer_account(mut self, account: Account) -> Self {
        self.backpointer_account = Some(account);
        self
    }

    pub fn idempotent(mut self) -> Self {
        self.idempotent = true;
        self
    }

    pub fn unwrapped_mint_account(mut self, account: Account) -> Self {
        self.unwrapped_mint_account = Some(account);
        self
    }

    pub fn freeze_authority(mut self, authority: Pubkey) -> Self {
        self.freeze_authority = Some(authority);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn execute(mut self) -> CreateMintResult {
        let unwrapped_mint_addr = self.unwrapped_mint_addr.unwrap_or_else(Pubkey::new_unique);
        let wrapped_token_program_id = self
            .wrapped_token_program_addr
            .unwrap_or_else(|| self.wrapped_token_program.id());

        let unwrapped_mint_account = self.unwrapped_mint_account.clone().unwrap_or_else(|| {
            let mut mint_builder = MintBuilder::new()
                .token_program(self.unwrapped_token_program)
                .mint_authority(Pubkey::new_unique());

            if let Some(freeze_auth) = self.freeze_authority {
                mint_builder = mint_builder.freeze_authority(freeze_auth);
            }

            mint_builder.build().account
        });

        let wrapped_mint_addr = self.wrapped_mint_addr.unwrap_or_else(|| {
            get_wrapped_mint_address(&unwrapped_mint_addr, &wrapped_token_program_id)
        });

        let wrapped_backpointer_address = self
            .backpointer_addr
            .unwrap_or_else(|| get_wrapped_mint_backpointer_address(&wrapped_mint_addr));

        let wrapped_mint_account = self.wrapped_mint_account.unwrap_or(Account {
            lamports: 100_000_000,
            ..Default::default()
        });

        let wrapped_backpointer_account = self.backpointer_account.unwrap_or(Account {
            lamports: 100_000_000,
            ..Default::default()
        });

        let wrapped_mint_authority_address = self
            .wrapped_mint_authority_addr
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint_addr));

        let instruction = create_mint(
            &spl_token_wrap::id(),
            &wrapped_mint_addr,
            &wrapped_backpointer_address,
            &unwrapped_mint_addr,
            &wrapped_mint_authority_address,
            &wrapped_token_program_id,
            self.idempotent,
        );

        let mut keyed_token_program = match self.wrapped_token_program {
            TokenProgram::SplToken => mollusk_svm_programs_token::token::keyed_account(),
            TokenProgram::SplToken2022 => mollusk_svm_programs_token::token2022::keyed_account(),
        };
        keyed_token_program.0 = wrapped_token_program_id;

        let accounts = &[
            (wrapped_mint_addr, wrapped_mint_account),
            (wrapped_backpointer_address, wrapped_backpointer_account),
            (unwrapped_mint_addr, unwrapped_mint_account),
            (wrapped_mint_authority_address, Account::default()),
            keyed_account_for_system_program(),
            keyed_token_program,
        ];

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, accounts, &self.checks);

        CreateMintResult {
            unwrapped_mint: KeyedAccount {
                key: unwrapped_mint_addr,
                account: result.get_account(&unwrapped_mint_addr).unwrap().clone(),
            },
            wrapped_mint: KeyedAccount {
                key: wrapped_mint_addr,
                account: result.get_account(&wrapped_mint_addr).unwrap().clone(),
            },
            wrapped_backpointer: KeyedAccount {
                key: wrapped_backpointer_address,
                account: result
                    .get_account(&wrapped_backpointer_address)
                    .unwrap()
                    .clone(),
            },
        }
    }
}
