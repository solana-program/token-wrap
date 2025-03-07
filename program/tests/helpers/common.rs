use {
    crate::helpers::{
        create_mint_builder::{KeyedAccount, TokenProgram},
        wrap_builder::TransferAuthority,
    },
    mollusk_svm::Mollusk,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_pod::{
        optional_keys::OptionalNonZeroPubkey,
        primitives::{PodBool, PodU64},
    },
    spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList},
    spl_token_2022::{
        extension::{
            mint_close_authority::MintCloseAuthority,
            transfer_fee::TransferFeeConfig,
            transfer_hook::{TransferHook, TransferHookAccount},
            BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut,
            StateWithExtensionsMut,
        },
        pod::{PodCOption, PodMint},
        state::{AccountState, Mint},
    },
    spl_transfer_hook_interface::{
        get_extra_account_metas_address, instruction::ExecuteInstruction,
    },
    std::convert::TryFrom,
};

pub fn init_mollusk() -> Mollusk {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);
    mollusk.add_program(
        &test_transfer_hook::id(),
        "test_transfer_hook",
        &mollusk_svm::program::loader_keys::LOADER_V3,
    );
    mollusk
}

pub const MINT_DECIMALS: u8 = 12;
pub const MINT_SUPPLY: u64 = 500_000_000;
pub const FREEZE_AUTHORITY: Pubkey =
    Pubkey::from_str_const("11111115q4EpJaTXAZWpCg3J2zppWGSZ46KXozzo9");

fn token_2022_with_extension_data(supply: u64) -> Vec<u8> {
    let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(&[
        ExtensionType::MintCloseAuthority,
        ExtensionType::TransferFeeConfig,
    ])
    .unwrap();
    let mut buffer = vec![0; mint_size];
    let mut state =
        PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();
    state.base.decimals = MINT_DECIMALS;
    state.base.is_initialized = PodBool::from_bool(true);
    state.base.supply = PodU64::from(supply);
    state.base.freeze_authority = PodCOption::from(COption::Some(FREEZE_AUTHORITY));
    state.init_account_type().unwrap();

    // Initialize MintCloseAuthority extension
    let extension = state.init_extension::<MintCloseAuthority>(false).unwrap();
    let close_authority = OptionalNonZeroPubkey::try_from(Some(Pubkey::new_unique())).unwrap();
    extension.close_authority = close_authority;

    // Initialize TransferFeeConfig extension
    let transfer_fee_ext = state.init_extension::<TransferFeeConfig>(false).unwrap();
    let transfer_fee_config_authority = Pubkey::new_unique();
    let withdraw_withheld_authority = Pubkey::new_unique();
    transfer_fee_ext.transfer_fee_config_authority =
        OptionalNonZeroPubkey::try_from(Some(transfer_fee_config_authority)).unwrap();
    transfer_fee_ext.withdraw_withheld_authority =
        OptionalNonZeroPubkey::try_from(Some(withdraw_withheld_authority)).unwrap();
    transfer_fee_ext.withheld_amount = PodU64::from(0);

    buffer
}

// spl_token and spl_token_2022 are the same account structure except for owner
pub fn setup_mint(token_program: TokenProgram, rent: &Rent, mint_authority: Pubkey) -> Account {
    let state = spl_token::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        mint_authority: COption::Some(mint_authority),
        freeze_authority: COption::Some(FREEZE_AUTHORITY),
    };
    let mut data = match token_program {
        TokenProgram::SplToken => vec![0u8; spl_token::state::Mint::LEN],
        TokenProgram::SplToken2022 => token_2022_with_extension_data(MINT_SUPPLY),
    };
    state.pack_into_slice(&mut data);

    let lamports = rent.minimum_balance(data.len());

    Account {
        lamports,
        data,
        owner: token_program.id(),
        ..Default::default()
    }
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

pub fn unwrapped_mint_with_transfer_hook(hook_program_id: Pubkey) -> KeyedAccount {
    let mint_len =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::TransferHook]).unwrap();
    let mut data = vec![0u8; mint_len];
    let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut data).unwrap();

    let extension = mint.init_extension::<TransferHook>(true).unwrap();
    extension.program_id = OptionalNonZeroPubkey(hook_program_id);

    mint.base.mint_authority = PodCOption::some(Pubkey::new_unique());
    mint.base.decimals = MINT_DECIMALS;
    mint.base.supply = MINT_SUPPLY.into();
    mint.base.freeze_authority = PodCOption::none();
    mint.base.is_initialized = PodBool::from_bool(true);

    mint.init_account_type().unwrap();

    KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            lamports: Rent::default().minimum_balance(Mint::LEN),
            data,
            owner: spl_token_2022::id(),
            ..Default::default()
        },
    }
}

pub fn setup_transfer_hook_account(owner: &Pubkey, mint: &KeyedAccount, amount: u64) -> Account {
    let account_size =
        ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(&[
            ExtensionType::TransferHookAccount,
        ])
        .unwrap();
    let mut account_data = vec![0; account_size];
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack_uninitialized(
        &mut account_data,
    )
    .unwrap();

    let extension = state.init_extension::<TransferHookAccount>(true).unwrap();
    extension.transferring = false.into();

    state.base = spl_token_2022::state::Account {
        mint: mint.key,
        amount,
        owner: *owner,
        state: AccountState::Initialized,
        ..Default::default()
    };
    state.pack_base();
    state.init_account_type().unwrap();

    Account {
        lamports: Rent::default().minimum_balance(Mint::LEN),
        data: account_data,
        owner: spl_token_2022::id(),
        ..Default::default()
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
