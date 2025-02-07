use crate::helpers::mint_builder::{CreateMintResult, KeyedAccount};
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_wrap::instruction::wrap;
use spl_token_wrap::{get_escrow_address, get_wrapped_mint_authority};

pub struct WrapBuilder<'a> {
    mollusk: &'a mut Mollusk,
    mint_result: CreateMintResult,
    wrap_amount: Option<u64>,
    recipient: Option<KeyedAccount>,
    checks: Vec<Check<'a>>,
}

impl<'a> WrapBuilder<'a> {
    pub fn new(mollusk: &'a mut Mollusk, mint_result: CreateMintResult) -> Self {
        Self {
            mollusk,
            mint_result,
            wrap_amount: None,
            recipient: None,
            checks: vec![],
        }
    }

    pub fn wrap_amount(mut self, amount: u64) -> Self {
        self.wrap_amount = Some(amount);
        self
    }

    pub fn recipient(mut self, pair: KeyedAccount) -> Self {
        self.recipient = Some(pair);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn setup_token_account(wrapped_mint_addr: Pubkey, starting_amount: u64) -> KeyedAccount {
        let recipient_addr = Pubkey::new_unique();
        let mut recipient_token_account = Account {
            lamports: 100_000_000,
            owner: spl_token_2022::id(),
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };
        let recipient_account_data = spl_token::state::Account {
            mint: wrapped_mint_addr,
            owner: recipient_addr,
            amount: starting_amount,
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

        let mut unwrapped_token_account = Account {
            lamports: 100_000_000,
            owner: spl_token::id(),
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };

        let wrap_amount = self.wrap_amount.unwrap_or(500);

        let token = spl_token::state::Account {
            mint: self.mint_result.unwrapped_mint.key,
            owner: unwrapped_token_account_authority,
            amount: wrap_amount,
            delegate: None.into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        spl_token::state::Account::pack(token, &mut unwrapped_token_account.data).unwrap();

        let unwrapped_escrow_address = get_escrow_address(
            &unwrapped_token_account_authority,
            &self.mint_result.unwrapped_mint.key,
        );
        let wrapped_mint_authority = get_wrapped_mint_authority(&self.mint_result.wrapped_mint.key);

        let mut unwrapped_escrow_account = Account {
            lamports: 100_000_000,
            owner: spl_token::id(),
            data: vec![0; spl_token::state::Account::LEN],
            ..Default::default()
        };
        let escrow_token = spl_token::state::Account {
            mint: self.mint_result.unwrapped_mint.key,
            owner: wrapped_mint_authority,
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
            .unwrap_or_else(|| Self::setup_token_account(self.mint_result.wrapped_mint.key, 0));

        let instruction = wrap(
            &spl_token_wrap::id(),
            &unwrapped_token_account_authority,
            &unwrapped_escrow_address,
            &unwrapped_token_account_address,
            &recipient.key,
            &self.mint_result.wrapped_mint.key,
            &spl_token::id(),
            &spl_token_2022::id(),
            &self.mint_result.unwrapped_mint.key,
            &wrapped_mint_authority,
            &[],
            wrap_amount,
        );

        let accounts = &[
            (unwrapped_token_account_authority, Account::default()),
            (unwrapped_escrow_address, unwrapped_escrow_account),
            (unwrapped_token_account_address, unwrapped_token_account),
            recipient.pair(),
            self.mint_result.wrapped_mint.pair(),
            mollusk_svm_programs_token::token::keyed_account(),
            mollusk_svm_programs_token::token2022::keyed_account(),
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
                key: self.mint_result.wrapped_mint.key,
                account: result
                    .get_account(&self.mint_result.wrapped_mint.key)
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
