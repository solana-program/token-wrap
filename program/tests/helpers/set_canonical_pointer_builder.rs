use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        mint_builder::MintBuilder,
    },
    mollusk_svm::{program::keyed_account_for_system_program, result::Check, Mollusk},
    solana_account::Account,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_token_wrap::get_canonical_pointer_address,
};

pub struct SetCanonicalPointerResult {
    pub canonical_pointer: KeyedAccount,
}

pub struct SetCanonicalPointerBuilder<'a> {
    mollusk: Mollusk,
    checks: Vec<Check<'a>>,
    unwrapped_mint_authority: Option<Pubkey>,
    is_authority_signer: bool,
    canonical_pointer: Option<KeyedAccount>,
    unwrapped_mint: Option<KeyedAccount>,
    new_program_id: Option<Pubkey>,
}

impl Default for SetCanonicalPointerBuilder<'_> {
    fn default() -> Self {
        Self {
            mollusk: init_mollusk(),
            checks: vec![],
            unwrapped_mint_authority: None,
            is_authority_signer: true,
            canonical_pointer: None,
            unwrapped_mint: None,
            new_program_id: None,
        }
    }
}

impl<'a> SetCanonicalPointerBuilder<'a> {
    pub fn unwrapped_mint_authority(mut self, key: Pubkey) -> Self {
        self.unwrapped_mint_authority = Some(key);
        self
    }

    pub fn authority_not_signer(mut self) -> Self {
        self.is_authority_signer = false;
        self
    }

    pub fn canonical_pointer(mut self, account: KeyedAccount) -> Self {
        self.canonical_pointer = Some(account);
        self
    }

    pub fn unwrapped_mint(mut self, account: KeyedAccount) -> Self {
        self.unwrapped_mint = Some(account);
        self
    }

    pub fn new_program_id(mut self, program_id: Pubkey) -> Self {
        self.new_program_id = Some(program_id);
        self
    }

    pub fn check(mut self, check: Check<'a>) -> Self {
        self.checks.push(check);
        self
    }

    pub fn execute(mut self) -> SetCanonicalPointerResult {
        let unwrapped_mint_authority_key = self
            .unwrapped_mint_authority
            .unwrap_or_else(Pubkey::new_unique);

        let unwrapped_mint = self.unwrapped_mint.unwrap_or_else(|| {
            MintBuilder::new()
                .token_program(TokenProgram::SplToken)
                .mint_authority(unwrapped_mint_authority_key)
                .build()
        });

        let expected_pointer_address = get_canonical_pointer_address(&unwrapped_mint.key);

        let canonical_pointer = self.canonical_pointer.unwrap_or_else(|| KeyedAccount {
            key: expected_pointer_address,
            account: Account {
                lamports: Rent::default().minimum_balance(std::mem::size_of::<
                    spl_token_wrap::state::CanonicalDeploymentPointer,
                >()),
                ..Default::default()
            },
        });

        let new_program_id = self.new_program_id.unwrap_or_else(Pubkey::new_unique);

        let unwrapped_mint_authority = KeyedAccount {
            key: unwrapped_mint_authority_key,
            account: Account::default(),
        };

        let mut instruction = spl_token_wrap::instruction::set_canonical_pointer(
            &spl_token_wrap::id(),
            &unwrapped_mint_authority.key,
            &canonical_pointer.key,
            &unwrapped_mint.key,
            &new_program_id,
        );

        // Allow testing with non-signer authority for negative test cases
        if !self.is_authority_signer {
            instruction.accounts[0].is_signer = false;
        }

        let accounts = &[
            unwrapped_mint_authority.pair(),
            canonical_pointer.pair(),
            unwrapped_mint.pair(),
            keyed_account_for_system_program(),
        ];

        if self.checks.is_empty() {
            self.checks.push(Check::success());
        }

        let result =
            self.mollusk
                .process_and_validate_instruction(&instruction, accounts, &self.checks);

        SetCanonicalPointerResult {
            canonical_pointer: KeyedAccount {
                key: canonical_pointer.key,
                account: result.get_account(&canonical_pointer.key).unwrap().clone(),
            },
        }
    }
}
