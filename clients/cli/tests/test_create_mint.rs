use {
    crate::helpers::{setup, TOKEN_WRAP_CLI_BIN},
    serial_test::serial,
    solana_program_pack::Pack,
    spl_token::{self, state::Mint as SplTokenMint},
    spl_token_2022::state::Mint as SplToken2022Mint,
    spl_token_wrap::{
        self, get_wrapped_mint_address, get_wrapped_mint_backpointer_address, state::Backpointer,
    },
    std::process::Command,
};

mod helpers;

#[tokio::test]
#[serial]
async fn test_create_mint() {
    let env = setup().await;
    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "create-mint",
            "-C",
            &env.config_file_path,
            &env.unwrapped_mint.to_string(),
            &env.unwrapped_token_program.to_string(),
            &env.wrapped_token_program.to_string(),
            "--idempotent",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    // Derive expected account addresses
    let wrapped_mint_address =
        get_wrapped_mint_address(&env.unwrapped_mint, &env.wrapped_token_program);
    let backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    // Fetch created accounts
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let backpointer_account = env
        .rpc_client
        .get_account(&backpointer_address)
        .await
        .unwrap();

    // Verify owners
    assert_eq!(wrapped_mint_account.owner, env.wrapped_token_program);
    assert_eq!(backpointer_account.owner, spl_token_wrap::id());

    // Verify mint properties
    let unwrapped_mint_account = env
        .rpc_client
        .get_account(&env.unwrapped_mint)
        .await
        .unwrap();
    let unwrapped_mint_data = SplTokenMint::unpack(&unwrapped_mint_account.data).unwrap();
    let wrapped_mint_data = SplToken2022Mint::unpack(&wrapped_mint_account.data).unwrap();
    assert_eq!(wrapped_mint_data.decimals, unwrapped_mint_data.decimals);

    // Verify backpointer data
    let backpointer = *bytemuck::from_bytes::<Backpointer>(&backpointer_account.data);
    assert_eq!(backpointer.unwrapped_mint, env.unwrapped_mint);
}
