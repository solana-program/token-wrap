use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_program::system_program;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use spl_pod::optional_keys::OptionalNonZeroPubkey;
use spl_pod::primitives::{PodBool, PodU64};
use spl_token_2022::extension::mint_close_authority::MintCloseAuthority;
use spl_token_2022::extension::{
    BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut,
};
use spl_token_2022::pod::{PodCOption, PodMint};
use spl_token_wrap::{
    get_wrapped_mint_address, get_wrapped_mint_backpointer_address, instruction::create_mint,
};
use std::convert::TryFrom;

pub const MINT_DECIMALS: u8 = 12;
pub const MINT_SUPPLY: u64 = 500_000_000;
pub const FREEZE_AUTHORITY: Pubkey =
    Pubkey::from_str_const("11111115q4EpJaTXAZWpCg3J2zppWGSZ46KXozzo9");

pub struct CreateMintResult {
    pub unwrapped_mint: KeyedAccount,
    pub wrapped_mint: KeyedAccount,
    pub wrapped_backpointer: KeyedAccount,
}

#[derive(Debug, Clone)]
pub struct KeyedAccount {
    pub key: Pubkey,
    pub account: Account,
}

impl KeyedAccount {
    pub fn pair(&self) -> (Pubkey, Account) {
        (self.key, self.account.clone())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokenProgram {
    SplToken,
    Token2022,
}

impl TokenProgram {
    pub fn id(&self) -> Pubkey {
        match self {
            TokenProgram::SplToken => spl_token::id(),
            TokenProgram::Token2022 => spl_token_2022::id(),
        }
    }
}

pub struct MintBuilder<'a> {
    mollusk: &'a mut Mollusk,
    wrapped_token_program: TokenProgram,
    wrapped_token_program_addr: Option<Pubkey>,
    unwrapped_mint_addr: Option<Pubkey>,
    unwrapped_mint_account: Option<Account>,
    unwrapped_token_program: TokenProgram,
    wrapped_mint_addr: Option<Pubkey>,
    wrapped_mint_account: Option<Account>,
    backpointer_addr: Option<Pubkey>,
    backpointer_account: Option<Account>,
    idempotent: bool,
    checks: Vec<Check<'a>>,
}

impl<'a> MintBuilder<'a> {
    pub fn new(mollusk: &'a mut Mollusk) -> Self {
        Self {
            mollusk,
            wrapped_token_program: TokenProgram::Token2022,
            wrapped_token_program_addr: None,
            unwrapped_mint_addr: None,
            unwrapped_mint_account: None,
            unwrapped_token_program: TokenProgram::SplToken,
            wrapped_mint_addr: None,
            wrapped_mint_account: None,
            backpointer_addr: None,
            backpointer_account: None,
            idempotent: false,
            checks: vec![],
        }
    }

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

    pub fn unwrapped_mint_account(mut self, account: Account) -> Self {
        self.unwrapped_mint_account = Some(account);
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

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    fn token_2022_with_extension_data(&self) -> Vec<u8> {
        let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(&[
            ExtensionType::MintCloseAuthority,
        ])
        .unwrap();
        let mut buffer = vec![0; mint_size];
        let mut state =
            PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();
        state.base.decimals = MINT_DECIMALS;
        state.base.is_initialized = PodBool::from_bool(true);
        state.base.supply = PodU64::from(MINT_SUPPLY);
        state.base.freeze_authority = PodCOption::from(COption::Some(FREEZE_AUTHORITY));
        state.init_account_type().unwrap();

        let extension = state.init_extension::<MintCloseAuthority>(true).unwrap();
        let close_authority =
            OptionalNonZeroPubkey::try_from(Some(Pubkey::new_from_array([1; 32]))).unwrap();
        extension.close_authority = close_authority;

        buffer
    }

    // Spl_token and token_2022 are the same account structure except for owner
    fn setup_mint(&self, rent: &Rent) -> Account {
        let state = spl_token::state::Mint {
            decimals: MINT_DECIMALS,
            is_initialized: true,
            supply: MINT_SUPPLY,
            freeze_authority: COption::Some(FREEZE_AUTHORITY),
            ..Default::default()
        };
        let mut data = match self.unwrapped_token_program {
            TokenProgram::SplToken => vec![0u8; spl_token::state::Mint::LEN],
            TokenProgram::Token2022 => self.token_2022_with_extension_data(),
        };
        state.pack_into_slice(&mut data);

        let lamports = rent.minimum_balance(data.len());

        Account {
            lamports,
            data,
            owner: self.unwrapped_token_program.id(),
            ..Default::default()
        }
    }

    pub fn execute(mut self) -> CreateMintResult {
        let unwrapped_mint_addr = self.unwrapped_mint_addr.unwrap_or_else(Pubkey::new_unique);
        let wrapped_token_program_id = self
            .wrapped_token_program_addr
            .unwrap_or_else(|| self.wrapped_token_program.id());

        let unwrapped_mint_account = self
            .unwrapped_mint_account
            .clone()
            .unwrap_or_else(|| self.setup_mint(&self.mollusk.sysvars.rent));

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

        let instruction = create_mint(
            &spl_token_wrap::id(),
            &wrapped_mint_addr,
            &wrapped_backpointer_address,
            &unwrapped_mint_addr,
            &wrapped_token_program_id,
            self.idempotent,
        );

        let mut keyed_token_program = match self.wrapped_token_program {
            TokenProgram::SplToken => mollusk_svm_programs_token::token::keyed_account(),
            TokenProgram::Token2022 => mollusk_svm_programs_token::token2022::keyed_account(),
        };
        keyed_token_program.0 = wrapped_token_program_id;

        let accounts = &[
            (wrapped_mint_addr, wrapped_mint_account),
            (wrapped_backpointer_address, wrapped_backpointer_account),
            (unwrapped_mint_addr, unwrapped_mint_account),
            (
                system_program::id(),
                Account {
                    executable: true,
                    ..Default::default()
                },
            ),
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
