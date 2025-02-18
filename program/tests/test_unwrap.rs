use {
    crate::helpers::{
        common::{setup_multisig, MINT_SUPPLY},
        create_mint_builder::{CreateMintBuilder, KeyedAccount, TokenProgram},
        unwrap_builder::{UnwrapBuilder, UnwrapResult},
    },
    mollusk_svm::result::Check,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        error::TokenError,
        extension::PodStateWithExtensions,
        pod::{PodAccount, PodMint},
    },
    spl_token_wrap::error::TokenWrapError,
};

pub mod helpers;

#[test]
fn test_zero_amount_unwrap() {
    UnwrapBuilder::default()
        .unwrap_amount(0)
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

    UnwrapBuilder::default()
        .wrapped_mint(incorrect_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_incorrect_wrapped_mint_authority() {
    let incorrect_authority = Pubkey::new_unique();
    UnwrapBuilder::default()
        .wrapped_mint_authority(incorrect_authority)
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}

#[test]
fn test_unwrap_amount_exceeds_unwrappers_balance() {
    let wrapped_balance = 1_000;
    let unwrap_amount = 42_000;

    UnwrapBuilder::default()
        .wrapped_token_starting_amount(wrapped_balance)
        .unwrap_amount(unwrap_amount)
        .check(Check::err(TokenError::InsufficientFunds.into()))
        .execute();
}

fn assert_unwrap_result(
    source_starting_amount: u64,
    recipient_starting_amount: u64,
    escrow_starting_amount: u64,
    unwrap_amount: u64,
    unwrap_result: &UnwrapResult,
) {
    // Verify wrapped tokens were burned (source account)
    let wrapped_token = PodStateWithExtensions::<PodAccount>::unpack(
        &unwrap_result.wrapped_token_account.account.data,
    )
    .unwrap();
    assert_eq!(
        wrapped_token.base.amount,
        source_starting_amount
            .checked_sub(unwrap_amount)
            .unwrap()
            .into()
    );

    // Verify wrapped mint supply decreased
    let mint = PodStateWithExtensions::<PodMint>::unpack(&unwrap_result.wrapped_mint.account.data)
        .unwrap();
    assert_eq!(
        u64::from(mint.base.supply),
        MINT_SUPPLY.checked_sub(unwrap_amount).unwrap()
    );

    // Verify escrow was debited
    let escrow_token =
        PodStateWithExtensions::<PodAccount>::unpack(&unwrap_result.unwrapped_escrow.account.data)
            .unwrap();
    assert_eq!(
        u64::from(escrow_token.base.amount),
        escrow_starting_amount.checked_sub(unwrap_amount).unwrap()
    );

    // Verify recipient received unwrapped tokens
    let recipient_token = PodStateWithExtensions::<PodAccount>::unpack(
        &unwrap_result.recipient_unwrapped_token.account.data,
    )
    .unwrap();
    assert_eq!(
        u64::from(recipient_token.base.amount),
        recipient_starting_amount
            .checked_add(unwrap_amount)
            .unwrap()
    );
}

#[test]
fn test_successful_spl_token_2022_to_spl_token_unwrap() {
    let source_starting_amount = 50_000;
    let recipient_starting_amount = 50_000;
    let escrow_starting_amount = 150_000;
    let unwrap_amount = 12_555;

    let wrap_result = UnwrapBuilder::default()
        .wrapped_token_starting_amount(source_starting_amount)
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .escrow_starting_amount(escrow_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &wrap_result,
    );
}

#[test]
fn test_successful_spl_token_to_spl_token_2022_unwrap() {
    let source_starting_amount = 50_000;
    let recipient_starting_amount = 25_000;
    let escrow_starting_amount = 42_000;
    let unwrap_amount = 40_000;

    let wrap_result = UnwrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken)
        .escrow_starting_amount(escrow_starting_amount)
        .wrapped_token_starting_amount(source_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &wrap_result,
    );
}

#[test]
fn test_successful_token_2022_to_token_2022_unwrap() {
    let source_starting_amount = 150_000;
    let recipient_starting_amount = 0;
    let escrow_starting_amount = 100_000;
    let unwrap_amount = 100_000;

    let wrap_result = UnwrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .escrow_starting_amount(escrow_starting_amount)
        .wrapped_token_starting_amount(source_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &wrap_result,
    );
}

#[test]
fn test_unwrap_with_spl_token_multisig() {
    let multisig = setup_multisig(TokenProgram::SplToken);

    let source_starting_amount = 100_000;
    let recipient_starting_amount = 0;
    let escrow_starting_amount = 100_000;
    let unwrap_amount = 100_000;

    let wrap_result = UnwrapBuilder::default()
        .transfer_authority(multisig)
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken)
        .escrow_starting_amount(escrow_starting_amount)
        .wrapped_token_starting_amount(source_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &wrap_result,
    );
}

#[test]
fn test_unwrap_with_spl_token_2022_multisig() {
    let multisig = setup_multisig(TokenProgram::SplToken2022);

    let source_starting_amount = 101;
    let recipient_starting_amount = 101;
    let escrow_starting_amount = 202;
    let unwrap_amount = 101;

    let wrap_result = UnwrapBuilder::default()
        .transfer_authority(multisig)
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .escrow_starting_amount(escrow_starting_amount)
        .wrapped_token_starting_amount(source_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &wrap_result,
    );
}
