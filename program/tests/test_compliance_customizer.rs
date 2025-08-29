pub mod helpers;

use {
    crate::helpers::create_mint_builder::CreateMintBuilder,
    helpers::common::TokenProgram,
    solana_pubkey::Pubkey,
    solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey,
    spl_pod::primitives::PodBool,
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint,
            default_account_state::DefaultAccountState, pausable::PausableConfig,
            permanent_delegate::PermanentDelegate, BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodMint,
        state::AccountState,
    },
    spl_token_wrap::mint_customizer::compliance::{
        AUDITOR_ELGAMAL_PUBKEY_B64, CONFIDENTIAL_TRANSFER_AUTHORITY, FREEZE_AUTHORITY,
        PAUSE_AUTHORITY, PERMANENT_DELEGATE,
    },
    std::str::FromStr,
};

#[test]
// To test, adjust the processor function:
// process_create_mint::<ComplianceMintCustomizer>(program_id, accounts, idempotent)
#[ignore]
fn test_create_mint_with_compliance_customizer() {
    let result = CreateMintBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .execute();

    assert_eq!(
        result.wrapped_mint.account.owner,
        TokenProgram::SplToken2022.id()
    );

    let mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();

    // Assert base mint data is customized
    assert_eq!(
        mint_state.base.freeze_authority.ok_or(()).unwrap(),
        FREEZE_AUTHORITY
    );
    assert_eq!(mint_state.base.decimals, 12);

    // Assert PermanentDelegate extension
    let perm_delegate_ext = mint_state.get_extension::<PermanentDelegate>().unwrap();
    assert_eq!(
        Option::<Pubkey>::from(perm_delegate_ext.delegate).unwrap(),
        PERMANENT_DELEGATE
    );

    // Assert DefaultAccountState extension
    let default_state_ext = mint_state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state_ext.state, AccountState::Initialized as u8);

    // Assert ConfidentialTransferMint extension
    let ct_ext = mint_state
        .get_extension::<ConfidentialTransferMint>()
        .unwrap();
    assert_eq!(
        Option::<Pubkey>::from(ct_ext.authority).unwrap(),
        CONFIDENTIAL_TRANSFER_AUTHORITY
    );
    assert_eq!(ct_ext.auto_approve_new_accounts, PodBool::from_bool(true));
    assert_eq!(
        Option::<PodElGamalPubkey>::from(ct_ext.auditor_elgamal_pubkey).unwrap(),
        PodElGamalPubkey::from_str(AUDITOR_ELGAMAL_PUBKEY_B64).unwrap()
    );

    // Assert Pausable extension
    let pausable_ext = mint_state.get_extension::<PausableConfig>().unwrap();
    assert_eq!(
        Option::<Pubkey>::from(pausable_ext.authority).unwrap(),
        PAUSE_AUTHORITY
    );
    assert!(!bool::from(pausable_ext.paused));
}
