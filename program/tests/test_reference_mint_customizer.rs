use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram, DEFAULT_MINT_DECIMALS},
        mint_builder::MintBuilder,
    },
    mollusk_svm::result::Check,
    solana_account::Account,
    solana_account_info::AccountInfo,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint,
            default_account_state::DefaultAccountState, permanent_delegate::PermanentDelegate,
            BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodMint,
        state::AccountState,
    },
    spl_token_wrap::{
        get_wrapped_mint_authority,
        mint_customizer::{
            compliance::{
                ComplianceMintCustomizer, AUDITOR_ELGAMAL_PUBKEY, PERMANENT_DELEGATE_ADDRESS,
            },
            interface::MintCustomizer,
        },
    },
};

pub mod helpers;

#[test]
fn test_compliance_customizer_space_calculation() {
    // Test that the space calculation is correct.
    let expected_space = ComplianceMintCustomizer::get_token_2022_mint_space().unwrap();
    let calculated_space = spl_token_2022::extension::ExtensionType::try_calculate_account_len::<
        spl_token_2022::state::Mint,
    >(&[
        spl_token_2022::extension::ExtensionType::PermanentDelegate,
        spl_token_2022::extension::ExtensionType::DefaultAccountState,
        spl_token_2022::extension::ExtensionType::ConfidentialTransferMint,
    ])
    .unwrap();
    assert_eq!(expected_space, calculated_space);
}

#[test]
fn test_compliance_customizer_get_auth_and_decimals() {
    // Test that the freeze authority and decimals are set correctly.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    // Build a temporary AccountInfo view over the unwrapped mint data
    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let ai = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        data.as_mut_slice(),
        &unwrapped_mint.account.owner,
        unwrapped_mint.account.executable,
        unwrapped_mint.account.rent_epoch,
    );
    let (freeze_authority, decimals) =
        ComplianceMintCustomizer::get_freeze_auth_and_decimals(&ai).unwrap();

    let expected_authority = get_wrapped_mint_authority(&unwrapped_mint.key);
    assert_eq!(freeze_authority, Some(expected_authority));
    assert_eq!(decimals, DEFAULT_MINT_DECIMALS);
}

#[test]
fn test_compliance_customizer_initialize_extensions() {
    // Test that all extensions are initialized correctly via CPI.
    let mollusk = init_mollusk();
    let wrapped_token_program = TokenProgram::SplToken2022;

    let mut wrapped_mint = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            lamports: 1_000_000_000, // Must be rent-exempt
            data: vec![0; ComplianceMintCustomizer::get_token_2022_mint_space().unwrap()],
            owner: wrapped_token_program.id(),
            ..Default::default()
        },
    };

    // Initialize the extensions by invoking Token-2022 instructions directly.
    // 1) Permanent Delegate
    let inst = spl_token_2022::instruction::initialize_permanent_delegate(
        &wrapped_token_program.id(),
        &wrapped_mint.key,
        &PERMANENT_DELEGATE_ADDRESS,
    )
    .unwrap();
    let result = mollusk.process_and_validate_instruction(
        &inst,
        &[wrapped_mint.pair()],
        &[Check::success()],
    );
    let updated = result.get_account(&wrapped_mint.key).unwrap().clone();
    wrapped_mint.account = updated;

    // 2) Default Account State: Frozen
    let inst = spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(
        &wrapped_token_program.id(),
        &wrapped_mint.key,
        &AccountState::Frozen,
    )
    .unwrap();
    let result = mollusk.process_and_validate_instruction(
        &inst,
        &[wrapped_mint.pair()],
        &[Check::success()],
    );
    let updated = result.get_account(&wrapped_mint.key).unwrap().clone();
    wrapped_mint.account = updated;

    // 3) Confidential Transfer Mint with auditor + auto-approve
    let inst = spl_token_2022::extension::confidential_transfer::instruction::initialize_mint(
        &wrapped_token_program.id(),
        &wrapped_mint.key,
        Some(get_wrapped_mint_authority(&wrapped_mint.key)),
        true,
        Some(AUDITOR_ELGAMAL_PUBKEY.to_bytes().into()),
    )
    .unwrap();
    let result = mollusk.process_and_validate_instruction(
        &inst,
        &[wrapped_mint.pair()],
        &[Check::success()],
    );

    // Assert the state of the mint account after the function call.
    let final_mint_account = result.get_account(&wrapped_mint.key).unwrap();
    let mut data = final_mint_account.data.clone();
    let wrapped_mint_state =
        spl_token_2022::extension::PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(
            &mut data,
        )
        .unwrap();

    // Verify Permanent Delegate extension
    let permanent_delegate_ext = wrapped_mint_state
        .get_extension::<PermanentDelegate>()
        .unwrap();
    assert_eq!(
        Option::<Pubkey>::from(permanent_delegate_ext.delegate).unwrap(),
        PERMANENT_DELEGATE_ADDRESS
    );

    // Verify Default Account State extension is Frozen
    let default_account_state_ext = wrapped_mint_state
        .get_extension::<DefaultAccountState>()
        .unwrap();
    assert_eq!(
        AccountState::try_from(default_account_state_ext.state).unwrap(),
        AccountState::Frozen
    );

    // Verify Confidential Transfer Mint extension with auditor
    let ct_mint_ext = wrapped_mint_state
        .get_extension::<ConfidentialTransferMint>()
        .unwrap();
    let expected_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    assert_ne!(ct_mint_ext.auditor_elgamal_pubkey, Default::default());
    assert_eq!(
        Option::<Pubkey>::from(ct_mint_ext.authority).unwrap(),
        expected_authority
    );
    assert!(bool::from(ct_mint_ext.auto_approve_new_accounts));
}
