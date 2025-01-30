use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::program_pack::Pack;
use solana_program::rent::Rent;
use solana_program::system_program;
use solana_sdk::account::Account;
use spl_token_2022::state::Mint;
use spl_token_wrap::instruction::TokenWrapInstruction;
use spl_token_wrap::state::Backpointer;
use {
    mollusk_svm::{result::Check, Mollusk},
    solana_program::pubkey::Pubkey,
    spl_token_wrap::instruction::create_mint,
};

#[test]
fn test_account_flags() {
    let program_id = Pubkey::new_unique();

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

    // 1. Wrong flag on wrapped_mint_account
    let account_metas = vec![
        AccountMeta::new_readonly(wrapped_mint_account, false),
        AccountMeta::new(wrapped_backpointer_account, false),
        AccountMeta::new_readonly(unwrapped_mint_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent: false }.pack();
    let instruction = Instruction::new_with_bytes(program_id, &data, account_metas);

    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let accounts = &[
        (wrapped_mint_account, Account::default()),
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidArgument)],
    );

    // 2. Wrong flag on wrapped_backpointer_account
    let account_metas = vec![
        AccountMeta::new(wrapped_mint_account, false),
        AccountMeta::new_readonly(wrapped_backpointer_account, false),
        AccountMeta::new_readonly(unwrapped_mint_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent: false }.pack();
    let instruction = Instruction::new_with_bytes(program_id, &data, account_metas);
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidArgument)],
    );

    // 3. Wrong flag on unwrapped_mint_account
    let account_metas = vec![
        AccountMeta::new(wrapped_mint_account, false),
        AccountMeta::new(wrapped_backpointer_account, false),
        AccountMeta::new(unwrapped_mint_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent: false }.pack();
    let instruction = Instruction::new_with_bytes(program_id, &data, account_metas);
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidArgument)],
    );

    // 4. Wrong flag on system_program
    let account_metas = vec![
        AccountMeta::new(wrapped_mint_account, false),
        AccountMeta::new(wrapped_backpointer_account, false),
        AccountMeta::new_readonly(unwrapped_mint_account, false),
        AccountMeta::new(system_program::id(), false),
        AccountMeta::new_readonly(wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent: false }.pack();
    let instruction = Instruction::new_with_bytes(program_id, &data, account_metas);
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidArgument)],
    );

    // 5. Wrong flag on wrapped_token_program_id
    let account_metas = vec![
        AccountMeta::new(wrapped_mint_account, false),
        AccountMeta::new(wrapped_backpointer_account, false),
        AccountMeta::new(unwrapped_mint_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent: false }.pack();
    let instruction = Instruction::new_with_bytes(program_id, &data, account_metas);
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidArgument)],
    );
}

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
fn test_create_mint_insufficient_funds() {
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

    // Calculate minimum rent for Mint account
    let rent = Rent::default(); // Using default rent for test
    let space = Mint::get_packed_len();
    let mint_rent_required = rent.minimum_balance(space);

    // Create wrapped_mint_account with insufficient lamports
    let insufficient_lamports = mint_rent_required - 1; // Less than required rent
    let wrapped_mint_account_insufficent_funds = Account {
        lamports: insufficient_lamports,
        ..Account::default()
    };

    let accounts = &[
        (wrapped_mint_account, wrapped_mint_account_insufficent_funds),
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InsufficientFunds)],
    );
}

#[test]
fn test_create_mint_backpointer_insufficient_funds() {
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

    // Calculate minimum rent for Backpointer account
    let rent = Rent::default(); // Using default rent for test
    let backpointer_space = std::mem::size_of::<Backpointer>();
    let backpointer_rent_required = rent.minimum_balance(backpointer_space);

    // Create wrapped_backpointer_account with insufficient lamports
    let insufficient_lamports = backpointer_rent_required - 1; // Less than required rent
    let wrapped_backpointer_account_insufficent_funds = Account {
        lamports: insufficient_lamports,
        ..Account::default()
    };

    let accounts = &[
        (wrapped_mint_account, Account::default()),
        (
            wrapped_backpointer_account,
            wrapped_backpointer_account_insufficent_funds,
        ),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InsufficientFunds)],
    );
}

// TODO: In progress. Should assert after success:
//       - wrapped_mint_account is initialized, owner is ?
//       - wrapped_backpointer_account, owner is token wrap program
//       - unwrapped_mint_account is unchanged
#[test]
fn test_success() {
    let program_id = Pubkey::new_unique();

    let wrapped_mint_account = Pubkey::new_unique(); // TODO: Don't use a random one, create a real mint
    let wrapped_backpointer_account = Pubkey::new_unique(); // TODO: Don't use a random one, create a real mint
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
