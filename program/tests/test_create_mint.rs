use solana_program::program_error::ProgramError;
use solana_program::system_program;
use solana_sdk::account::Account;
use {
    mollusk_svm::{result::Check, Mollusk},
    solana_program::pubkey::Pubkey,
    spl_token_wrap::instruction::create_mint,
};

#[test]
fn test_idempotency_false_with_existing_account() {
    let program_id = Pubkey::new_unique();

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_account,
        &wrapped_backpointer_account,
        &unwrapped_mint_account,
        &wrapped_token_program_id,
        false,
    );

    // Simulating existing data on mint or backpointer
    let account_with_data = Account {
        data: vec![1; 10],
        ..Account::default()
    };

    // idempotent: true causes these to fail
    let accounts = &[
        (wrapped_mint_account, account_with_data.clone()), // mint already exists
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::AccountAlreadyInitialized)],
    );

    let accounts = &[
        (wrapped_mint_account, Account::default()),
        (wrapped_backpointer_account, account_with_data), // backpointer already exists
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::AccountAlreadyInitialized)],
    );
}

#[test]
fn test_idempotency_true_with_existing_account() {
    let program_id = Pubkey::new_unique();

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_account,
        &wrapped_backpointer_account,
        &unwrapped_mint_account,
        &wrapped_token_program_id,
        true,
    );

    // Simulating existing data on mint or backpointer
    let account_with_data = Account {
        data: vec![1; 10],
        ..Account::default()
    };

    // idempotent: true causes these to return successfully
    let accounts = &[
        (wrapped_mint_account, account_with_data.clone()), // mint already exists
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(&instruction, accounts, &[Check::success()]);

    let accounts = &[
        (wrapped_mint_account, Account::default()),
        (wrapped_backpointer_account, account_with_data), // backpointer already exists
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(&instruction, accounts, &[Check::success()]);
}

#[test]
fn test_something() {
    let program_id = Pubkey::new_unique();

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_account,
        &wrapped_backpointer_account,
        &unwrapped_mint_account,
        &wrapped_token_program_id,
        false,
    );

    let accounts = &[
        (
            wrapped_mint_account,
            Account {
                lamports: 100_000_000, // Pre-funded
                ..Account::default()
            },
        ),
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(&instruction, accounts, &[Check::success()]);
}

// TODO:
//     - Test if not enough rent funded
