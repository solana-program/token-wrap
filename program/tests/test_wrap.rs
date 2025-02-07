use {
    crate::helpers::{
        mint_builder::{KeyedAccount, MintBuilder, TokenProgram},
        wrap_builder::{WrapBuilder, WrapResult},
    },
    mollusk_svm::{result::Check, Mollusk},
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
};

pub mod helpers;

#[test]
fn test_zero_amount_wrap() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();
    WrapBuilder::new(&mut mollusk, mint_result)
        .wrap_amount(0)
        .check(Check::err(ProgramError::InvalidArgument))
        .execute();
}

#[test]
fn test_incorrect_wrapped_mint_address() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();

    let incorrect_wrapped_mint = KeyedAccount {
        key: Pubkey::new_unique(), // Wrong mint address
        account: mint_result.wrapped_mint.account.clone(),
    };

    WrapBuilder::new(&mut mollusk, mint_result)
        .wrapped_mint(incorrect_wrapped_mint)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_incorrect_wrapped_mint_authority() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();

    let incorrect_authority = Pubkey::new_unique();
    WrapBuilder::new(&mut mollusk, mint_result)
        .wrapped_mint_authority(incorrect_authority)
        .check(Check::err(ProgramError::IncorrectAuthority))
        .execute();
}

#[test]
fn test_incorrect_escrow_address() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();

    let incorrect_escrow_addr = Pubkey::new_unique();
    WrapBuilder::new(&mut mollusk, mint_result)
        .unwrapped_escrow_addr(incorrect_escrow_addr)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_incorrect_escrow_owner() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();

    let incorrect_escrow_owner = Pubkey::new_unique();
    WrapBuilder::new(&mut mollusk, mint_result)
        .unwrapped_escrow_owner(incorrect_escrow_owner)
        .check(Check::err(ProgramError::InvalidAccountOwner))
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
    let recipient_token =
        spl_token::state::Account::unpack(&wrap_result.recipient_wrapped_token.account.data)
            .unwrap();
    assert_eq!(
        recipient_token.amount,
        starting_amount.checked_add(wrap_amount).unwrap()
    );
    assert_eq!(recipient_token.mint, wrap_result.wrapped_mint.key);

    // Verify wrapped mint supply increased
    let wrapped_mint =
        spl_token_2022::state::Mint::unpack(&wrap_result.wrapped_mint.account.data).unwrap();
    assert_eq!(wrapped_mint.supply, wrap_amount);
}

#[test]
fn test_wrap_amount_exceeds_balance() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk)
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::Token2022)
        .execute();

    // Try to wrap more tokens than we have in the account
    let starting_balance = 100;
    let wrap_amount = starting_balance + 1;

    WrapBuilder::new(&mut mollusk, mint_result)
        .wrap_amount(wrap_amount)
        .unwrapped_token_starting_amount(starting_balance)
        .check(Check::err(ProgramError::Custom(1)))
        .execute();
}

#[test]
fn test_wrap_with_uninitialized_escrow() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk).execute();

    // Create an uninitialized escrow account (just empty data)
    let uninitialized_escrow = Account {
        lamports: 100_000_000,
        owner: spl_token::id(),
        data: vec![0; spl_token::state::Account::LEN],
        ..Default::default()
    };

    WrapBuilder::new(&mut mollusk, mint_result)
        .unwrapped_escrow_account(uninitialized_escrow)
        .check(Check::err(ProgramError::UninitializedAccount))
        .execute();
}

#[test]
fn test_successful_spl_token_wrap() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk)
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::Token2022)
        .execute();

    let starting_amount = 50_000;
    let wrap_amount = 12_555;

    let wrap_result = WrapBuilder::new(&mut mollusk, mint_result)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_successful_token_2022_to_spl_token_wrap() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk)
        .unwrapped_token_program(TokenProgram::Token2022)
        .wrapped_token_program(TokenProgram::SplToken)
        .execute();

    let starting_amount = 64_532;
    let wrap_amount = 7_543;

    let wrap_result = WrapBuilder::new(&mut mollusk, mint_result)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}

#[test]
fn test_successful_token_2022_to_token_2022() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk)
        .unwrapped_token_program(TokenProgram::Token2022)
        .wrapped_token_program(TokenProgram::Token2022)
        .execute();

    let starting_amount = 345;
    let wrap_amount = 599;

    let wrap_result = WrapBuilder::new(&mut mollusk, mint_result)
        .recipient_starting_amount(starting_amount)
        .wrap_amount(wrap_amount)
        .execute();

    assert_wrap_result(starting_amount, wrap_amount, &wrap_result);
}
