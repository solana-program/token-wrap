use {
    crate::helpers::{
        common::{
            setup_counter, setup_multisig, setup_native_token, setup_transfer_hook_account,
            setup_validation_state_account, transfer_fee_mint, unwrapped_mint_with_transfer_hook,
            MINT_SUPPLY,
        },
        create_mint_builder::{CreateMintBuilder, KeyedAccount, TokenProgram},
        unwrap_builder::{UnwrapBuilder, UnwrapResult},
        wrap_builder::TransferAuthority,
    },
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        error::TokenError,
        extension::{
            transfer_fee::{TransferFeeAmount, TransferFeeConfig},
            BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::{PodAccount, PodMint},
    },
    spl_token_wrap::{
        error::TokenWrapError, get_escrow_address, get_wrapped_mint_address,
        get_wrapped_mint_authority,
    },
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
fn test_incorrect_escrow_address() {
    let not_derived_ata = Default::default();

    UnwrapBuilder::default()
        .unwrapped_escrow_account(not_derived_ata)
        .check(Check::err(TokenWrapError::EscrowMismatch.into()))
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

#[test]
fn test_unwrap_with_transfer_hook() {
    let hook_program_id = test_transfer_hook::id();

    // Testing if counter account is incremented via transfer hook
    let counter = setup_counter(hook_program_id);
    let unwrapped_mint = unwrapped_mint_with_transfer_hook(hook_program_id);

    let source_starting_amount = 50_000;
    let recipient_starting_amount = 50_000;
    let escrow_starting_amount = 150_000;
    let unwrap_amount = 12_555;

    // Escrow & unwrapped token account need to have TransferHook extension as well
    let transfer_authority = TransferAuthority {
        keyed_account: Default::default(),
        signers: vec![],
    };
    let recipient_token_account = setup_transfer_hook_account(
        &transfer_authority.keyed_account.key,
        &unwrapped_mint,
        recipient_starting_amount,
    );

    let escrow_account = {
        let wrapped_mint_addr =
            get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
        let mint_authority = get_wrapped_mint_authority(&wrapped_mint_addr);
        KeyedAccount {
            key: get_escrow_address(
                &unwrapped_mint.key,
                &unwrapped_mint.account.owner,
                &spl_token_2022::id(),
            ),
            account: setup_transfer_hook_account(
                &mint_authority,
                &unwrapped_mint,
                escrow_starting_amount,
            ),
        }
    };

    // Validation state account required in order for counter account to be passed
    // in transfer hook
    let validation_state_account =
        setup_validation_state_account(&hook_program_id, &counter, &unwrapped_mint);

    // Execute the unwrap instruction using our UnwrapBuilder.
    let unwrap_result = UnwrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_starting_amount(source_starting_amount)
        .recipient_starting_amount(recipient_starting_amount)
        .recipient_token_account(recipient_token_account)
        .transfer_authority(transfer_authority)
        .escrow_starting_amount(escrow_starting_amount)
        .unwrap_amount(unwrap_amount)
        .unwrapped_mint(unwrapped_mint)
        .unwrapped_escrow_account(escrow_account)
        .add_extra_account(counter)
        .add_extra_account(KeyedAccount {
            key: hook_program_id,
            account: create_program_account_loader_v3(&hook_program_id),
        })
        .add_extra_account(validation_state_account)
        .check(Check::success())
        .execute();

    assert_unwrap_result(
        source_starting_amount,
        recipient_starting_amount,
        escrow_starting_amount,
        unwrap_amount,
        &unwrap_result,
    );

    // Verify counter was incremented
    let count = unwrap_result.extra_accounts[0].clone().account.data[0];
    assert_eq!(count, 1)
}

#[test]
fn test_successfully_unwraps_to_native_mint() {
    let source_starting_amount = 50_000;
    let recipient_starting_amount = 25_000;
    let escrow_starting_amount = 42_000;
    let unwrap_amount = 40_000;

    let transfer_authority = TransferAuthority {
        keyed_account: Default::default(),
        signers: vec![],
    };

    let (native_mint, native_token_account) =
        setup_native_token(recipient_starting_amount, &transfer_authority);

    let wrap_result = UnwrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .unwrapped_mint(native_mint)
        .transfer_authority(transfer_authority)
        .recipient_token_account(native_token_account)
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
fn unwrap_with_transfer_fee() {
    let unwrap_amount = 500_000;
    let unwrapped_mint = transfer_fee_mint();

    let mint = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint.account.data).unwrap();
    let transfer_fee_cfg = mint.get_extension::<TransferFeeConfig>().unwrap();
    let transfer_fee = transfer_fee_cfg
        .calculate_epoch_fee(0, unwrap_amount)
        .unwrap();

    let result = UnwrapBuilder::default()
        .unwrapped_mint(unwrapped_mint.clone())
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_starting_amount(unwrap_amount)
        .escrow_starting_amount(unwrap_amount)
        .unwrap_amount(unwrap_amount)
        .check(Check::success())
        .execute();

    // Source wrapped account burned in full
    let wrapped_src =
        PodStateWithExtensions::<PodAccount>::unpack(&result.wrapped_token_account.account.data)
            .unwrap();
    assert_eq!(u64::from(wrapped_src.base.amount), 0);

    // Recipient received the unwrapped amount minus fee
    let recipient = PodStateWithExtensions::<PodAccount>::unpack(
        &result.recipient_unwrapped_token.account.data,
    )
    .unwrap();
    assert_eq!(
        u64::from(recipient.base.amount),
        unwrap_amount - transfer_fee
    );

    // Recipient withheld amount bumped higher
    let recipient_ext = recipient.get_extension::<TransferFeeAmount>().unwrap();
    assert_eq!(u64::from(recipient_ext.withheld_amount), transfer_fee);

    // Escrow is zeroed out
    let escrow =
        PodStateWithExtensions::<PodAccount>::unpack(&result.unwrapped_escrow.account.data)
            .unwrap();
    assert_eq!(u64::from(escrow.base.amount), 0);
}
