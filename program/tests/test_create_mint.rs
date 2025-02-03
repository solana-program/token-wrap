use solana_program::program_error::ProgramError;
use solana_program::program_pack::Pack;
use solana_program::rent::Rent;
use solana_program::system_program;
use solana_sdk::account::Account;
use solana_sdk::program_option::COption;
use spl_token_2022::state::Mint;
use spl_token_wrap::state::Backpointer;
use spl_token_wrap::{
    get_wrapped_mint_address, get_wrapped_mint_authority, get_wrapped_mint_backpointer_address,
};
use {
    mollusk_svm::{result::Check, Mollusk},
    solana_program::pubkey::Pubkey,
    spl_token_wrap::instruction::create_mint,
};

const MINT_DECIMALS: u8 = 12;
const MINT_SUPPLY: u64 = 500_000_000;
const FREEZE_AUTHORITY: &str = "11111115q4EpJaTXAZWpCg3J2zppWGSZ46KXozzo9";

fn setup_spl_mint(rent: &Rent) -> Account {
    let state = spl_token::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        freeze_authority: COption::Some(Pubkey::from_str_const(FREEZE_AUTHORITY)),
        ..Default::default()
    };
    let mut data = vec![0u8; spl_token::state::Mint::LEN];
    state.pack_into_slice(&mut data);

    let lamports = rent.minimum_balance(data.len());

    Account {
        lamports,
        data,
        owner: spl_token::id(),
        ..Default::default()
    }
}

fn setup_token_2022_mint(rent: &Rent) -> Account {
    let state = spl_token_2022::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        freeze_authority: COption::Some(Pubkey::from_str_const(FREEZE_AUTHORITY)),
        ..Default::default()
    };
    let mut data = vec![0u8; spl_token_2022::state::Mint::LEN];
    state.pack_into_slice(&mut data);

    let lamports = rent.minimum_balance(data.len());

    Account {
        lamports,
        data,
        owner: spl_token_2022::id(),
        ..Default::default()
    }
}

#[test]
fn test_idempotency_false_with_existing_account() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

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
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

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
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let wrapped_mint_account = Pubkey::new_unique();
    let wrapped_backpointer_account = Pubkey::new_unique();
    let unwrapped_mint_account = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();

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
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
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

    let rent = &mollusk.sysvars.rent;
    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            wrapped_backpointer_account_insufficent_funds,
        ),
        (unwrapped_mint_address, setup_spl_mint(rent)),
        (
            system_program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InsufficientFunds)],
    );
}

#[test]
fn test_successful_spl_token_to_token_2022() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let rent = &mollusk.sysvars.rent;
    let unwrapped_mint_account = setup_spl_mint(rent);

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            system_program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    let result = mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[
            Check::success(),
            // Ensure unwrapped_mint_account remains unchanged
            Check::account(&unwrapped_mint_address)
                .data(&unwrapped_mint_account.data)
                .build(),
        ],
    );

    // Assert state of resulting wrapped mint account

    let resulting_wrapped_mint_account = &result.resulting_accounts[0].1;
    assert_eq!(resulting_wrapped_mint_account.owner, spl_token_2022::id());

    let wrapped_mint_data = Mint::unpack(&resulting_wrapped_mint_account.data).unwrap();
    assert_eq!(wrapped_mint_data.decimals, MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    assert_eq!(
        wrapped_mint_data.mint_authority.unwrap(),
        expected_mint_authority,
    );
    assert_eq!(wrapped_mint_data.supply, 0);
    assert!(wrapped_mint_data.is_initialized);
    assert_eq!(
        wrapped_mint_data.freeze_authority.unwrap(),
        Pubkey::from_str_const(FREEZE_AUTHORITY)
    );

    // Assert state of resulting backpointer account

    let resulting_backpointer_account = &result.resulting_accounts[1].1;
    assert_eq!(resulting_backpointer_account.owner, program_id);

    let backpointer = bytemuck::from_bytes::<Backpointer>(&resulting_backpointer_account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint_address);
}

#[test]
fn test_successful_token_2022_to_spl_token() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let rent = &mollusk.sysvars.rent;
    let unwrapped_mint_account = setup_token_2022_mint(rent);

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            system_program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token::keyed_account(),
    ];
    let result = mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[
            Check::success(),
            // Ensure unwrapped_mint_account remains unchanged
            Check::account(&unwrapped_mint_address)
                .data(&unwrapped_mint_account.data)
                .build(),
        ],
    );

    // Assert state of resulting wrapped mint account

    let resulting_wrapped_mint_account = &result.resulting_accounts[0].1;
    assert_eq!(resulting_wrapped_mint_account.owner, spl_token::id());

    let wrapped_mint_data = Mint::unpack(&resulting_wrapped_mint_account.data).unwrap();
    assert_eq!(wrapped_mint_data.decimals, MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    assert_eq!(
        wrapped_mint_data.mint_authority.unwrap(),
        expected_mint_authority,
    );
    assert_eq!(wrapped_mint_data.supply, 0);
    assert!(wrapped_mint_data.is_initialized);
    assert_eq!(
        wrapped_mint_data.freeze_authority.unwrap(),
        Pubkey::from_str_const(FREEZE_AUTHORITY)
    );

    // Assert state of resulting backpointer account

    let resulting_backpointer_account = &result.resulting_accounts[1].1;
    assert_eq!(resulting_backpointer_account.owner, program_id);

    let backpointer = bytemuck::from_bytes::<Backpointer>(&resulting_backpointer_account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint_address);
}
