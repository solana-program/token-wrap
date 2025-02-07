use crate::helpers::mint_builder::{MintBuilder, TokenProgram};
use crate::helpers::wrap_builder::WrapBuilder;
use mollusk_svm::result::Check;
use mollusk_svm::Mollusk;
use solana_program_error::ProgramError;
use solana_program_pack::Pack;

mod helpers;

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
fn test_successful_spl_token_wrap() {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let mint_result = MintBuilder::new(&mut mollusk)
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::Token2022)
        .execute();

    let starting_amount = 25;
    let recipient = WrapBuilder::setup_token_account(mint_result.wrapped_mint.key, starting_amount);
    let wrap_amount = 1234;

    let wrap_result = WrapBuilder::new(&mut mollusk, mint_result)
        .recipient(recipient)
        .wrap_amount(wrap_amount)
        .execute();

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
    assert_eq!(recipient_token.amount, starting_amount + wrap_amount);
    assert_eq!(recipient_token.mint, wrap_result.wrapped_mint.key);

    // Verify wrapped mint supply increased
    let wrapped_mint =
        spl_token_2022::state::Mint::unpack(&wrap_result.wrapped_mint.account.data).unwrap();
    assert_eq!(wrapped_mint.supply, wrap_amount);
}
