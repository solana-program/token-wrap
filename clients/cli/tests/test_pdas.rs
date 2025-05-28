use {
    crate::helpers::TOKEN_WRAP_CLI_BIN,
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        self, get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_authority,
        get_wrapped_mint_backpointer_address,
    },
    std::{process::Command, str::FromStr},
};

pub mod helpers;

#[test]
fn test_pdas() {
    let unwrapped_mint = Pubkey::from_str("FSi5sv14NFh71zDpBx1Ee1EFtDYRnN2fNYu7ixsPKNzJ").unwrap();

    // Execute the pdas command with JSON output
    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "find-pdas",
            &unwrapped_mint.to_string(),
            &spl_token::id().to_string(),
            &spl_token_2022::id().to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Parse the JSON output
    let output_str = String::from_utf8(output.stdout).unwrap();
    let json_result: serde_json::Value = serde_json::from_str(&output_str).unwrap();

    // Calculate the expected addresses
    let expected_wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &spl_token_2022::id());
    let expected_authority = get_wrapped_mint_authority(&expected_wrapped_mint);
    let expected_backpointer = get_wrapped_mint_backpointer_address(&expected_wrapped_mint);
    let expected_escrow =
        get_escrow_address(&unwrapped_mint, &spl_token::id(), &spl_token_2022::id());

    // Verify the JSON values match the expected addresses
    assert_eq!(
        json_result["wrappedMintAddress"].as_str().unwrap(),
        expected_wrapped_mint.to_string(),
    );

    assert_eq!(
        json_result["wrappedMintAuthority"].as_str().unwrap(),
        expected_authority.to_string(),
    );

    assert_eq!(
        json_result["unwrappedEscrow"].as_str().unwrap(),
        expected_escrow.to_string(),
    );

    assert_eq!(
        json_result["wrappedBackpointerAddress"].as_str().unwrap(),
        expected_backpointer.to_string(),
    );
}
