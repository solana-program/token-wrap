use {
    mollusk_svm::Mollusk,
    mollusk_svm_programs_token,
    solana_account::Account,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList},
    spl_transfer_hook_interface::{
        get_extra_account_metas_address, instruction::ExecuteInstruction,
    },
};

pub const DEFAULT_MINT_DECIMALS: u8 = 12;
pub const DEFAULT_MINT_SUPPLY: u64 = 500_000_000;

#[derive(Default, Debug, Clone)]
pub struct KeyedAccount {
    pub key: Pubkey,
    pub account: Account,
}

impl KeyedAccount {
    pub fn pair(&self) -> (Pubkey, Account) {
        (self.key, self.account.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenProgram {
    SplToken,
    SplToken2022,
}

impl TokenProgram {
    pub fn id(&self) -> Pubkey {
        match self {
            TokenProgram::SplToken => spl_token::id(),
            TokenProgram::SplToken2022 => spl_token_2022::id(),
        }
    }

    pub fn keyed_account(&self) -> (Pubkey, Account) {
        match self {
            TokenProgram::SplToken => mollusk_svm_programs_token::token::keyed_account(),
            TokenProgram::SplToken2022 => mollusk_svm_programs_token::token2022::keyed_account(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransferAuthority {
    pub keyed_account: KeyedAccount,
    pub signers: Vec<Pubkey>,
}

pub fn init_mollusk() -> Mollusk {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);
    mollusk_svm_programs_token::associated_token::add_program(&mut mollusk);
    mollusk.add_program(
        &test_transfer_hook::id(),
        "test_transfer_hook",
        &mollusk_svm::program::loader_keys::LOADER_V3,
    );
    mollusk
}

pub fn setup_multisig(program: TokenProgram) -> TransferAuthority {
    let multisig_key = Pubkey::new_unique();
    let signer0_key = Pubkey::new_unique();
    let signer1_key = Pubkey::new_unique();
    let signer2_key = Pubkey::new_unique();

    let mut multisig_account = Account {
        lamports: 100_000_000,
        owner: program.id(),
        data: vec![0; spl_token_2022::state::Multisig::LEN],
        ..Account::default()
    };

    let multisig_state = spl_token_2022::state::Multisig {
        m: 2,
        n: 3,
        is_initialized: true,
        signers: [
            signer0_key,
            signer1_key,
            signer2_key,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        ],
    };
    spl_token_2022::state::Multisig::pack(multisig_state, &mut multisig_account.data).unwrap();
    TransferAuthority {
        keyed_account: KeyedAccount {
            key: multisig_key,
            account: multisig_account,
        },
        signers: vec![signer0_key, signer1_key, signer2_key],
    }
}

pub fn setup_counter(hook_program_id: Pubkey) -> KeyedAccount {
    let account = Account {
        lamports: Rent::default().minimum_balance(1),
        owner: hook_program_id,
        data: vec![0],
        executable: false,
        rent_epoch: 0,
    };
    KeyedAccount {
        key: Pubkey::new_unique(),
        account,
    }
}

pub fn setup_validation_state_account(
    hook_program_id: &Pubkey,
    counter: &KeyedAccount,
    unwrapped_mint: &KeyedAccount,
) -> KeyedAccount {
    let validation_state_pubkey =
        get_extra_account_metas_address(&unwrapped_mint.key, hook_program_id);
    let extra_account_metas =
        vec![ExtraAccountMeta::new_with_pubkey(&counter.key, false, true).unwrap()];
    let account_size = ExtraAccountMetaList::size_of(extra_account_metas.len()).unwrap();
    let mut validation_data = vec![0; account_size];
    ExtraAccountMetaList::init::<ExecuteInstruction>(&mut validation_data, &extra_account_metas)
        .unwrap();

    KeyedAccount {
        key: validation_state_pubkey,
        account: Account {
            lamports: Rent::default().minimum_balance(account_size),
            data: validation_data,
            owner: *hook_program_id,
            executable: false,
            rent_epoch: 0,
        },
    }
}
