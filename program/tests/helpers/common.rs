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

use std::fs::OpenOptions;
use std::io::{self, Write};
pub const FILE_PATH: &'static str = "log_file.txt";

/// A utility for logging to a file
pub fn logger(contents: &str, overwrite: bool) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true) // create the file if it doesn't exist
        .append(overwrite) // append or overwrite
        .open(FILE_PATH)?;

    file.write_all(contents.as_bytes())?;
    Ok(())
}
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

fn _token_2022_with_extension_data(supply: u64) -> Vec<u8> {
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
pub fn setup_mint(
    token_program: TokenProgram,
    rent: &Rent,
    mint_authority: Pubkey,
) -> Account {
    let state = spl_token::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        mint_authority: COption::Some(mint_authority),
        freeze_authority: COption::Some(FREEZE_AUTHORITY),
    };
    let mut data = match token_program.clone() {
        TokenProgram::SplToken => vec![0u8; spl_token::state::Mint::LEN],
        TokenProgram::SplToken2022 { extensions} => {
            token_2022_with_extension_data_generic(MINT_SUPPLY, extensions)
        }
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

pub fn setup_native_token(
    balance: u64,
    owner: &TransferAuthority,
    extensions: Vec<ExtensionType>,
) -> (KeyedAccount, Account) {
    let native_mint = KeyedAccount {
        key: spl_token_2022::native_mint::id(),
        account: setup_mint(
            TokenProgram::SplToken2022 { extensions },
            &Rent::default(),
            Pubkey::new_unique(),
        ),
    };

    let account_size =
        ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(&[]).unwrap();
    let mut account_data = vec![0; account_size];
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack_uninitialized(
        &mut account_data,
    )
    .unwrap();

    state.base = spl_token_2022::state::Account {
        mint: native_mint.key,
        amount: balance,
        owner: owner.keyed_account.key,
        state: AccountState::Initialized,
        is_native: COption::Some(20),
        ..Default::default()
    };
    state.pack_base();

    let native_token_account = Account {
        lamports: Rent::default()
            .minimum_balance(spl_token_2022::state::Account::LEN)
            .checked_add(balance)
            .unwrap(),
        data: account_data,
        owner: spl_token_2022::id(),
        ..Default::default()
    };
    (native_mint, native_token_account)
}

// =========================== RUNTIME VERIFICATION ===========================

fn token_2022_with_extension_data_generic(supply: u64, extensions: Vec<ExtensionType>) -> Vec<u8> {
    let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(&extensions).unwrap();
    let mut buffer = vec![0; mint_size];
    let mut state: PodStateWithExtensionsMut<'_, PodMint> =
        PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();
    state.base.decimals = MINT_DECIMALS;
    state.base.is_initialized = PodBool::from_bool(true);
    state.base.supply = PodU64::from(supply);
    state.base.freeze_authority = PodCOption::from(COption::Some(FREEZE_AUTHORITY));
    state.init_account_type().unwrap();

    extensions.iter().for_each(|ext| match ext {
        ExtensionType::Uninitialized => _uninitialized(&mut state),
        ExtensionType::TransferFeeConfig => transfer_fee_config(&mut state),
        ExtensionType::TransferFeeAmount => _transfer_fee_amount(&mut state),
        ExtensionType::MintCloseAuthority => mint_close_authority(&mut state),
        ExtensionType::ConfidentialTransferMint => _confidential_transfer_mint(&mut state),
        ExtensionType::ConfidentialTransferAccount => _confidential_transfer_account(&mut state),
        ExtensionType::DefaultAccountState => _default_account_state(&mut state),
        ExtensionType::ImmutableOwner => _immutable_owner(&mut state),
        ExtensionType::MemoTransfer => _memo_transfer(&mut state),
        ExtensionType::NonTransferable => _non_transferable(&mut state),
        ExtensionType::InterestBearingConfig => _interest_bearing_config(&mut state),
        ExtensionType::CpiGuard => _cpi_guard(&mut state),
        ExtensionType::PermanentDelegate => _permanent_delegate(&mut state),
        ExtensionType::NonTransferableAccount => _non_transferable_account(&mut state),
        ExtensionType::TransferHook => _transfer_hook(&mut state),
        ExtensionType::TransferHookAccount => _transfer_hook_account(&mut state),
        ExtensionType::ConfidentialTransferFeeConfig => {
            _confidential_transfer_fee_config(&mut state)
        }
        ExtensionType::ConfidentialTransferFeeAmount => {
            _confidential_transfer_fee_amount(&mut state)
        }
        ExtensionType::MetadataPointer => _metadata_pointer(&mut state),
        ExtensionType::TokenMetadata => _token_metadata(&mut state),
        ExtensionType::GroupPointer => _group_pointer(&mut state),
        ExtensionType::TokenGroup => _token_group(&mut state),
        ExtensionType::GroupMemberPointer => _group_member_pointer(&mut state),
        ExtensionType::TokenGroupMember => _token_group_member(&mut state),
        ExtensionType::ConfidentialMintBurn => _confidential_mint_burn(&mut state),
        ExtensionType::ScaledUiAmount => _scaled_ui_amount(&mut state),
        ExtensionType::Pausable => _pausable(&mut state),
        ExtensionType::PausableAccount => _pausable_account(&mut state),
    });

    buffer
}

fn _uninitialized(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

/// Initialize TransferFeeConfig extension
fn transfer_fee_config(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let transfer_fee_ext = state.init_extension::<TransferFeeConfig>(false).unwrap();
    let transfer_fee_config_authority = Pubkey::new_unique();
    let withdraw_withheld_authority = Pubkey::new_unique();
    transfer_fee_ext.transfer_fee_config_authority =
        OptionalNonZeroPubkey::try_from(Some(transfer_fee_config_authority)).unwrap();
    transfer_fee_ext.withdraw_withheld_authority =
        OptionalNonZeroPubkey::try_from(Some(withdraw_withheld_authority)).unwrap();
    transfer_fee_ext.withheld_amount = PodU64::from(0);
}

fn _transfer_fee_amount(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

/// Initialize MintCloseAuthority extension
fn mint_close_authority(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let extension = state.init_extension::<MintCloseAuthority>(false).unwrap();
    let close_authority = OptionalNonZeroPubkey::try_from(Some(Pubkey::new_unique())).unwrap();
    extension.close_authority = close_authority;
}

fn _confidential_transfer_mint(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _confidential_transfer_account(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _default_account_state(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _immutable_owner(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _memo_transfer(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _non_transferable(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _interest_bearing_config(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _cpi_guard(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _permanent_delegate(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _non_transferable_account(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _transfer_hook(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _transfer_hook_account(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _confidential_transfer_fee_config(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _confidential_transfer_fee_amount(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _metadata_pointer(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _token_metadata(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _group_pointer(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _token_group(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _group_member_pointer(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _token_group_member(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _confidential_mint_burn(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _scaled_ui_amount(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _pausable(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}

fn _pausable_account(state: &mut PodStateWithExtensionsMut<'_, PodMint>) {
    let _ = state;
    todo!();
}
