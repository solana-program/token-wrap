use solana_program::system_program;
use solana_sdk::account::Account;
use {
    mollusk_svm::{result::Check, Mollusk},
    solana_program::pubkey::Pubkey,
    spl_token_wrap::instruction::create_mint,
};

#[test]
fn test_create_mint_idempotency() {
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
        (wrapped_mint_account, Account::default()),
        (wrapped_backpointer_account, Account::default()),
        (unwrapped_mint_account, Account::default()),
        (system_program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(&instruction, accounts, &[Check::success()]);
}
