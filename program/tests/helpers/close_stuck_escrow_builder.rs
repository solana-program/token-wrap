use {
    crate::helpers::{
        common::{init_mollusk, setup_mint},
        create_mint_builder::{KeyedAccount, TokenProgram},
    },
    mollusk_svm::{result::Check, Mollusk},
    mollusk_svm_programs_token::token2022,
    solana_account::Account,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_token_wrap::{
        get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_authority, instruction,
    },
};

pub struct CloseStuckEscrowBuilder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    escrow_owner: Option<Pubkey>,
    escrow_account: Option<KeyedAccount>,
    destination_account: Option<KeyedAccount>,
    unwrapped_mint: Option<KeyedAccount>,
    wrapped_mint: Option<KeyedAccount>,
    wrapped_mint_authority: Option<Pubkey>,
    wrapped_token_program: Option<TokenProgram>,
    unwrapped_token_program: Option<TokenProgram>,
}

impl Default for CloseStuckEscrowBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: vec![],
            escrow_owner: None,
            escrow_account: None,
            destination_account: None,
            unwrapped_mint: None,
            wrapped_mint: None,
            wrapped_mint_authority: None,
            wrapped_token_program: None,
            unwrapped_token_program: None,
        }
    }
}

impl<'a> CloseStuckEscrowBuilder<'a> {
    pub fn destination_account(mut self, account: KeyedAccount) -> Self {
        self.destination_account = Some(account);
        self
    }

    pub fn escrow_account(mut self, account: KeyedAccount) -> Self {
        self.escrow_account = Some(account);
        self
    }

    pub fn escrow_owner(mut self, owner: Pubkey) -> Self {
        self.escrow_owner = Some(owner);
        self
    }

    pub fn unwrapped_token_program(mut self, program: TokenProgram) -> Self {
        self.unwrapped_token_program = Some(program);
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

    pub fn wrapped_mint_authority(mut self, authority: Pubkey) -> Self {
        self.wrapped_mint_authority = Some(authority);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn execute(mut self) {
        let unwrapped_token_program = self
            .unwrapped_token_program
            .unwrap_or(TokenProgram::SplToken2022);
        let wrapped_token_program = self
            .wrapped_token_program
            .unwrap_or(TokenProgram::SplToken2022);

        let unwrapped_mint = self.unwrapped_mint.unwrap_or_else(|| KeyedAccount {
            key: Pubkey::new_unique(),
            account: setup_mint(
                unwrapped_token_program,
                &self.mollusk.sysvars.rent,
                Pubkey::new_unique(),
            ),
        });

        let wrapped_mint = self.wrapped_mint.unwrap_or_else(|| KeyedAccount {
            key: get_wrapped_mint_address(&unwrapped_mint.key, &wrapped_token_program.id()),
            account: setup_mint(
                wrapped_token_program,
                &self.mollusk.sysvars.rent,
                Pubkey::new_unique(),
            ),
        });

        let wrapped_mint_authority = self
            .wrapped_mint_authority
            .unwrap_or_else(|| get_wrapped_mint_authority(&wrapped_mint.key));

        let destination_account = self.destination_account.unwrap_or_else(|| KeyedAccount {
            key: Pubkey::new_unique(),
            account: Account::default(),
        });

        let escrow_address = get_escrow_address(
            &unwrapped_mint.key,
            &unwrapped_token_program.id(),
            &wrapped_token_program.id(),
        );

        let escrow_account = self.escrow_account.unwrap_or_else(|| {
            let owner = self.escrow_owner.unwrap_or(spl_token_2022::id());
            let len = spl_token_2022::state::Account::LEN;
            KeyedAccount {
                key: escrow_address,
                account: Account {
                    owner,
                    lamports: Rent::default().minimum_balance(len),
                    data: vec![0; len],
                    ..Default::default()
                },
            }
        });

        let instruction = instruction::close_stuck_escrow(
            &spl_token_wrap::id(),
            &escrow_account.key,
            &destination_account.key,
            &unwrapped_mint.key,
            &wrapped_mint.key,
            &wrapped_mint_authority,
        );

        let accounts = &[
            escrow_account.pair(),
            destination_account.pair(),
            unwrapped_mint.pair(),
            wrapped_mint.pair(),
            (wrapped_mint_authority, Account::default()),
            token2022::keyed_account(),
        ];

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        self.mollusk
            .process_and_validate_instruction(&instruction, accounts, &self.checks);
    }
}
