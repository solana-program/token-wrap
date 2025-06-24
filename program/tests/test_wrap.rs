use {
    crate::helpers::{
        common::{
            setup_counter, setup_multisig, setup_validation_state_account, KeyedAccount,
            TokenProgram, DEFAULT_MINT_SUPPLY,
        },
        create_mint_builder::CreateMintBuilder,
        mint_builder::MintBuilder,
        token_account_builder::TokenAccountBuilder,
        wrap_builder::{WrapBuilder, WrapResult},
    },
    helpers::common::TransferAuthority,
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    solana_account::{Account, ReadableAccount},
    solana_program_error::ProgramError,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            transfer_fee::{TransferFeeAmount, TransferFeeConfig},
            BaseStateWithExtensions,
            ExtensionType::{
                ImmutableOwner, TransferFeeConfig as TransferFeeConfigExt, TransferHook,
                TransferHookAccount,
            },
            PodStateWithExtensions,
        },
        pod::{PodAccount, PodMint},
    },
    spl_token_wrap::{error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority},
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
fn test_incorrect_escrow_address() {
    let not_derived_ata = Pubkey::new_unique();

    WrapBuilder::default()
        .unwrapped_escrow_addr(not_derived_ata)
        .check(Check::err(TokenWrapError::EscrowMismatch.into()))
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
        PodStateWithExtensions::<PodAccount>::unpack(&wrap_result.unwrapped_escrow.account.data)
            .unwrap();
    assert_eq!(u64::from(escrow_token.base.amount), wrap_amount);

    // Verify the source account was debited
    let unwrapped_token =
        PodStateWithExtensions::<PodAccount>::unpack(&wrap_result.unwrapped_token.account.data)
            .unwrap();
    assert_eq!(u64::from(unwrapped_token.base.amount), 0);

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
        DEFAULT_MINT_SUPPLY.checked_add(wrap_amount).unwrap()
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

#[test]
fn test_wrap_with_spl_token_multisig() {
    let starting_amount = 500_000;
    let wrap_amount = 8_000;
    let multisig = setup_multisig(TokenProgram::SplToken);

    let wrap_result = WrapBuilder::default()
        .transfer_authority(multisig)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_wrap_with_token_2022_multisig() {
    let starting_amount = 10_000;
    let wrap_amount = 252;
    let multisig = setup_multisig(TokenProgram::SplToken2022);

    let wrap_result = WrapBuilder::default()
        .transfer_authority(multisig)
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_wrap_with_transfer_hook() {
    let hook_program_id = test_transfer_hook::id();

    // Testing if counter account is incremented via transfer hook
    let counter = setup_counter(hook_program_id);
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(TransferHook)
        .build();

    // Escrow & unwrapped token account need to have TransferHook extension as well
    let wrap_amount = 12_555;
    let transfer_authority = TransferAuthority {
        keyed_account: Default::default(),
        signers: vec![],
    };
    let unwrapped_token_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(unwrapped_mint.clone())
        .owner(transfer_authority.keyed_account.key)
        .amount(wrap_amount)
        .with_extension(TransferHookAccount)
        .build();

    let escrow_account = {
        let wrapped_mint_addr =
            get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
        let mint_authority = get_wrapped_mint_authority(&wrapped_mint_addr);
        TokenAccountBuilder::new()
            .token_program(TokenProgram::SplToken2022)
            .mint(unwrapped_mint.clone())
            .owner(mint_authority)
            .amount(0)
            .with_extension(TransferHookAccount)
            .with_extension(ImmutableOwner)
            .build()
            .account
    };

    // Validation state account required in order for counter account to be passed
    // in transfer hook
    let validation_state_account =
        setup_validation_state_account(&hook_program_id, &counter, &unwrapped_mint);

    let starting_amount = 50_000;

    let wrap_result = WrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .unwrapped_mint(unwrapped_mint)
        .transfer_authority(transfer_authority)
        .unwrapped_token_account(unwrapped_token_account)
        .unwrapped_escrow_account(escrow_account)
        .add_extra_account(counter)
        .add_extra_account(KeyedAccount {
            key: hook_program_id,
            account: create_program_account_loader_v3(&hook_program_id),
        })
        .add_extra_account(validation_state_account)
        .execute();

    // Verify results
    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);

    // Verify counter was incremented
    let count = wrap_result.extra_accounts[0].clone().account.data[0];
    assert_eq!(count, 1)
}

#[test]
fn test_successfully_wraps_native_mint() {
    let starting_amount = 50_000;
    let wrap_amount = 12_555;

    let transfer_authority = TransferAuthority {
        keyed_account: Default::default(),
        signers: vec![],
    };

    let native_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_authority(Pubkey::new_unique())
        .mint_key(spl_token_2022::native_mint::id())
        .build();

    let native_token_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(native_mint.clone())
        .owner(transfer_authority.keyed_account.key)
        .amount(wrap_amount)
        .native_balance(wrap_amount)
        .build();

    let wrap_result = WrapBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .unwrapped_token_account(native_token_account)
        .unwrapped_mint(native_mint)
        .transfer_authority(transfer_authority)
        .wrapped_token_program(TokenProgram::SplToken)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn wrap_with_transfer_fee() {
    let wrap_amount = 500_000;
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(TransferFeeConfigExt)
        .build();
    let transfer_authority = KeyedAccount::default();
    let unwrapped_token_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(unwrapped_mint.clone())
        .owner(transfer_authority.key)
        .amount(wrap_amount)
        .with_extension(TransferFeeConfigExt)
        .build();
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority_pda = get_wrapped_mint_authority(&wrapped_mint_address);
    let escrow = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(unwrapped_mint.clone())
        .owner(wrapped_mint_authority_pda)
        .amount(0) // escrow is empty before wrap
        .with_extension(TransferFeeConfigExt)
        .with_extension(ImmutableOwner)
        .build()
        .account;

    let wrap_res = WrapBuilder::default()
        .unwrapped_mint(unwrapped_mint.clone())
        .unwrapped_token_account(unwrapped_token_account)
        .unwrapped_escrow_account(escrow)
        .transfer_authority(TransferAuthority {
            keyed_account: transfer_authority,
            signers: vec![],
        })
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .wrap_amount(wrap_amount)
        .recipient_starting_amount(0)
        .check(Check::success())
        .execute();

    let unwrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint.account.data).unwrap();
    let transfer_fee_config = unwrapped_mint_state
        .get_extension::<TransferFeeConfig>()
        .unwrap();

    let fee = transfer_fee_config
        .calculate_epoch_fee(0, wrap_amount)
        .unwrap();
    let net_transfer_to_escrow = wrap_amount.checked_sub(fee).unwrap();

    // Recipient of wrapped tokens receives the net amount
    let recipient_wrapped_token_state = PodStateWithExtensions::<PodAccount>::unpack(
        wrap_res.recipient_wrapped_token.account.data(),
    )
    .unwrap();
    assert_eq!(
        u64::from(recipient_wrapped_token_state.base.amount),
        net_transfer_to_escrow
    );

    // Escrow receives net amount from the user's source
    let escrow_state =
        PodStateWithExtensions::<PodAccount>::unpack(wrap_res.unwrapped_escrow.account.data())
            .unwrap();
    assert_eq!(u64::from(escrow_state.base.amount), net_transfer_to_escrow);

    // Fee shows as withheld in the escrow's TransferFeeAmount extension
    let escrow_transfer_fee_amount_ext = escrow_state.get_extension::<TransferFeeAmount>().unwrap();
    assert_eq!(
        u64::from(escrow_transfer_fee_amount_ext.withheld_amount),
        fee
    );
}
