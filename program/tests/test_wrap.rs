use {
    crate::helpers::{
        common::MINT_SUPPLY,
        create_mint_builder::{CreateMintBuilder, KeyedAccount, TokenProgram},
        wrap_builder::{WrapBuilder, WrapResult},
    },
    mollusk_svm::result::Check,
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::PodStateWithExtensions,
        pod::{PodAccount, PodMint},
    },
    spl_token_wrap::error::TokenWrapError,
};

pub mod helpers;

#[test]
fn test_zero_amount_wrap() {
    WrapBuilder::default()
        .wrap_amount(0)
        .check(Check::err(TokenWrapError::ZeroWrapAmount.into()))
        .execute();
}

#[test]
fn test_incorrect_wrapped_mint_address() {
    let mint_result = CreateMintBuilder::default().execute();

    let incorrect_wrapped_mint = KeyedAccount {
        key: Pubkey::new_unique(), // Wrong mint address
        account: mint_result.wrapped_mint.account.clone(),
    };

    WrapBuilder::default()
        .wrapped_mint(incorrect_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_incorrect_wrapped_mint_authority() {
    let incorrect_authority = Pubkey::new_unique();
    WrapBuilder::default()
        .wrapped_mint_authority(incorrect_authority)
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}

#[test]
fn test_incorrect_escrow_owner() {
    let incorrect_escrow_owner = Pubkey::new_unique();
    WrapBuilder::default()
        .unwrapped_escrow_owner(incorrect_escrow_owner)
        .check(Check::err(TokenWrapError::EscrowOwnerMismatch.into()))
        .execute();
}

fn assert_wrap_result(starting_amount: u64, wrap_amount: u64, wrap_result: &WrapResult) {
    // Verify the unwrapped tokens were transferred to escrow
    let escrow_token =
        spl_token::state::Account::unpack(&wrap_result.unwrapped_escrow.account.data).unwrap();
    assert_eq!(escrow_token.amount, wrap_amount);

    // Verify the source account was debited
    let unwrapped_token =
        spl_token::state::Account::unpack(&wrap_result.unwrapped_token.account.data).unwrap();
    assert_eq!(unwrapped_token.amount, 0);

    // Verify wrapped tokens were minted to recipient
    let recipient_token = PodStateWithExtensions::<PodAccount>::unpack(
        &wrap_result.recipient_wrapped_token.account.data,
    )
    .unwrap();
    assert_eq!(
        recipient_token.base.amount,
        starting_amount.checked_add(wrap_amount).unwrap().into()
    );
    assert_eq!(recipient_token.base.mint, wrap_result.wrapped_mint.key);

    // Verify wrapped mint supply increased
    let mint =
        PodStateWithExtensions::<PodMint>::unpack(&wrap_result.wrapped_mint.account.data).unwrap();
    assert_eq!(
        u64::from(mint.base.supply),
        MINT_SUPPLY.checked_add(wrap_amount).unwrap()
    );
}

#[test]
fn test_wrap_amount_exceeds_balance() {
    // Try to wrap more tokens than we have in the account
    let starting_balance = 100;
    let wrap_amount = starting_balance + 1;

    WrapBuilder::default()
        .wrap_amount(wrap_amount)
        .unwrapped_token_starting_amount(starting_balance)
        .check(Check::err(ProgramError::Custom(1)))
        .execute();
}

#[test]
fn test_wrap_with_uninitialized_escrow() {
    // Create an uninitialized escrow account (just empty data)
    let uninitialized_escrow = Account {
        lamports: 100_000_000,
        owner: spl_token::id(),
        data: vec![0; spl_token::state::Account::LEN],
        ..Default::default()
    };

    WrapBuilder::default()
        .unwrapped_escrow_account(uninitialized_escrow)
        .check(Check::err(ProgramError::UninitializedAccount))
        .execute();
}

#[test]
fn test_successful_spl_token_wrap() {
    let starting_amount = 50_000;
    let wrap_amount = 12_555;

    let wrap_result = WrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_successful_spl_token_2022_to_spl_token_wrap() {
    let starting_amount = 64_532;
    let wrap_amount = 7_543;

    let wrap_result = WrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_successful_spl_token_2022_to_token_2022() {
    let starting_amount = 345;
    let wrap_amount = 599;

    let wrap_result = WrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}
