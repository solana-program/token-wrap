use {
    crate::helpers::{
        close_stuck_escrow_builder::CloseStuckEscrowBuilder,
        common::{init_mollusk, KeyedAccount, TokenProgram, DEFAULT_MINT_DECIMALS},
        extensions::MintExtension::{MintCloseAuthority, TransferHook},
        mint_builder::MintBuilder,
        token_account_builder::TokenAccountBuilder,
    },
    mollusk_svm::{program::keyed_account_for_system_program, result::Check},
    mollusk_svm_programs_token::token2022,
    solana_account::Account,
    solana_instruction::Instruction,
    solana_program_error::ProgramError,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_associated_token_account_client::{
        address::get_associated_token_address_with_program_id,
        instruction::create_associated_token_account,
    },
    spl_token_2022::{
        extension::{
            transfer_fee::instruction::initialize_transfer_fee_config,
            BaseStateWithExtensionsMut,
            ExtensionType::{self, ImmutableOwner, TransferFeeConfig, TransferHookAccount},
            PodStateWithExtensionsMut,
        },
        instruction::initialize_mint2,
        pod::PodAccount,
        state::{AccountState, Mint},
    },
    spl_token_wrap::{
        error::TokenWrapError, get_escrow_address, get_wrapped_mint_address,
        get_wrapped_mint_authority, get_wrapped_mint_backpointer_address, state::Backpointer,
    },
};

pub mod helpers;

#[test]
fn test_close_stuck_escrow_fails_for_spl_token() {
    CloseStuckEscrowBuilder::default()
        .escrow_owner(spl_token::id()) // Invalid owner
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_for_spl_token_mint() {
    CloseStuckEscrowBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken) // Set unwrapped mint owner to spl-token
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_wrapped_mint_mismatch() {
    let incorrect_wrapped_mint = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            owner: spl_token_2022::id(),
            ..Default::default()
        },
    };

    CloseStuckEscrowBuilder::default()
        .wrapped_mint(incorrect_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_wrapped_mint_authority_mismatch() {
    let incorrect_authority = Pubkey::new_unique();

    CloseStuckEscrowBuilder::default()
        .wrapped_mint_authority(incorrect_authority)
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_escrow_mismatch() {
    let incorrect_escrow = KeyedAccount {
        key: Pubkey::new_unique(), // Not the derived ATA
        account: Account {
            owner: spl_token_2022::id(),
            lamports: Rent::default().minimum_balance(spl_token_2022::state::Account::LEN),
            ..Default::default()
        },
    };
    CloseStuckEscrowBuilder::default()
        .escrow_account(incorrect_escrow)
        .check(Check::err(TokenWrapError::EscrowMismatch.into()))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_escrow_owner_mismatch() {
    let unwrapped_token_program = TokenProgram::SplToken2022;
    let wrapped_token_program = TokenProgram::SplToken2022;

    let unwrapped_mint = MintBuilder::new()
        .token_program(unwrapped_token_program)
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(wrapped_token_program)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
        ))
        .mint_authority(Pubkey::new_unique())
        .build();

    // This is the correct PDA that *should* own the escrow account's tokens.
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    let expected_escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &unwrapped_token_program.id(),
        &wrapped_token_program.id(),
    );

    // Create an escrow account at the correct ATA, but set its internal owner field
    // to a random, incorrect public key.
    let account_size = spl_token_2022::state::Account::LEN;
    let mut account_data = vec![0; account_size];
    let mut state =
        PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut account_data).unwrap();
    state.base.mint = unwrapped_mint.key;
    state.base.owner = Pubkey::new_unique();
    state.base.amount = 0.into();
    state.base.state = AccountState::Initialized.into();
    state.init_account_type().unwrap();

    let escrow_with_wrong_owner = KeyedAccount {
        key: expected_escrow_address,
        account: Account {
            lamports: Rent::default().minimum_balance(account_size),
            data: account_data,
            owner: spl_token_2022::id(),
            ..Default::default()
        },
    };

    CloseStuckEscrowBuilder::default()
        .unwrapped_token_program(unwrapped_token_program)
        .wrapped_token_program(wrapped_token_program)
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .wrapped_mint_authority(wrapped_mint_authority)
        .escrow_account(escrow_with_wrong_owner)
        .check(Check::err(TokenWrapError::EscrowOwnerMismatch.into()))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_with_non_zero_balance() {
    let unwrapped_token_program = TokenProgram::SplToken2022;
    let wrapped_token_program = TokenProgram::SplToken2022;

    let unwrapped_mint = MintBuilder::new()
        .token_program(unwrapped_token_program)
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(wrapped_token_program)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
        ))
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    let expected_escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &unwrapped_token_program.id(),
        &wrapped_token_program.id(),
    );

    // Create the escrow account with a non-zero balance
    let account_size = spl_token_2022::state::Account::LEN;
    let mut account_data = vec![0; account_size];
    let mut state =
        PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut account_data).unwrap();
    state.base.mint = unwrapped_mint.key;
    state.base.owner = wrapped_mint_authority;
    state.base.amount = 100.into(); // Non-zero balance
    state.base.state = AccountState::Initialized.into();
    state.init_account_type().unwrap();

    let escrow_with_balance = KeyedAccount {
        key: expected_escrow_address,
        account: Account {
            lamports: Rent::default().minimum_balance(account_size),
            data: account_data,
            owner: spl_token_2022::id(),
            ..Default::default()
        },
    };

    CloseStuckEscrowBuilder::default()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .escrow_account(escrow_with_balance)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_when_in_good_state() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_authority(Pubkey::new_unique())
        .with_extension(TransferHook)
        .build();
    let wrapped_token_program = TokenProgram::SplToken2022;

    let wrapped_mint = MintBuilder::new()
        .token_program(wrapped_token_program)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
        ))
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    let escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &unwrapped_mint.account.owner,
        &wrapped_token_program.id(),
    );

    let good_escrow_account = KeyedAccount {
        key: escrow_address,
        account: TokenAccountBuilder::new()
            .token_program(TokenProgram::SplToken2022)
            .mint(unwrapped_mint.clone())
            .owner(wrapped_mint_authority)
            .amount(0)
            .with_extension(ImmutableOwner)
            .with_extension(TransferHookAccount)
            .build()
            .account,
    };

    CloseStuckEscrowBuilder::default()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .escrow_account(good_escrow_account)
        .check(Check::err(TokenWrapError::EscrowInGoodState.into()))
        .execute();
}

#[test]
fn test_close_stuck_escrow_succeeds() {
    let wrapped_token_program = TokenProgram::SplToken2022;

    // The "new" mint, with extensions that the old escrow doesn't have.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_authority(Pubkey::new_unique())
        .with_extension(TransferHook)
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(wrapped_token_program)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
        ))
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    let escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &unwrapped_mint.account.owner,
        &wrapped_token_program.id(),
    );

    // The "old" escrow, initialized for a mint with no extensions, making it stuck.
    let account_size = spl_token_2022::state::Account::LEN;
    let mut account_data = vec![0; account_size];
    let mut state =
        PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut account_data).unwrap();
    state.base.mint = unwrapped_mint.key;
    state.base.owner = wrapped_mint_authority;
    state.base.amount = 0.into();
    state.base.state = AccountState::Initialized.into();
    state.init_account_type().unwrap();

    let stuck_escrow = KeyedAccount {
        key: escrow_address,
        account: Account {
            lamports: Rent::default().minimum_balance(account_size),
            data: account_data,
            owner: spl_token_2022::id(),
            ..Default::default()
        },
    };
    let initial_escrow_lamports = stuck_escrow.account.lamports;

    let destination = KeyedAccount::default();
    let initial_destination_lamports = destination.account.lamports;

    CloseStuckEscrowBuilder::default()
        .wrapped_token_program(wrapped_token_program)
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .escrow_account(stuck_escrow.clone())
        .destination_account(destination.clone())
        .check(Check::account(&stuck_escrow.key).closed().build())
        .check(
            Check::account(&destination.key)
                .lamports(initial_destination_lamports + initial_escrow_lamports)
                .build(),
        )
        .execute();
}

#[test]
fn test_close_stuck_escrow_fails_when_account_frozen() {
    let unwrapped_token_program = TokenProgram::SplToken2022;
    let wrapped_token_program = TokenProgram::SplToken2022;

    let unwrapped_mint = MintBuilder::new()
        .token_program(unwrapped_token_program)
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(wrapped_token_program)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &wrapped_token_program.id(),
        ))
        .mint_authority(Pubkey::new_unique())
        .build();

    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint.key);
    let escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &unwrapped_mint.account.owner,
        &wrapped_token_program.id(),
    );

    let frozen_escrow_account = KeyedAccount {
        key: escrow_address,
        account: TokenAccountBuilder::new()
            .token_program(TokenProgram::SplToken2022)
            .mint(unwrapped_mint.clone())
            .owner(wrapped_mint_authority)
            .amount(0)
            .state(AccountState::Frozen)
            .with_extension(ImmutableOwner)
            .build()
            .account,
    };

    CloseStuckEscrowBuilder::default()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .escrow_account(frozen_escrow_account)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_end_to_end_close_mint_case() {
    let mollusk = init_mollusk();
    let payer = Pubkey::new_unique();
    let close_authority = Pubkey::new_unique();

    // Create an unwrapped mint that has a close authority.
    let unwrapped_mint_addr = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_addr)
        .mint_authority(Pubkey::new_unique())
        .decimals(DEFAULT_MINT_DECIMALS)
        .supply(0)
        .with_extension(MintCloseAuthority(close_authority))
        .build();

    // Derive all necessary PDAs
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let escrow_address = get_escrow_address(
        &unwrapped_mint.key,
        &TokenProgram::SplToken2022.id(),
        &spl_token_2022::id(),
    );

    let create_mint_ix = spl_token_wrap::instruction::create_mint(
        &spl_token_wrap::id(),
        &wrapped_mint_address,
        &backpointer_address,
        &unwrapped_mint.key,
        &spl_token_2022::id(),
        false,
    );

    // This is the account created based on the size of the old mint
    let create_escrow_ix = create_associated_token_account(
        &payer,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        &spl_token_2022::id(),
    );

    let close_unwrapped_mint_ix = spl_token_2022::instruction::close_account(
        &spl_token_2022::id(),
        &unwrapped_mint.key,
        &payer,
        &close_authority,
        &[],
    )
    .unwrap();

    // The newly created mint at the same address w/ different extensions and space
    let mint_extensions = vec![TransferFeeConfig];
    let mint_space = ExtensionType::try_calculate_account_len::<Mint>(&mint_extensions).unwrap();
    let create_mint_account_ix = solana_system_interface::instruction::create_account(
        &payer,
        &unwrapped_mint_addr,
        Rent::default().minimum_balance(mint_space),
        mint_space as u64,
        &spl_token_2022::id(),
    );

    let init_mint_ix = initialize_mint2(
        &spl_token_2022::id(),
        &unwrapped_mint_addr,
        &payer,
        None,
        DEFAULT_MINT_DECIMALS,
    )
    .unwrap();

    let init_transfer_fee_ix = initialize_transfer_fee_config(
        &spl_token_2022::id(),
        &unwrapped_mint_addr,
        Some(&payer),
        Some(&payer),
        100,
        1_000_000,
    )
    .unwrap();

    let close_stuck_escrow_ix = spl_token_wrap::instruction::close_stuck_escrow(
        &spl_token_wrap::id(),
        &escrow_address,
        &payer,
        &unwrapped_mint_addr,
        &wrapped_mint_address,
        &wrapped_mint_authority,
    );

    let create_recipient_wrapped_ix = create_associated_token_account(
        &payer,
        &payer,
        &wrapped_mint_address,
        &spl_token_2022::id(),
    );
    let recipient_wrapped_addr = get_associated_token_address_with_program_id(
        &payer,
        &wrapped_mint_address,
        &spl_token_2022::id(),
    );

    let wrap_amount: u64 = 5_000;

    let unwrapped_token_account_addr = Pubkey::new_unique();
    let unwrapped_token_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(unwrapped_mint.clone())
        .owner(payer)
        .amount(wrap_amount)
        .with_extension(TransferFeeConfig)
        .build()
        .account;

    let wrap_ix = spl_token_wrap::instruction::wrap(
        &spl_token_wrap::id(),
        &recipient_wrapped_addr,
        &wrapped_mint_address,
        &wrapped_mint_authority,
        &spl_token_2022::id(),
        &spl_token_2022::id(),
        &unwrapped_token_account_addr,
        &unwrapped_mint.key,
        &escrow_address,
        &payer,
        &[],
        wrap_amount,
    );

    // These accounts represent the state before any instructions are run.
    let initial_accounts = vec![
        (
            payer,
            Account {
                lamports: 10_000_000_000,
                ..Default::default()
            },
        ),
        (close_authority, Account::default()),
        unwrapped_mint.pair(),
        (
            wrapped_mint_address,
            Account {
                lamports: mollusk.sysvars.rent.minimum_balance(
                    ExtensionType::try_calculate_account_len::<Mint>(&[
                        ExtensionType::ConfidentialTransferMint,
                    ])
                    .unwrap(),
                ),
                ..Default::default()
            },
        ),
        (
            backpointer_address,
            Account {
                lamports: mollusk
                    .sysvars
                    .rent
                    .minimum_balance(std::mem::size_of::<Backpointer>()),
                ..Default::default()
            },
        ),
        (escrow_address, Account::default()),
        (wrapped_mint_authority, Account::default()),
        keyed_account_for_system_program(),
        token2022::keyed_account(),
        token2022::keyed_account(),
        mollusk_svm_programs_token::associated_token::keyed_account(),
        (recipient_wrapped_addr, Account::default()),
        (unwrapped_token_account_addr, unwrapped_token_account),
    ];

    let success_check = [Check::success()];
    let close_unwrapped_mint_checks = [
        Check::success(),
        Check::account(&unwrapped_mint.key).closed().build(),
    ];
    let checks_and_instructions: Vec<(&Instruction, &[Check])> = vec![
        (&create_mint_ix, &success_check),
        (&create_escrow_ix, &success_check),
        (&close_unwrapped_mint_ix, &close_unwrapped_mint_checks),
        (&create_mint_account_ix, &success_check),
        (&init_transfer_fee_ix, &success_check),
        (&init_mint_ix, &success_check),
        (&close_stuck_escrow_ix, &success_check),
        (&create_escrow_ix, &success_check),
        (&create_recipient_wrapped_ix, &success_check),
        (&wrap_ix, &success_check),
    ];

    mollusk.process_and_validate_instruction_chain(&checks_and_instructions, &initial_accounts);
}
